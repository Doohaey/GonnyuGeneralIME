import AppKit
import Carbon.HIToolbox
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
        let region = env["GANNYU_REGION_ID"] ?? GannyuRegionStore.shared.currentID()
        let connection = env["GANNYU_IMK_CONNECTION"]
            ?? Bundle.main.object(forInfoDictionaryKey: "InputMethodConnectionName") as? String
            ?? "\(bundleID)_Connection"

        guard NSClassFromString("GannyuInputController") != nil else {
            throw NSError(
                domain: "org.doohaey.GonnyuInputMethod",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "GannyuInputController is not available to InputMethodKit"]
            )
        }
        guard let server = IMKServer(name: connection, bundleIdentifier: bundleID) else {
            throw NSError(
                domain: "org.doohaey.GonnyuInputMethod",
                code: 3,
                userInfo: [NSLocalizedDescriptionKey: "IMKServer could not register connection: \(connection)"]
            )
        }

        engine = try GannyuEngine(regionID: region)
        self.server = server
        if UserDefaults.standard.bool(forKey: "GannyuIMKDiagnostics") {
            NSLog("[GonnyuIMK] server-ready")
        }
        print("GannyuInputMethodHost ready")
        print("region=\(region ?? "(default)")")
        print("schema=\(try engine?.snapshot().schemaId ?? "(unavailable)")")
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            try startup()
        } catch {
            fputs("failed to start host: \(error)\n", stderr)
            NSApp.terminate(nil)
        }
        installStatusItem()
        DistributedNotificationCenter.default().addObserver(
            self,
            selector: #selector(inputSourceDidChange),
            name: NSNotification.Name("com.apple.Carbon.TISNotifySelectedKeyboardInputSourceChanged"),
            object: nil
        )
    }

    @objc private func inputSourceDidChange() {
        guard let bundleID = Bundle.main.bundleIdentifier else { return }
        let props = [kTISPropertyInputSourceIsSelected as String: true] as CFDictionary
        let isActive: Bool
        if let cfList = TISCreateInputSourceList(props, false)?.takeRetainedValue() {
            let count = CFArrayGetCount(cfList)
            isActive = (0..<count).contains { i in
                guard let rawPtr = CFArrayGetValueAtIndex(cfList, i) else { return false }
                let src = Unmanaged<TISInputSource>.fromOpaque(rawPtr).takeUnretainedValue()
                guard let bidPtr = TISGetInputSourceProperty(src, kTISPropertyBundleID) else { return false }
                return (Unmanaged<CFString>.fromOpaque(bidPtr).takeUnretainedValue() as String) == bundleID
            }
        } else {
            isActive = false
        }
        statusItem?.isVisible = isActive
    }

    private func installStatusItem() {
        let item = statusItem ?? NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let menu = NSMenu(title: "Gonnyu 地区")
        let regions = (try? GannyuEngine.availableRegions()) ?? []
        let currentID = GannyuRegionStore.shared.currentID()
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
        _ = GannyuRegionStore.shared.select(raw)
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
