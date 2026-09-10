import Foundation
import GannyuMacOSSupport

struct SmokeConfig {
    var manifest: String?
    var region: String?
    var retrieveInput = "gau"
    var composeInput = "吹牛"
}

func parseArgs() throws -> SmokeConfig {
    var config = SmokeConfig()
    let args = Array(CommandLine.arguments.dropFirst())
    var index = 0
    while index < args.count {
        switch args[index] {
        case "--manifest":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --manifest") }
            config.manifest = args[index]
        case "--region":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --region") }
            config.region = args[index]
        case "--retrieve":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --retrieve") }
            config.retrieveInput = args[index]
        case "--compose":
            index += 1
            guard index < args.count else { throw SmokeArgumentError("missing value for --compose") }
            config.composeInput = args[index]
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
    let engine = try GannyuEngine(manifestPath: config.manifest, regionID: config.region)
    print("entries=\(engine.entryCount())")
    print("retrieve \(config.retrieveInput)")
    print(try engine.retrieve(config.retrieveInput))
    print("compose \(config.composeInput)")
    print(try engine.compose(config.composeInput))
} catch {
    fputs("smoke failed: \(error)\n", stderr)
    exit(1)
}
