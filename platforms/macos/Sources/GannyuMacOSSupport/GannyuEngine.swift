import Foundation
import CGannyuInput

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

    public func compose(_ input: String) throws -> String {
        try jsonCall(input) { handle, text, out in
            gannyu_pipeline_compose(handle, text, out)
        }
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
