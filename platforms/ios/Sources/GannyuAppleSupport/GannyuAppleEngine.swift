import Foundation
import CGannyuInput

public struct GannyuAppleCandidate: Decodable {
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

public struct GannyuAppleRegion: Decodable, Equatable {
    public let id: String
    public let nameZh: String

    private enum CodingKeys: String, CodingKey {
        case id
        case nameZh = "name_zh"
    }
}

public enum GannyuAppleEngineError: Error {
    case ffi(Int32, String)
}

public final class GannyuAppleEngine {
    private var handle: OpaquePointer?

    public init(regionID: String) throws {
        var created: OpaquePointer?
        let status = regionID.withCString { region in
            gannyu_pipeline_create(nil, region, &created)
        }
        guard status == gannyu_ffi_status_ok(), let created else {
            throw GannyuAppleEngineError.ffi(status, Self.lastError())
        }
        handle = created
    }

    deinit {
        if let handle {
            gannyu_pipeline_destroy(handle)
        }
    }

    public static func regions() throws -> [GannyuAppleRegion] {
        var output: UnsafeMutablePointer<CChar>?
        let status = gannyu_region_list(nil, &output)
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuAppleEngineError.ffi(status, lastError())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode([GannyuAppleRegion].self, from: Data(String(cString: output).utf8))
    }

    public func candidates(for input: String) throws -> [GannyuAppleCandidate] {
        guard let handle else { return [] }
        var output: UnsafeMutablePointer<CChar>?
        let status = input.withCString { value in
            gannyu_pipeline_retrieve(handle, value, &output)
        }
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuAppleEngineError.ffi(status, Self.lastError())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode([GannyuAppleCandidate].self, from: Data(String(cString: output).utf8))
    }

    public func formatPreedit(_ input: String, consumedBytes: Int = 0) throws -> String {
        guard let handle else { return input }
        var output: UnsafeMutablePointer<CChar>?
        let status = input.withCString { value in
            gannyu_pipeline_format_preedit(handle, value, consumedBytes, &output)
        }
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuAppleEngineError.ffi(status, Self.lastError())
        }
        defer { gannyu_string_destroy(output) }
        return String(cString: output)
    }

    public func boost(_ text: String) {
        guard let handle else { return }
        text.withCString { value in
            _ = gannyu_pipeline_user_dict_boost(handle, value, nil)
        }
    }

    public func saveUserWord(_ text: String, reading: String, mandarinReading: String?) {
        guard let handle, text.count >= 2, !reading.isEmpty else { return }
        text.withCString { headword in
            reading.withCString { pinyin in
                if let mandarinReading {
                    mandarinReading.withCString { mandarin in
                        _ = gannyu_pipeline_user_dict_add(handle, headword, pinyin, mandarin, nil)
                    }
                } else {
                    _ = gannyu_pipeline_user_dict_add(handle, headword, pinyin, nil, nil)
                }
            }
        }
    }

    private static func lastError() -> String {
        var output: UnsafeMutablePointer<CChar>?
        guard gannyu_last_error(&output) == gannyu_ffi_status_ok(), let output else {
            return "unknown FFI error"
        }
        defer { gannyu_string_destroy(output) }
        return String(cString: output)
    }
}

public final class GannyuAppleRegionStore {
    public static let didChange = Notification.Name("org.doohaey.gonnyu.apple.regionDidChange")
    private let key = "org.doohaey.gonnyu.region"
    private let defaults: UserDefaults

    public init(bundle: Bundle = .main) {
        guard let group = bundle.object(forInfoDictionaryKey: "GannyuAppGroupIdentifier") as? String,
              let defaults = UserDefaults(suiteName: group) else {
            preconditionFailure("GannyuAppGroupIdentifier must resolve to an App Group")
        }
        self.defaults = defaults
    }

    public func currentID(in regions: [GannyuAppleRegion]) -> String? {
        guard let first = regions.first else { return nil }
        let stored = defaults.string(forKey: key)
        let resolved = regions.contains(where: { $0.id == stored }) ? stored! : first.id
        if stored != resolved {
            defaults.set(resolved, forKey: key)
        }
        return resolved
    }

    public func select(_ id: String, in regions: [GannyuAppleRegion]) -> Bool {
        guard regions.contains(where: { $0.id == id }) else { return false }
        defaults.set(id, forKey: key)
        NotificationCenter.default.post(name: Self.didChange, object: id)
        return true
    }
}
