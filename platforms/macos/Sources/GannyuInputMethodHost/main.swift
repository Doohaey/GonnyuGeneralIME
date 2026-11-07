import AppKit
import Carbon.HIToolbox
import Foundation
import InputMethodKit
import GannyuMacOSSupport

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var server: IMKServer?
    private var candidateWindow: IMKCandidates?
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
        let candidateWindow = IMKCandidates(
            server: server,
            panelType: kIMKSingleColumnScrollingCandidatePanel,
            styleType: kIMKMain
        )
        candidateWindow?.setAttributes([
            IMKCandidatesOpacityAttributeName: NSNumber(value: 0),
            IMKCandidatesSendServerKeyEventFirst: NSNumber(value: true),
        ])
        candidateWindow?.setDismissesAutomatically(false)
        self.candidateWindow = candidateWindow
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
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(inputOptionsDidChange),
            name: GannyuRegionStore.optionsDidChange,
            object: nil
        )
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
        let settingsEntry = NSMenuItem(title: "输入设置", action: nil, keyEquivalent: "")
        settingsEntry.submenu = inputSettingsMenu()
        menu.addItem(settingsEntry)
        let currentLabel = regions.first(where: { $0.id == currentID })?.nameZh ?? "未加载"
        item.button?.title = "赣·\(currentLabel)"
        item.menu = menu
        statusItem = item
    }

    private func inputSettingsMenu() -> NSMenu {
        let settings = NSMenu(title: "输入设置")
        settings.autoenablesItems = false
        let values = GannyuRegionStore.shared.inputOptions
        for index in [0, 1, 6, 2, 3, 7, 4] {
            let item = NSMenuItem(
                title: GannyuRegionStore.optionTitles[index],
                action: #selector(selectInputOption(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.tag = index
            item.indentationLevel = [3, 7, 4].contains(index) ? 1 : 0
            item.state = values[index] != 0 ? .on : .off
            if values[8] != 0 {
                item.isEnabled = ![2, 4].contains(index)
            } else {
                item.isEnabled = [3, 7, 4].contains(index) ? values[2] != 0 : true
            }
            settings.addItem(item)
        }
        let currentRegion = (try? GannyuEngine.availableRegions())?.first { $0.id == GannyuRegionStore.shared.currentID() }
        let schemeAvailable = currentRegion?.transcriptionSchemes?.contains("ipa") ?? false
        let schemeEntry = NSMenuItem(title: "方案", action: nil, keyEquivalent: "")
        let schemeMenu = NSMenu(title: "方案")
        schemeMenu.autoenablesItems = false
        let firstScheme = NSMenuItem(title: "一式（汉拼式）", action: #selector(selectInputOption(_:)), keyEquivalent: "")
        firstScheme.target = self
        firstScheme.tag = 20
        firstScheme.state = values[8] == 0 ? .on : .off
        schemeMenu.addItem(firstScheme)
        let secondScheme = NSMenuItem(title: "二式（组合式）", action: #selector(selectInputOption(_:)), keyEquivalent: "")
        secondScheme.target = self
        secondScheme.tag = 21
        secondScheme.state = (schemeAvailable && values[8] != 0) ? .on : .off
        secondScheme.isEnabled = schemeAvailable
        schemeMenu.addItem(secondScheme)
        schemeEntry.submenu = schemeMenu
        settings.addItem(schemeEntry)
        settings.addItem(.separator())
        for (index, title) in ["新老兼有", "新派", "老派"].enumerated() {
            let item = NSMenuItem(title: title, action: #selector(selectInputOption(_:)), keyEquivalent: "")
            item.target = self
            item.tag = 10 + index
            item.state = values[5] == Int32(index) ? .on : .off
            settings.addItem(item)
        }
        return settings
    }

    @objc private func selectInputOption(_ item: NSMenuItem) {
        let values = GannyuRegionStore.shared.inputOptions
        if item.tag == 20 || item.tag == 21 {
            GannyuRegionStore.shared.setInputOption(8, value: item.tag == 21 ? 1 : 0)
            return
        }
        let index = item.tag < 10 ? item.tag : 5
        let value = item.tag < 10 ? (values[index] == 0 ? 1 : 0) : Int32(item.tag - 10)
        GannyuRegionStore.shared.setInputOption(index, value: value)
    }

    @objc private func inputOptionsDidChange() {
        do {
            try engine?.setInputOptions(GannyuRegionStore.shared.inputOptions)
        } catch {
            NSLog("[GonnyuInputMethodHost] input options failed: %@", String(describing: error))
        }
        installStatusItem()
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
