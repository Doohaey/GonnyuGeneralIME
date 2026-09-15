import Foundation
import GannyuMacOSSupport

struct SmokeConfig {
    var region: String?
    var input = "gau"
}

func parseArgs() throws -> SmokeConfig {
    var config = SmokeConfig()
    let args = Array(CommandLine.arguments.dropFirst())
    var index = 0
    while index < args.count {
        switch args[index] {
        case "--region":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --region") }
            config.region = args[index]
        case "--input":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --input") }
            config.input = args[index]
        default:
            throw SmokeArgumentError("unknown argument: \(args[index])")
        }
        index += 1
    }
    return config
}

struct SmokeArgumentError: Error, CustomStringConvertible {
    let description: String

    init(_ description: String) {
        self.description = description
    }
}

do {
    let config = try parseArgs()
    let engine = try GannyuEngine(regionID: config.region)
    print("schema=\(try engine.snapshot().schemaId ?? "")")
    for character in config.input {
        let result = try engine.process(.text(String(character)))
        print("input=\(result.rawInput) page=\(result.pageNumber) candidates=\(result.candidates.count)")
    }
    let result = try engine.snapshot()
    print("first=\(result.candidates.first?.text ?? "")")
} catch {
    fputs("smoke failed: \(error)\n", stderr)
    exit(1)
}
