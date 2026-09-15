import Foundation
import CryptoKit
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
    case resources(String)
}

private struct GonnyuAppleResourceFile: Decodable {
    let path: String
    let size: Int
    let sha256: String
}

private struct GonnyuAppleResourceManifest: Decodable {
    let schemaVersion: String
    let regions: [GonnyuAppleRegion]
    let files: [GonnyuAppleResourceFile]

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case regions, files
    }
}

private struct GonnyuAppleResourcePaths {
    let shared: URL
    let prebuilt: URL
    let regions: [GonnyuAppleRegion]
}

private final class GonnyuAppleResourceStore {
    private static let installLock = NSLock()
    private let bundle: Bundle
    private let fileManager = FileManager.default
    private let resourcesRoot: URL
    private let currentVersionFile: URL

    let userDataDirectory: URL

    init(bundle: Bundle = .main) throws {
        guard let group = bundle.object(forInfoDictionaryKey: "GannyuAppGroupIdentifier") as? String,
              let container = FileManager.default.containerURL(
                  forSecurityApplicationGroupIdentifier: group
              ) else {
            throw GonnyuAppleEngineError.resources("GannyuAppGroupIdentifier must resolve to an App Group")
        }
        self.bundle = bundle
        let applicationSupport = container
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("GonnyuInputMethod", isDirectory: true)
        let resources = applicationSupport.appendingPathComponent("Resources", isDirectory: true)
        resourcesRoot = resources
        currentVersionFile = resources.appendingPathComponent("current-version", isDirectory: false)
        userDataDirectory = applicationSupport.appendingPathComponent("UserData", isDirectory: true)
        try fileManager.createDirectory(at: resourcesRoot, withIntermediateDirectories: true)
        try fileManager.createDirectory(at: userDataDirectory, withIntermediateDirectories: true)
    }

    func prepare() throws -> [GonnyuAppleRegion] {
        Self.installLock.lock()
        defer { Self.installLock.unlock() }
        if let bundled = bundle.url(forResource: "rime", withExtension: nil) {
            try install(from: bundled)
        }
        return try currentPaths().regions
    }

    func currentPaths() throws -> GonnyuAppleResourcePaths {
        guard let versionData = fileManager.contents(atPath: currentVersionFile.path),
              let version = String(data: versionData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              isSafeComponent(version) else {
            throw GonnyuAppleEngineError.resources("Open the Gonnyu app once to install keyboard resources")
        }
        let root = resourcesRoot.appendingPathComponent(version, isDirectory: true)
        let manifest = try loadManifest(at: root)
        let shared = root.appendingPathComponent("shared", isDirectory: true)
        let prebuilt = root.appendingPathComponent("prebuilt", isDirectory: true)
        guard fileManager.fileExists(atPath: shared.path), fileManager.fileExists(atPath: prebuilt.path) else {
            throw GonnyuAppleEngineError.resources("Installed keyboard resources are incomplete")
        }
        return GonnyuAppleResourcePaths(shared: shared, prebuilt: prebuilt, regions: manifest.regions)
    }

    private func install(from source: URL) throws {
        let manifest = try loadManifest(at: source)
        guard isSafeComponent(manifest.schemaVersion) else {
            throw GonnyuAppleEngineError.resources("Invalid keyboard resource version")
        }
        let target = resourcesRoot.appendingPathComponent(manifest.schemaVersion, isDirectory: true)
        if fileManager.fileExists(atPath: target.path), try verify(root: target, manifest: manifest) {
            try writeCurrentVersion(manifest.schemaVersion)
            try removeObsoleteResources(keeping: target)
            return
        }

        let staging = resourcesRoot.appendingPathComponent(
            ".\(manifest.schemaVersion).staging-\(UUID().uuidString)",
            isDirectory: true
        )
        try? fileManager.removeItem(at: staging)
        try fileManager.createDirectory(at: staging, withIntermediateDirectories: true)
        do {
            for item in manifest.files {
                let relative = try safeRelativePath(item.path)
                let input = source.appendingPathComponent(relative, isDirectory: false)
                let output = staging.appendingPathComponent(relative, isDirectory: false)
                try fileManager.createDirectory(
                    at: output.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                try fileManager.copyItem(at: input, to: output)
            }
            try fileManager.copyItem(
                at: source.appendingPathComponent("resource-manifest.json", isDirectory: false),
                to: staging.appendingPathComponent("resource-manifest.json", isDirectory: false)
            )
            guard try verify(root: staging, manifest: manifest) else {
                throw GonnyuAppleEngineError.resources("Keyboard resource verification failed")
            }
            try? fileManager.removeItem(at: target)
            try fileManager.moveItem(at: staging, to: target)
            try writeCurrentVersion(manifest.schemaVersion)
            try removeObsoleteResources(keeping: target)
        } catch {
            try? fileManager.removeItem(at: staging)
            throw error
        }
    }

    private func loadManifest(at root: URL) throws -> GonnyuAppleResourceManifest {
        let url = root.appendingPathComponent("resource-manifest.json", isDirectory: false)
        return try JSONDecoder().decode(GonnyuAppleResourceManifest.self, from: Data(contentsOf: url))
    }

    private func verify(root: URL, manifest: GonnyuAppleResourceManifest) throws -> Bool {
        for item in manifest.files {
            let relative = try safeRelativePath(item.path)
            let file = root.appendingPathComponent(relative, isDirectory: false)
            let values = try file.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
            guard values.isRegularFile == true, values.fileSize == item.size else { return false }
            let digest = try sha256(of: file)
            guard digest == item.sha256 else { return false }
        }
        return true
    }

    private func sha256(of file: URL) throws -> String {
        let input = try FileHandle(forReadingFrom: file)
        defer { try? input.close() }
        var hasher = SHA256()
        while let chunk = try input.read(upToCount: 1024 * 1024), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    private func writeCurrentVersion(_ version: String) throws {
        try Data((version + "\n").utf8).write(to: currentVersionFile, options: .atomic)
    }

    private func removeObsoleteResources(keeping target: URL) throws {
        for item in try fileManager.contentsOfDirectory(
            at: resourcesRoot,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: []
        ) where item != target && item != currentVersionFile {
            try fileManager.removeItem(at: item)
        }
    }

    private func isSafeComponent(_ value: String) -> Bool {
        !value.isEmpty && value != "." && value != ".." && !value.contains("/") && !value.contains("\\")
    }

    private func safeRelativePath(_ value: String) throws -> String {
        let parts = value.split(separator: "/", omittingEmptySubsequences: false)
        guard !value.hasPrefix("/"), !parts.isEmpty,
              parts.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else {
            throw GonnyuAppleEngineError.resources("Invalid keyboard resource path")
        }
        return value
    }
}

public final class GonnyuAppleEngine {
    private var handle: OpaquePointer?

    public init(regionID: String, userDataDirectory: URL) throws {
        let resourceStore = try GonnyuAppleResourceStore()
        let resources = try resourceStore.currentPaths()
        var created: OpaquePointer?
        let status = regionID.withCString { region in
            resources.shared.path.withCString { sharedDirectory in
                resources.prebuilt.path.withCString { prebuiltDirectory in
                    userDataDirectory.path.withCString { dataDirectory in
                        var config = GannyuEngineConfig(
                            struct_size: MemoryLayout<GannyuEngineConfig>.size,
                            region_id: region,
                            shared_data_dir: sharedDirectory,
                            prebuilt_data_dir: prebuiltDirectory,
                            user_data_dir: dataDirectory
                        )
                        return gannyu_engine_create(&config, &created)
                    }
                }
            }
        }
        guard status == gannyu_ffi_status_ok(), let created else {
            throw GonnyuAppleEngineError.ffi(status, Self.lastError())
        }
        guard gannyu_engine_set_candidate_limit(created, 60) == gannyu_ffi_status_ok() else {
            throw GonnyuAppleEngineError.ffi(gannyu_ffi_status_ok(), Self.lastError())
        }
        handle = created
    }

    deinit {
        if let handle {
            gannyu_pipeline_destroy(handle)
        }
    }

    public static func regions() throws -> [GonnyuAppleRegion] {
        try GonnyuAppleResourceStore().prepare()
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

    public func changeCandidatePage(direction: Int) throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_change_page(handle, Int32(direction), out)
        }
    }

    public func clearComposition() throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_clear_composition(handle, out)
        }
    }

    public func clearUserData() throws -> GonnyuAppleSnapshot {
        try jsonCall { handle, out in
            gannyu_engine_reset_user_data(handle, Int32(GANNYU_USER_DATA_ALL), out)
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
    private let resetRequestsDirectory: URL
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
        let applicationSupport = container
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("GonnyuInputMethod", isDirectory: true)
        self.userDataDirectory = applicationSupport.appendingPathComponent("UserData", isDirectory: true)
        self.resetRequestsDirectory = applicationSupport.appendingPathComponent(
            "PendingUserDataResets",
            isDirectory: true
        )
        try? FileManager.default.createDirectory(
            at: userDataDirectory,
            withIntermediateDirectories: true
        )
        try? FileManager.default.createDirectory(
            at: resetRequestsDirectory,
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

    public func requestUserDataReset(regionIDs: [String], in regions: [GonnyuAppleRegion]) throws {
        let available = Set(regions.map(\.id))
        let requested = Array(Set(regionIDs)).sorted()
        guard !requested.isEmpty, requested.allSatisfy(available.contains) else {
            throw GonnyuAppleEngineError.resources("Invalid user data reset request")
        }
        let request = resetRequestsDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
            .appendingPathExtension("json")
        try JSONEncoder().encode(requested).write(to: request, options: .atomic)
    }

    public func pendingUserDataResetRegionIDs() -> [String] {
        let files = (try? FileManager.default.contentsOfDirectory(
            at: resetRequestsDirectory,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        )) ?? []
        var ids = Set<String>()
        for file in files where file.pathExtension == "json" {
            guard let data = try? Data(contentsOf: file),
                  let request = try? JSONDecoder().decode([String].self, from: data) else { continue }
            ids.formUnion(request)
        }
        return ids.sorted()
    }

    public func finishPendingUserDataResets() throws {
        for file in try FileManager.default.contentsOfDirectory(
            at: resetRequestsDirectory,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) where file.pathExtension == "json" {
            try FileManager.default.removeItem(at: file)
        }
    }
}
