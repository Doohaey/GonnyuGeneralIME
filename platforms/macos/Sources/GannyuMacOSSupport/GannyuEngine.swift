import Foundation
import CryptoKit
import CGannyuInput

public struct GannyuRegion: Decodable, Equatable {
    public let id: String
    public let nameZh: String

    private enum CodingKeys: String, CodingKey {
        case id
        case nameZh = "name_zh"
    }
}

public struct GannyuCandidate: Decodable {
    public let text: String
    public let annotation: String
    public let globalIndex: Int
    public let pageIndex: Int

    private enum CodingKeys: String, CodingKey {
        case text, annotation, globalIndex, pageIndex
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        text = try values.decode(String.self, forKey: .text)
        annotation = try values.decodeIfPresent(String.self, forKey: .annotation) ?? ""
        globalIndex = try values.decode(Int.self, forKey: .globalIndex)
        pageIndex = try values.decode(Int.self, forKey: .pageIndex)
    }
}

public struct GannyuSnapshot: Decodable {
    public let handled: Bool
    public let commitText: String?
    public let rawInput: String
    public let preedit: String
    public let caret: Int
    public let candidates: [GannyuCandidate]
    public let highlightedIndex: Int?
    public let pageNumber: Int
    public let hasPreviousPage: Bool
    public let hasNextPage: Bool
    public let schemaId: String?
    public let asciiMode: Bool
}

public enum GannyuKeyEvent: Encodable {
    case text(String)
    case backspace
    case space
    case enter

    private enum CodingKeys: String, CodingKey { case type, text }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .text(let text):
            try values.encode("text", forKey: .type)
            try values.encode(text, forKey: .text)
        case .backspace:
            try values.encode("backspace", forKey: .type)
        case .space:
            try values.encode("space", forKey: .type)
        case .enter:
            try values.encode("enter", forKey: .type)
        }
    }
}

public enum GannyuEngineError: Error, CustomStringConvertible {
    case status(Int32, String)
    case resources(String)

    public var description: String {
        switch self {
        case .status(let code, let message):
            return "librime status \(code): \(message)"
        case .resources(let message):
            return message
        }
    }
}

private struct GannyuResourceFile: Decodable {
    let path: String
    let size: Int
    let sha256: String
}

private struct GannyuResourceManifest: Decodable {
    let regions: [GannyuRegion]
    let files: [GannyuResourceFile]
}

public final class GannyuRegionStore {
    public static let shared = GannyuRegionStore()
    public static let didChange = Notification.Name("org.doohaey.gonnyu.regionDidChange")
    private let key = "org.doohaey.gonnyu.region"

    public func currentID() -> String? {
        guard let regions = try? GannyuEngine.availableRegions(),
              let first = regions.first else { return nil }
        let stored = UserDefaults.standard.string(forKey: key)
        let resolved = regions.contains(where: { $0.id == stored }) ? stored! : first.id
        if stored != resolved { UserDefaults.standard.set(resolved, forKey: key) }
        return resolved
    }

    public func select(_ regionID: String) -> Bool {
        guard let regions = try? GannyuEngine.availableRegions(),
              regions.contains(where: { $0.id == regionID }) else { return false }
        UserDefaults.standard.set(regionID, forKey: key)
        NotificationCenter.default.post(name: Self.didChange, object: regionID)
        return true
    }
}

public final class GannyuEngine {
    private static let verificationLock = NSLock()
    private static var verifiedRoot: String?
    private var handle: OpaquePointer?

    public init(regionID: String? = nil) throws {
        let resources = try Self.resourceRoot()
        let userData = try Self.userDataDirectory()
        let available = try Self.availableRegions()
        let region = regionID ?? GannyuRegionStore.shared.currentID() ?? available.first?.id
        guard let region else { throw GannyuEngineError.resources("no Rime region") }
        var created: OpaquePointer?
        let status = region.withCString { regionPointer in
            resources.appendingPathComponent("shared").path.withCString { sharedPointer in
                resources.appendingPathComponent("prebuilt").path.withCString { prebuiltPointer in
                    userData.path.withCString { userPointer in
                        var config = GannyuEngineConfig(
                            struct_size: MemoryLayout<GannyuEngineConfig>.size,
                            region_id: regionPointer,
                            shared_data_dir: sharedPointer,
                            prebuilt_data_dir: prebuiltPointer,
                            user_data_dir: userPointer
                        )
                        return gannyu_engine_create(&config, &created)
                    }
                }
            }
        }
        guard status == gannyu_ffi_status_ok(), let created else {
            throw GannyuEngineError.status(status, Self.lastError())
        }
        handle = created
    }

    deinit { if let handle { gannyu_pipeline_destroy(handle) } }

    public static func availableRegions() throws -> [GannyuRegion] {
        try validatedManifest().regions
    }

    public func snapshot() throws -> GannyuSnapshot {
        try jsonCall { gannyu_engine_snapshot($0, $1) }
    }

    public func process(_ event: GannyuKeyEvent) throws -> GannyuSnapshot {
        let data = try JSONEncoder().encode(event)
        return try data.withUnsafeBytes { bytes in
            let json = String(decoding: bytes, as: UTF8.self)
            return try json.withCString { pointer in
                try jsonCall { gannyu_engine_process_key($0, pointer, $1) }
            }
        }
    }

    public func selectCandidate(globalIndex: Int) throws -> GannyuSnapshot {
        try jsonCall { gannyu_engine_select_candidate($0, globalIndex, $1) }
    }

    public func changePage(direction: Int) throws -> GannyuSnapshot {
        try jsonCall { gannyu_engine_change_page($0, Int32(direction), $1) }
    }

    public func clearComposition() throws -> GannyuSnapshot {
        try jsonCall { gannyu_engine_clear_composition($0, $1) }
    }

    public func switchRegion(_ regionID: String) throws -> GannyuSnapshot {
        try regionID.withCString { pointer in
            try jsonCall { gannyu_engine_switch_region($0, pointer, $1) }
        }
    }

    private func jsonCall<T: Decodable>(
        _ run: (OpaquePointer?, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32
    ) throws -> T {
        guard let handle else { throw GannyuEngineError.resources("engine unavailable") }
        var output: UnsafeMutablePointer<CChar>?
        let status = run(handle, &output)
        guard status == gannyu_ffi_status_ok(), let output else {
            throw GannyuEngineError.status(status, Self.lastError())
        }
        defer { gannyu_string_destroy(output) }
        return try JSONDecoder().decode(T.self, from: Data(String(cString: output).utf8))
    }

    private static func resourceRoot() throws -> URL {
        let root: URL
        if let override = ProcessInfo.processInfo.environment["GANNYU_RIME_RESOURCE_ROOT"] {
            root = URL(fileURLWithPath: override, isDirectory: true)
        } else if let bundled = Bundle.main.resourceURL {
            root = bundled.appendingPathComponent("rime", isDirectory: true)
        } else {
            throw GannyuEngineError.resources("missing bundle resources")
        }
        _ = try validatedManifest(at: root)
        return root
    }

    private static func validatedManifest() throws -> GannyuResourceManifest {
        let root: URL
        if let override = ProcessInfo.processInfo.environment["GANNYU_RIME_RESOURCE_ROOT"] {
            root = URL(fileURLWithPath: override, isDirectory: true)
        } else if let bundled = Bundle.main.resourceURL {
            root = bundled.appendingPathComponent("rime", isDirectory: true)
        } else {
            throw GannyuEngineError.resources("missing bundle resources")
        }
        return try validatedManifest(at: root)
    }

    private static func validatedManifest(at root: URL) throws -> GannyuResourceManifest {
        verificationLock.lock()
        defer { verificationLock.unlock() }
        let manifest = try JSONDecoder().decode(
            GannyuResourceManifest.self,
            from: Data(contentsOf: root.appendingPathComponent("resource-manifest.json"))
        )
        guard !manifest.regions.isEmpty, !manifest.files.isEmpty,
              FileManager.default.fileExists(atPath: root.appendingPathComponent("shared").path),
              FileManager.default.fileExists(atPath: root.appendingPathComponent("prebuilt").path) else {
            throw GannyuEngineError.resources("incomplete Rime resources")
        }
        if verifiedRoot == root.path { return manifest }
        for entry in manifest.files {
            let components = entry.path.split(separator: "/", omittingEmptySubsequences: false)
            guard !entry.path.hasPrefix("/"),
                  components.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else {
                throw GannyuEngineError.resources("invalid Rime resource path")
            }
            let file = root.appendingPathComponent(entry.path)
            let attributes = try FileManager.default.attributesOfItem(atPath: file.path)
            guard (attributes[.type] as? FileAttributeType) == .typeRegular,
                  (attributes[.size] as? NSNumber)?.intValue == entry.size else {
                throw GannyuEngineError.resources("Rime resource size mismatch: \(entry.path)")
            }
            let input = try FileHandle(forReadingFrom: file)
            defer { try? input.close() }
            var hasher = SHA256()
            while let chunk = try input.read(upToCount: 1024 * 1024), !chunk.isEmpty {
                hasher.update(data: chunk)
            }
            let digest = hasher.finalize().map { String(format: "%02x", $0) }.joined()
            guard digest == entry.sha256 else {
                throw GannyuEngineError.resources("Rime resource checksum mismatch: \(entry.path)")
            }
        }
        verifiedRoot = root.path
        return manifest
    }

    private static func userDataDirectory() throws -> URL {
        if let override = ProcessInfo.processInfo.environment["GANNYU_RIME_USER_DATA_DIR"] {
            let directory = URL(fileURLWithPath: override, isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            return directory
        }
        let directory = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        ).appendingPathComponent("GonnyuInputMethod/UserData", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private static func lastError() -> String {
        var output: UnsafeMutablePointer<CChar>?
        guard gannyu_last_error(&output) == gannyu_ffi_status_ok(), let output else {
            return "unknown librime error"
        }
        defer { gannyu_string_destroy(output) }
        return String(cString: output)
    }
}
