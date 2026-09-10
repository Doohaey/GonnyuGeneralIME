import AppKit
import Carbon.HIToolbox
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
    private var bufferCursor = 0
    private var cachedText = ""
    private var currentCandidates: [GannyuRetrievedCandidate] = []
    private var isActive = false
    private lazy var candidatesWindow: IMKCandidates? = {
        guard let server = server() else {
            return nil
        }
        let window = IMKCandidates(server: server, panelType: kIMKSingleRowSteppingCandidatePanel)
        window?.setAttributes([
            IMKCandidatesSendServerKeyEventFirst: NSNumber(value: true),
        ])
        return window
    }()

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        NotificationCenter.default.addObserver(self, selector: #selector(regionDidChange), name: GannyuRegionStore.didChange, object: nil)
    }

    deinit { NotificationCenter.default.removeObserver(self) }

    @objc(activateServer:)
    override func activateServer(_ sender: Any!) {
        isActive = true
    }

    @objc(deactivateServer:)
    override func deactivateServer(_ sender: Any!) {
        isActive = false
        clearComposition(client: sender)
    }

    @objc(handleEvent:client:)
    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, event.type == .keyDown else {
            return false
        }

        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if !modifiers.intersection([.command, .control, .function]).isEmpty {
            return false
        }

        switch Int(event.keyCode) {
        case kVK_LeftArrow:
            guard hasComposition else {
                return false
            }
            moveCursor(by: -1)
            updateComposition()
            return true
        case kVK_RightArrow:
            guard hasComposition else {
                return false
            }
            moveCursor(by: 1)
            updateComposition()
            return true
        case kVK_Delete:
            return handleDelete(client: sender)
        case kVK_Escape:
            guard hasComposition else {
                return false
            }
            clearComposition(client: sender)
            return true
        case kVK_Space, kVK_Return, kVK_ANSI_KeypadEnter:
            guard hasComposition else {
                return false
            }
            commitCandidate(at: 0, client: sender)
            return true
        default:
            break
        }

        if let index = candidateIndex(forKeyCode: Int(event.keyCode)) {
            guard hasComposition, index < currentCandidates.count else {
                return false
            }
            commitCandidate(at: index, client: sender)
            return true
        }

        let input = event.charactersIgnoringModifiers ?? event.characters ?? ""
        if let index = candidateIndex(for: input) {
            guard hasComposition, index < currentCandidates.count else {
                return false
            }
            commitCandidate(at: index, client: sender)
            return true
        }

        return processText(input, client: sender)
    }

    private func processText(_ string: String!, client sender: Any!) -> Bool {
        guard let string else {
            return false
        }
        let normalized = string.lowercased()
        if normalized.isEmpty {
            return false
        }
        if normalized == " " || normalized == "\r" || normalized == "\n" {
            guard !buffer.isEmpty else {
                return false
            }
            commitCandidate(at: 0, client: sender)
            return true
        }
        if let index = candidateIndex(for: normalized) {
            guard !buffer.isEmpty, index < currentCandidates.count else {
                return false
            }
            commitCandidate(at: index, client: sender)
            return true
        }
        guard shouldAppend(normalized) else {
            return false
        }
        insertTextIntoBuffer(normalized)
        refreshCandidates(client: sender)
        return true
    }

    @objc(composedString:)
    override func composedString(_ sender: Any!) -> Any! {
        guard !buffer.isEmpty || !cachedText.isEmpty else {
            return ""
        }
        let reading = currentPreeditDisplay()
        return cachedText.isEmpty ? reading : "\(cachedText)  \(reading)"
    }

    @objc(originalString:)
    override func originalString(_ sender: Any!) -> NSAttributedString! {
        NSAttributedString(string: cachedText + buffer)
    }

    @objc(selectionRange)
    override func selectionRange() -> NSRange {
        let prefix = String(buffer.prefix(bufferCursor))
        let prefixDisplay = (try? engine?.formatPreedit(prefix)) ?? prefix
        return NSRange(location: composedPrefixLength + nsLength(of: prefixDisplay), length: 0)
    }

    @objc(replacementRange)
    override func replacementRange() -> NSRange {
        guard let client = client() as? NSTextInputClient else {
            return NSRange(location: NSNotFound, length: 0)
        }
        let markedRange = client.markedRange()
        if markedRange.location != NSNotFound {
            return markedRange
        }
        return client.selectedRange()
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
        guard hasComposition else {
            return
        }
        if buffer.isEmpty {
            commitText(cachedText, client: sender)
            return
        }
        commitCandidate(at: 0, client: sender)
    }

    @objc(candidateSelected:)
    override func candidateSelected(_ candidateString: NSAttributedString!) {
        let selectedIndex = candidatesWindow?.selectedCandidate() ?? NSNotFound
        if selectedIndex != NSNotFound, currentCandidates.indices.contains(selectedIndex) {
            commitCandidate(at: selectedIndex, client: client())
            return
        }
        guard
            let candidateString,
            let index = currentCandidates.firstIndex(where: { $0.text == candidateString.string })
        else {
            return
        }
        commitCandidate(at: index, client: client())
    }

    private func refreshCandidates(client sender: Any!) {
        bufferCursor = min(max(bufferCursor, 0), buffer.count)
        currentCandidates = (try? engine?.retrieveCandidates(buffer)) ?? []
        if !hasComposition {
            clearMarkedText(client: sender)
            hideCandidates()
            return
        }
        updateComposition()
        if currentCandidates.isEmpty {
            hideCandidates()
            return
        }
        candidatesWindow?.update()
        candidatesWindow?.show(kIMKLocateCandidatesBelowHint)
    }

    private func commitCandidate(at index: Int, client sender: Any!) {
        guard currentCandidates.indices.contains(index) else { return }
        let candidate = currentCandidates[index]
        cachedText += candidate.text
        let count = max(0, min(candidate.consumedBytes, buffer.utf8.count))
        if count > 0 && count < buffer.utf8.count {
            buffer = String(decoding: buffer.utf8.dropFirst(count), as: UTF8.self)
            bufferCursor = max(0, bufferCursor - count)
            refreshCandidates(client: sender)
            return
        }
        commitText(cachedText, client: sender)
    }

    private func commitText(_ text: String, client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            buffer = ""
            bufferCursor = 0
            cachedText = ""
            currentCandidates = []
            hideCandidates()
            return
        }
        client.insertText(text, replacementRange: currentReplacementRange(for: client))
        buffer = ""
        bufferCursor = 0
        cachedText = ""
        currentCandidates = []
        clearMarkedText(client: client)
        hideCandidates()
    }

    private func clearComposition(client sender: Any!) {
        buffer = ""
        bufferCursor = 0
        cachedText = ""
        currentCandidates = []
        clearMarkedText(client: sender)
        hideCandidates()
    }

    private func clearMarkedText(client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            return
        }
        client.unmarkText()
    }

    private func shouldAppend(_ string: String) -> Bool {
        string.unicodeScalars.allSatisfy {
            CharacterSet.lowercaseLetters.contains($0)
                || CharacterSet.uppercaseLetters.contains($0)
                || $0 == "'"
        }
    }

    private var hasComposition: Bool {
        !buffer.isEmpty || !cachedText.isEmpty
    }

    private var composedPrefixLength: Int {
        guard !cachedText.isEmpty, !buffer.isEmpty else {
            return nsLength(of: cachedText)
        }
        return nsLength(of: cachedText) + 2
    }

    private func handleDelete(client sender: Any!) -> Bool {
        guard hasComposition else {
            return false
        }
        if !cachedText.isEmpty, bufferCursor == 0 {
            cachedText.removeLast()
            if buffer.isEmpty && cachedText.isEmpty {
                clearComposition(client: sender)
                return true
            }
            refreshCandidates(client: sender)
            return true
        }
        guard bufferCursor > 0 else {
            return true
        }
        let end = buffer.index(buffer.startIndex, offsetBy: bufferCursor)
        let start = buffer.index(before: end)
        buffer.removeSubrange(start..<end)
        bufferCursor -= 1
        refreshCandidates(client: sender)
        return true
    }

    private func insertTextIntoBuffer(_ text: String) {
        let insertionIndex = buffer.index(buffer.startIndex, offsetBy: bufferCursor)
        buffer.insert(contentsOf: text, at: insertionIndex)
        bufferCursor += text.count
    }

    private func moveCursor(by delta: Int) {
        bufferCursor = min(max(bufferCursor + delta, 0), buffer.count)
    }

    private func currentPreeditDisplay() -> String {
        let consumedBytes = currentCandidates.first?.consumedBytes ?? 0
        return (try? engine?.formatPreedit(buffer, consumedBytes: consumedBytes)) ?? buffer
    }

    private func currentReplacementRange(for client: NSTextInputClient) -> NSRange {
        let markedRange = client.markedRange()
        if markedRange.location != NSNotFound {
            return markedRange
        }
        return client.selectedRange()
    }

    private func hideCandidates() {
        candidatesWindow?.hide()
    }

    private func nsLength(of string: String) -> Int {
        (string as NSString).length
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

    private func candidateIndex(forKeyCode keyCode: Int) -> Int? {
        switch keyCode {
        case kVK_ANSI_1, kVK_ANSI_Keypad1:
            return 0
        case kVK_ANSI_2, kVK_ANSI_Keypad2:
            return 1
        case kVK_ANSI_3, kVK_ANSI_Keypad3:
            return 2
        case kVK_ANSI_4, kVK_ANSI_Keypad4:
            return 3
        case kVK_ANSI_5, kVK_ANSI_Keypad5:
            return 4
        case kVK_ANSI_6, kVK_ANSI_Keypad6:
            return 5
        case kVK_ANSI_7, kVK_ANSI_Keypad7:
            return 6
        case kVK_ANSI_8, kVK_ANSI_Keypad8:
            return 7
        case kVK_ANSI_9, kVK_ANSI_Keypad9:
            return 8
        case kVK_ANSI_0, kVK_ANSI_Keypad0:
            return 9
        default:
            return nil
        }
    }

    @objc private func regionDidChange() {
        engine = nil
        buffer = ""
        bufferCursor = 0
        cachedText = ""
        currentCandidates = []
        hideCandidates()
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
