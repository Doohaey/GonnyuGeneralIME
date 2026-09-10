import AppKit
import InputMethodKit
import GannyuMacOSSupport

@objc(GannyuInputController)
final class GannyuInputController: IMKInputController {
    private lazy var engine: GannyuEngine? = {
        let env = ProcessInfo.processInfo.environment
        return try? GannyuEngine(
            manifestPath: env["GANNYU_MANIFEST"],
            regionID: env["GANNYU_REGION_ID"] ?? GannyuRegionStore.shared.current.rawValue
        )
    }()
    private var buffer = ""
    private var cachedText = ""
    private var currentCandidates: [GannyuRetrievedCandidate] = []

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        NotificationCenter.default.addObserver(self, selector: #selector(regionDidChange), name: GannyuRegionStore.didChange, object: nil)
    }

    deinit { NotificationCenter.default.removeObserver(self) }

    @objc(inputText:client:)
    override func inputText(_ string: String!, client sender: Any!) -> Bool {
        guard let string else {
            return false
        }
        if string.isEmpty {
            return false
        }
        if string == " " || string == "\r" || string == "\n" {
            guard !buffer.isEmpty else {
                return false
            }
            commitCandidate(at: 0, client: sender)
            return true
        }
        if let index = candidateIndex(for: string) {
            guard !buffer.isEmpty, index < currentCandidates.count else {
                return false
            }
            commitCandidate(at: index, client: sender)
            return true
        }
        guard shouldAppend(string) else {
            return false
        }
        buffer.append(contentsOf: string.lowercased())
        refreshCandidates(client: sender)
        return true
    }

    @objc(didCommandBySelector:client:)
    override func didCommand(by aSelector: Selector!, client sender: Any!) -> Bool {
        guard let aSelector else {
            return false
        }
        if aSelector == #selector(NSResponder.deleteBackward(_:)) {
            guard !buffer.isEmpty else {
                return false
            }
            buffer.removeLast()
            refreshCandidates(client: sender)
            return true
        }
        if aSelector == #selector(NSResponder.cancelOperation(_:)) {
            guard !buffer.isEmpty else {
                return false
            }
            clearComposition(client: sender)
            return true
        }
        if aSelector == #selector(NSResponder.insertNewline(_:)) || aSelector == #selector(NSResponder.insertTab(_:)) {
            guard !buffer.isEmpty else {
                return false
            }
            commitCandidate(at: 0, client: sender)
            return true
        }
        return false
    }

    @objc(composedString:)
    override func composedString(_ sender: Any!) -> Any! {
        guard !buffer.isEmpty || !cachedText.isEmpty else {
            return ""
        }
        let reading = (try? engine?.formatPreedit(buffer)) ?? buffer
        return cachedText.isEmpty ? reading : "\(cachedText)  \(reading)"
    }

    @objc(originalString:)
    override func originalString(_ sender: Any!) -> NSAttributedString! {
        NSAttributedString(string: cachedText + buffer)
    }

    @objc(candidates:)
    override func candidates(_ sender: Any!) -> [Any]! {
        currentCandidates.map(\.text)
    }

    @objc(menu)
    override func menu() -> NSMenu! {
        let menu = NSMenu(title: "Gonnyu")
        for region in GannyuRegion.allCases {
            let item = NSMenuItem(title: region.label, action: #selector(selectRegionFromMenu(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = region.rawValue
            item.state = region == GannyuRegionStore.shared.current ? .on : .off
            menu.addItem(item)
        }
        return menu
    }

    @objc(commitComposition:)
    override func commitComposition(_ sender: Any!) {
        guard !buffer.isEmpty else {
            return
        }
        commitCandidate(at: 0, client: sender)
    }

    @objc(candidateSelected:)
    override func candidateSelected(_ candidateString: NSAttributedString!) {
        guard let candidateString else {
            return
        }
        commitText(candidateString.string, client: client())
    }

    private func refreshCandidates(client sender: Any!) {
        currentCandidates = (try? engine?.retrieveCandidates(buffer)) ?? []
        if buffer.isEmpty {
            clearMarkedText(client: sender)
            return
        }
        updateComposition()
    }

    private func commitCandidate(at index: Int, client sender: Any!) {
        guard currentCandidates.indices.contains(index) else { return }
        let candidate = currentCandidates[index]
        cachedText += candidate.text
        let count = max(0, min(candidate.consumedBytes, buffer.utf8.count))
        if count > 0 && count < buffer.utf8.count {
            buffer = String(decoding: buffer.utf8.dropFirst(count), as: UTF8.self)
            refreshCandidates(client: sender)
            return
        }
        commitText(cachedText, client: sender)
    }

    private func commitText(_ text: String, client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            buffer = ""
            currentCandidates = []
            return
        }
        client.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        buffer = ""
        cachedText = ""
        currentCandidates = []
        clearMarkedText(client: client)
    }

    private func clearComposition(client sender: Any!) {
        buffer = ""
        cachedText = ""
        currentCandidates = []
        clearMarkedText(client: sender)
    }

    private func clearMarkedText(client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            return
        }
        client.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    private func shouldAppend(_ string: String) -> Bool {
        string.unicodeScalars.allSatisfy {
            CharacterSet.lowercaseLetters.contains($0)
                || CharacterSet.uppercaseLetters.contains($0)
                || $0 == "'"
        }
    }

    private func candidateIndex(for string: String) -> Int? {
        guard string.count == 1, let scalar = string.unicodeScalars.first else {
            return nil
        }
        guard CharacterSet.decimalDigits.contains(scalar) else {
            return nil
        }
        if scalar == "0" {
            return 9
        }
        guard let value = Int(string), value > 0 else {
            return nil
        }
        return value - 1
    }

    @objc private func regionDidChange() {
        engine = nil
        buffer = ""
        cachedText = ""
        currentCandidates = []
    }

    @objc private func selectRegionFromMenu(_ sender: Any?) {
        let item: NSMenuItem?
        if let menuItem = sender as? NSMenuItem {
            item = menuItem
        } else if let info = sender as? [AnyHashable: Any] {
            item = info[kIMKCommandMenuItemName] as? NSMenuItem
        } else {
            item = nil
        }
        guard let raw = item?.representedObject as? String, let region = GannyuRegion(rawValue: raw) else { return }
        GannyuRegionStore.shared.current = region
    }
}
