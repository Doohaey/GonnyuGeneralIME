import Foundation
import CGannyuInput

public struct GonnyuAppleCandidate: Decodable {
    public let text: String
    public let annotation: String
    public let reading: String?
    public let mandarinReading: String?
    public let globalIndex: Int
    public let pageIndex: Int
    public let deletable: Bool

    private enum CodingKeys: String, CodingKey {
        case text, annotation, reading
        case mandarinReading
        case globalIndex
        case pageIndex
        case deletable
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        text = try container.decode(String.self, forKey: .text)
        annotation = try container.decodeIfPresent(String.self, forKey: .annotation) ?? ""
        reading = try container.decodeIfPresent(String.self, forKey: .reading)
        mandarinReading = try container.decodeIfPresent(String.self, forKey: .mandarinReading)
        globalIndex = try container.decodeIfPresent(Int.self, forKey: .globalIndex) ?? 0
        pageIndex = try container.decodeIfPresent(Int.self, forKey: .pageIndex) ?? globalIndex
        deletable = try container.decodeIfPresent(Bool.self, forKey: .deletable) ?? false
    }
}

public struct GonnyuAppleSnapshot: Decodable {
    public let handled: Bool
    public let commitText: String?
    public let rawInput: String
    public let preedit: String
    public let caret: Int
    public let candidates: [GonnyuAppleCandidate]
    public let highlightedIndex: Int?
    public let pageNumber: Int
    public let hasPreviousPage: Bool
    public let hasNextPage: Bool
    public let schemaID: String?
    public let asciiMode: Bool

    private enum CodingKeys: String, CodingKey {
        case handled, commitText, rawInput, preedit, caret, candidates, highlightedIndex
        case pageNumber, hasPreviousPage, hasNextPage, asciiMode
        case schemaID = "schemaId"
    }

    public static let empty = GonnyuAppleSnapshot(
        handled: false,
        commitText: nil,
        rawInput: "",
        preedit: "",
        caret: 0,
        candidates: [],
        highlightedIndex: nil,
        pageNumber: 0,
        hasPreviousPage: false,
        hasNextPage: false,
        schemaID: nil,
        asciiMode: false
    )
}

public enum GonnyuAppleKeyEvent: Encodable {
    case text(String)
    case backspace
    case space
    case enter

    private enum CodingKeys: String, CodingKey {
        case type, text
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .text(text):
            try container.encode("text", forKey: .type)
            try container.encode(text, forKey: .text)
        case .backspace:
            try container.encode("backspace", forKey: .type)
        case .space:
            try container.encode("space", forKey: .type)
        case .enter:
            try container.encode("enter", forKey: .type)
        }
    }
}

public struct GonnyuAppleRegion: Decodable, Equatable {
    public let id: String
    public let nameZh: String

    private enum CodingKeys: String, CodingKey {
        case id
        case nameZh = "name_zh"
    }
}

public enum GonnyuAppleEngineError: Error {
    case ffi(Int32, String)
}

public enum GonnyuAppleUserDataScope: Int32 {
    case words = 1
    case frequencies = 2
    case all = 3
}

public final class GonnyuAppleEngine {
    private var handle: OpaquePointer?

    public init(regionID: String, userDataDirectory: URL) throws {
        var created: OpaquePointer?
        let status = regionID.withCString { region in
            userDataDirectory.path.withCString { dataDirectory in
                gannyu_pipeline_create_with_user_data_dir(nil, region, dataDirectory, &created)
            }
        }
        guard status == gannyu_ffi_status_ok(), let created else {
            throw GonnyuAppleEngineError.ffi(status, Self.lastError())
        }
        handle = created
    }

    deinit {
        if let handle {
            gannyu_pipeline_destroy(handle)
        }
    }

    public static func regions() throws -> [GonnyuAppleRegion] {
        var output: UnsafeMutablePointer<CChar>?
        let status = gannyu_region_list(nil, &output)
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GonnyuAppleEngineError.ffi(status, lastError())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode([GonnyuAppleRegion].self, from: Data(String(cString: output).utf8))
    }

    public func snapshot() throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_snapshot(handle, out)
        }
    }

    public func process(_ event: GonnyuAppleKeyEvent) throws -> GonnyuAppleSnapshot {
        let payload = try String(data: JSONEncoder().encode(event), encoding: .utf8) ?? ""
        return try payload.withCString { eventJSON in
            try jsonCall { handle, out in
                gannyu_engine_process_key(handle, eventJSON, out)
            }
        }
    }

    public func selectCandidate(globalIndex: Int) throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_select_candidate(handle, globalIndex, out)
        }
    }

    public func clearComposition() throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_clear_composition(handle, out)
        }
    }

    public func clearUserData(_ scope: GonnyuAppleUserDataScope) throws {
        guard let handle else {
            throw GonnyuAppleEngineError.ffi(-1, "pipeline is unavailable")
        }
        let status = gannyu_pipeline_user_data_clear(handle, scope.rawValue)
        guard status == gannyu_ffi_status_ok() else {
            throw GonnyuAppleEngineError.ffi(status, Self.lastError())
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

    private func jsonCall<T: Decodable>(
        run: (OpaquePointer?, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32
    ) throws -> T {
        guard let handle else {
            throw GonnyuAppleEngineError.ffi(-1, "pipeline is unavailable")
        }
        var output: UnsafeMutablePointer<CChar>?
        let status = run(handle, &output)
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GonnyuAppleEngineError.ffi(status, Self.lastError())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode(T.self, from: Data(String(cString: output).utf8))
    }
}

public final class GonnyuAppleRegionStore {
    public static let didChange = Notification.Name("org.doohaey.gonnyu.apple.regionDidChange")
    private let key = "org.doohaey.gonnyu.region"
    private let defaults: UserDefaults
    public let userDataDirectory: URL

    public init(bundle: Bundle = .main) {
        guard let group = bundle.object(forInfoDictionaryKey: "GannyuAppGroupIdentifier") as? String,
              let defaults = UserDefaults(suiteName: group),
              let container = FileManager.default.containerURL(
                  forSecurityApplicationGroupIdentifier: group
              ) else {
            preconditionFailure("GannyuAppGroupIdentifier must resolve to an App Group")
        }
        self.defaults = defaults
        self.userDataDirectory = container
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("GonnyuInputMethod", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: userDataDirectory,
            withIntermediateDirectories: true
        )
    }

    public func currentID(in regions: [GonnyuAppleRegion]) -> String? {
        guard let first = regions.first else { return nil }
        let stored = defaults.string(forKey: key)
        let resolved = regions.contains(where: { $0.id == stored }) ? stored! : first.id
        if stored != resolved {
            defaults.set(resolved, forKey: key)
        }
        return resolved
    }

    public func select(_ id: String, in regions: [GonnyuAppleRegion]) -> Bool {
        guard regions.contains(where: { $0.id == id }) else { return false }
        defaults.set(id, forKey: key)
        NotificationCenter.default.post(name: Self.didChange, object: id)
        return true
    }
}
