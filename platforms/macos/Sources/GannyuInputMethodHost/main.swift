import AppKit
import Foundation
import InputMethodKit
import GannyuMacOSSupport

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var server: IMKServer?
    private var engine: GannyuEngine?

    func startup() throws {
        let env = ProcessInfo.processInfo.environment
        let manifest = env["GANNYU_MANIFEST"]
        let region = env["GANNYU_REGION_ID"]
        let connection = env["GANNYU_IMK_CONNECTION"] ?? "org.doohaey.gannyu.inputmethod.connection"
        let bundleID = env["GANNYU_IMK_BUNDLE_ID"] ?? "org.doohaey.GannyuInputMethod"

        engine = try GannyuEngine(manifestPath: manifest, regionID: region)
        server = IMKServer(name: connection, bundleIdentifier: bundleID)
        let count = engine?.entryCount() ?? -1
        print("GannyuInputMethodHost ready")
        print("manifest=\(manifest ?? "(embedded)")")
        print("region=\(region ?? "(default)")")
        print("entries=\(count)")
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            try startup()
        } catch {
            fputs("failed to start host: \(error)\n", stderr)
            NSApp.terminate(nil)
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
if ProcessInfo.processInfo.environment["GANNYU_IMK_SELFTEST"] == "1" {
    do {
        try delegate.startup()
        exit(0)
    } catch {
        fputs("failed to start host: \(error)\n", stderr)
        exit(1)
    }
}
app.run()
