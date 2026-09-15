// swift-tools-version: 5.8
import PackageDescription

let package = Package(
    name: "GannyuMacOS",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .executable(name: "GannyuInputMethodHost", targets: ["GannyuInputMethodHost"]),
        .executable(name: "GannyuMacOSSmoke", targets: ["GannyuMacOSSmoke"]),
    ],
    targets: [
        .systemLibrary(
            name: "CGannyuInput",
            path: "Sources/CGannyuInput"
        ),
        .target(
            name: "GannyuMacOSSupport",
            dependencies: ["CGannyuInput"],
            path: "Sources/GannyuMacOSSupport",
            linkerSettings: [.unsafeFlags([
                "-L", "../../build/rime-macos/adapter",
                "-L", "../../build/rime-macos/prefix/lib",
                "-Xlinker", "-force_load", "-Xlinker", "../../build/rime-macos/adapter/libgannyu_rime_engine.a",
                "-Xlinker", "-force_load", "-Xlinker", "../../build/rime-macos/prefix/lib/librime.a",
                "-lleveldb", "-lmarisa", "-lopencc", "-lyaml-cpp", "-lglog", "-lboost_regex", "-lc++"
            ])]
        ),
        .executableTarget(
            name: "GannyuInputMethodHost",
            dependencies: ["GannyuMacOSSupport"],
            path: "Sources/GannyuInputMethodHost"
        ),
        .executableTarget(
            name: "GannyuMacOSSmoke",
            dependencies: ["GannyuMacOSSupport"],
            path: "Sources/GannyuMacOSSmoke"
        ),
    ]
)
