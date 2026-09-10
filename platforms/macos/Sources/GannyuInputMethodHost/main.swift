import AppKit
import Carbon
import Foundation
import InputMethodKit
import GannyuMacOSSupport

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var server: IMKServer?
    private var engine: GannyuEngine?

    func startup() throws {
        let env = ProcessInfo.processInfo.environment
        if env["GANNYU_REGISTER_INPUT_SOURCE"] == "1" {
            let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
            guard status == noErr else {
                throw NSError(
                    domain: "org.doohaey.GonnyuInputMethod",
                    code: Int(status),
                    userInfo: [NSLocalizedDescriptionKey: "TISRegisterInputSource failed: \(status)"]
                )
            }
            print("registered input source: \(Bundle.main.bundleURL.path)")
        }
        let manifest = env["GANNYU_MANIFEST"]
        let region = env["GANNYU_REGION_ID"]
        let bundleID = env["GANNYU_IMK_BUNDLE_ID"] ?? "org.doohaey.inputmethod.gonnyu.native"
        let connection = env["GANNYU_IMK_CONNECTION"] ?? "\(bundleID)_Connection"

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
        installMenu()
    }

    private func installMenu() {
        let menu = NSMenu(title: "Gonnyu")
        let regionMenu = NSMenu(title: "地区")
        for region in GannyuRegion.allCases {
            let item = NSMenuItem(title: region.label, action: #selector(selectRegion(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = region.rawValue
            item.state = region == GannyuRegionStore.shared.current ? .on : .off
            regionMenu.addItem(item)
        }
        let regionItem = NSMenuItem(title: "地区", action: nil, keyEquivalent: "")
        regionItem.submenu = regionMenu
        menu.addItem(regionItem)
        menu.addItem(.separator())
        menu.addItem(withTitle: "Gonnyu 输入法", action: nil, keyEquivalent: "")
        NSApp.mainMenu = menu
    }

    @objc private func selectRegion(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let region = GannyuRegion(rawValue: raw) else { return }
        GannyuRegionStore.shared.current = region
        installMenu()
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
