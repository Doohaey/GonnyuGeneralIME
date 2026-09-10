import AppKit
import Foundation
import InputMethodKit
import GannyuMacOSSupport

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var server: IMKServer?
    private var engine: GannyuEngine?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let env = ProcessInfo.processInfo.environment
        let manifest = env["GANNYU_MANIFEST"]
        let region = env["GANNYU_REGION_ID"]
        let connection = env["GANNYU_IMK_CONNECTION"] ?? "org.doohaey.gannyu.inputmethod.connection"
        let bundleID = env["GANNYU_IMK_BUNDLE_ID"] ?? "org.doohaey.GannyuInputMethod"

        do {
            engine = try GannyuEngine(manifestPath: manifest, regionID: region)
            server = IMKServer(name: connection, bundleIdentifier: bundleID)
            let count = engine?.entryCount() ?? -1
            print("GannyuInputMethodHost ready")
            print("manifest=\(manifest ?? "(embedded)")")
            print("region=\(region ?? "(default)")")
            print("entries=\(count)")
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
app.run()
