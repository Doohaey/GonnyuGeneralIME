import AppKit
import Carbon
import Foundation
import InputMethodKit
import GannyuMacOSSupport

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var server: IMKServer?
    private var engine: GannyuEngine?
    private var statusItem: NSStatusItem?

    func startup() throws {
        let env = ProcessInfo.processInfo.environment
        let bundleID = env["GANNYU_IMK_BUNDLE_ID"]
            ?? Bundle.main.bundleIdentifier
            ?? "org.doohaey.inputmethod.gonnyu.native"
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
        let region = env["GANNYU_REGION_ID"] ?? GannyuRegionStore.shared.currentID(manifestPath: manifest)
        let connection = env["GANNYU_IMK_CONNECTION"]
            ?? Bundle.main.object(forInfoDictionaryKey: "InputMethodConnectionName") as? String
            ?? "\(bundleID)_Connection"

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
        installStatusItem()
    }

    private func installStatusItem() {
        let item = statusItem ?? NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let menu = NSMenu(title: "Gonnyu 地区")
        let manifest = ProcessInfo.processInfo.environment["GANNYU_MANIFEST"]
        let regions = (try? GannyuEngine.availableRegions(manifestPath: manifest)) ?? []
        let currentID = GannyuRegionStore.shared.currentID(manifestPath: manifest)
        for region in regions {
            let item = NSMenuItem(title: region.nameZh, action: #selector(selectRegion(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = region.id
            item.state = region.id == currentID ? .on : .off
            menu.addItem(item)
        }
        menu.addItem(.separator())
        menu.addItem(withTitle: "Gonnyu 输入法", action: nil, keyEquivalent: "")
        let currentLabel = regions.first(where: { $0.id == currentID })?.nameZh ?? "未加载"
        item.button?.title = "赣·\(currentLabel)"
        item.menu = menu
        statusItem = item
    }

    @objc private func selectRegion(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String else { return }
        _ = GannyuRegionStore.shared.select(
            raw,
            manifestPath: ProcessInfo.processInfo.environment["GANNYU_MANIFEST"]
        )
        installStatusItem()
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
