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
            path: "Sources/GannyuMacOSSupport"
        ),
        .executableTarget(
            name: "GannyuInputMethodHost",
            dependencies: ["GannyuMacOSSupport"],
            path: "Sources/GannyuInputMethodHost",
            linkerSettings: [
                .unsafeFlags(["-L", "../../target/release"]),
            ]
        ),
        .executableTarget(
            name: "GannyuMacOSSmoke",
            dependencies: ["GannyuMacOSSupport"],
            path: "Sources/GannyuMacOSSmoke",
            linkerSettings: [
                .unsafeFlags(["-L", "../../target/release"]),
            ]
        ),
    ]
)
