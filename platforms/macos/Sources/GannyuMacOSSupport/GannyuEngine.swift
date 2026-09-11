import Foundation
import CGannyuInput

public struct GannyuRetrievedCandidate: Decodable {
    public let text: String
    public let annotation: String
    public let reading: String?
    public let mandarinReading: String?
    public let consumedBytes: Int

    private enum CodingKeys: String, CodingKey {
        case text, annotation, reading
        case mandarinReading = "mandarin_reading"
        case consumedBytes = "consumed_bytes"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        text = try container.decode(String.self, forKey: .text)
        annotation = try container.decodeIfPresent(String.self, forKey: .annotation) ?? ""
        reading = try container.decodeIfPresent(String.self, forKey: .reading)
        mandarinReading = try container.decodeIfPresent(String.self, forKey: .mandarinReading)
        consumedBytes = try container.decodeIfPresent(Int.self, forKey: .consumedBytes) ?? text.utf8.count
    }
}

public struct GannyuRegion: Decodable, Equatable {
    public let id: String
    public let nameZh: String

    private enum CodingKeys: String, CodingKey {
        case id
        case nameZh = "name_zh"
    }

    public init(id: String, nameZh: String) {
        self.id = id
        self.nameZh = nameZh
    }
}

public final class GannyuRegionStore {
    public static let shared = GannyuRegionStore()
    public static let didChange = Notification.Name("org.doohaey.gonnyu.regionDidChange")
    private let key = "org.doohaey.gonnyu.region"

    public func currentID(manifestPath: String?) -> String? {
        guard let regions = try? GannyuEngine.availableRegions(manifestPath: manifestPath),
              let first = regions.first else {
            return nil
        }
        let stored = UserDefaults.standard.string(forKey: key)
        let resolved = regions.contains(where: { $0.id == stored }) ? stored! : first.id
        if stored != resolved {
            UserDefaults.standard.set(resolved, forKey: key)
        }
        return resolved
    }

    public func select(_ regionID: String, manifestPath: String?) -> Bool {
        guard let regions = try? GannyuEngine.availableRegions(manifestPath: manifestPath),
              regions.contains(where: { $0.id == regionID }) else {
            return false
        }
        UserDefaults.standard.set(regionID, forKey: key)
        NotificationCenter.default.post(name: Self.didChange, object: regionID)
        return true
    }
}

public enum GannyuEngineError: Error, CustomStringConvertible {
    case status(Int32, String)

    public var description: String {
        switch self {
        case let .status(code, message):
            return "ffi status \(code): \(message)"
        }
    }
}

public final class GannyuEngine {
    private var handle: OpaquePointer?

    public init(manifestPath: String?, regionID: String?) throws {
        var created: OpaquePointer?
        let status = withOptionalCString(manifestPath) { manifest in
            withOptionalCString(regionID) { region in
                gannyu_pipeline_create(manifest, region, &created)
            }
        }
        guard status == gannyu_ffi_status_ok(), let created else {
            throw GannyuEngineError.status(status, lastErrorMessage())
        }
        handle = created
    }

    deinit {
        if let handle {
            gannyu_pipeline_destroy(handle)
        }
    }

    public func entryCount() -> Int32 {
        guard let handle else {
            return -1
        }
        return gannyu_pipeline_entry_count(handle)
    }

    public func retrieve(_ input: String) throws -> String {
        try jsonCall(input) { handle, text, out in
            gannyu_pipeline_retrieve(handle, text, out)
        }
    }

    public func retrieveCandidates(_ input: String) throws -> [GannyuRetrievedCandidate] {
        let data = Data(try retrieve(input).utf8)
        return try JSONDecoder().decode([GannyuRetrievedCandidate].self, from: data)
    }

    public static func availableRegions(manifestPath: String?) throws -> [GannyuRegion] {
        var output: UnsafeMutablePointer<CChar>?
        let status = withOptionalCString(manifestPath) { manifest in
            gannyu_region_list(manifest, &output)
        }
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuEngineError.status(status, lastErrorMessage())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode([GannyuRegion].self, from: Data(String(cString: output).utf8))
    }

    public func boostUserWord(_ text: String) {
        guard let handle else { return }
        text.withCString { headword in
            _ = gannyu_pipeline_user_dict_boost(handle, headword, nil)
        }
    }

    public func saveUserWord(_ text: String, reading: String, mandarinReading: String?) {
        guard let handle, text.count >= 2, !reading.isEmpty else { return }
        text.withCString { headword in
            reading.withCString { pinyin in
                withOptionalCString(mandarinReading) { mandarin in
                    _ = gannyu_pipeline_user_dict_add(handle, headword, pinyin, mandarin, nil)
                }
            }
        }
    }

    public func compose(_ input: String) throws -> String {
        try jsonCall(input) { handle, text, out in
            gannyu_pipeline_compose(handle, text, out)
        }
    }

    public func formatPreedit(_ input: String, consumedBytes: Int = 0) throws -> String {
        guard let handle else {
            throw GannyuEngineError.status(-1, "pipeline not initialized")
        }
        var output: UnsafeMutablePointer<CChar>?
        let status = input.withCString { text in
            gannyu_pipeline_format_preedit(handle, text, consumedBytes, &output)
        }
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuEngineError.status(status, lastErrorMessage())
        }
        defer { gannyu_string_destroy(output) }
        return String(cString: output)
    }

    private func jsonCall(
        _ input: String,
        run: (OpaquePointer?, UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32
    ) throws -> String {
        guard let handle else {
            throw GannyuEngineError.status(-1, "pipeline not initialized")
        }
        var output: UnsafeMutablePointer<CChar>?
        let status = input.withCString { text in
            run(handle, text, &output)
        }
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuEngineError.status(status, lastErrorMessage())
        }
        defer { gannyu_string_destroy(output) }
        return String(cString: output)
    }
}

private func withOptionalCString<Result>(
    _ value: String?,
    body: (UnsafePointer<CChar>?) -> Result
) -> Result {
    guard let value else {
        return body(nil)
    }
    return value.withCString(body)
}

private func lastErrorMessage() -> String {
    var output: UnsafeMutablePointer<CChar>?
    let status = gannyu_last_error(&output)
    guard status == gannyu_ffi_status_ok(), let output else {
        return "unknown error"
    }
    defer { gannyu_string_destroy(output) }
    return String(cString: output)
}
