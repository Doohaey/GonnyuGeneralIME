import AppKit
import Carbon.HIToolbox
import InputMethodKit
import GannyuMacOSSupport
import os

@objc(GannyuInputController)
final class GannyuInputController: IMKInputController {
    private var engine: GannyuEngine?
    private var snapshot: GannyuSnapshot?
    private var displays: [NSAttributedString] = []
    private var shiftOnlyPress = false
    private var selectedLine = 0
    private var candidateWindow: IMKCandidates?
    private let log = Logger(subsystem: "org.doohaey.inputmethod.gonnyu.native", category: "input")

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        createEngineIfNeeded()
        if let server {
            let window = IMKCandidates(server: server, panelType: kIMKSingleColumnScrollingCandidatePanel)
            window?.setDismissesAutomatically(false)
            window?.setAttributes([IMKCandidatesSendServerKeyEventFirst: true])
            candidateWindow = window
        }
        if diagnosticsEnabled { log.notice("IMK input controller created") }
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(regionDidChange),
            name: GannyuRegionStore.didChange,
            object: nil
        )
    }

    deinit { NotificationCenter.default.removeObserver(self) }

    @objc(activateServer:)
    override func activateServer(_ sender: Any!) {
        createEngineIfNeeded()
    }

    @objc(deactivateServer:)
    override func deactivateServer(_ sender: Any!) {
        clear(client: sender)
    }

    @objc(inputText:client:)
    override func inputText(_ string: String!, client sender: Any!) -> Bool {
        guard let string, !string.isEmpty else { return false }
        if diagnosticsEnabled { log.notice("IMK inputText received \(string, privacy: .private)") }
        return processText(string, client: sender)
    }

    @objc(handleEvent:client:)
    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event else { return false }
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if event.type == .flagsChanged {
            return handleModifierChange(modifiers, client: sender)
        }
        guard event.type == .keyDown else { return false }
        if diagnosticsEnabled {
            log.notice("IMK keyDown received keyCode=\(event.keyCode, privacy: .public)")
        }
        // A following key means this was a modifier chord, not a standalone
        // Shift language toggle.
        shiftOnlyPress = false
        if modifiers.contains(.command) || modifiers.contains(.control)
            || modifiers.contains(.option) || modifiers.contains(.function) { return false }

        let active = !(snapshot?.rawInput.isEmpty ?? true)
        switch Int(event.keyCode) {
        case kVK_Delete, kVK_ForwardDelete:
            return active ? process(.backspace, client: sender) : false
        case kVK_Escape:
            if active { clear(client: sender); return true }
            return false
        case kVK_Return, kVK_ANSI_KeypadEnter:
            return active ? selectHighlighted(client: sender) : false
        case kVK_Tab:
            return active ? selectHighlighted(client: sender) : false
        case kVK_UpArrow:
            return active ? moveSelection(-1, client: sender) : false
        case kVK_DownArrow:
            return active ? moveSelection(1, client: sender) : false
        // Keep paging on the two physical punctuation keys, regardless of
        // whether Shift produces < / > on the active keyboard layout.
        case kVK_ANSI_Comma:
            return active ? page(-1, client: sender) : false
        case kVK_ANSI_Period:
            return active ? page(1, client: sender) : false
        default:
            break
        }
        if active, let line = candidateLineNumber(event.keyCode) {
            return selectLine(line, client: sender)
        }
        let characters = event.characters?.isEmpty == false
            ? event.characters
            : event.charactersIgnoringModifiers
        guard let characters, !characters.isEmpty else {
            log.debug("IMK key event has no printable characters; keyCode=\(event.keyCode, privacy: .public)")
            return false
        }
        return processText(characters, client: sender)
    }

    override func recognizedEvents(_ sender: Any!) -> Int {
        // IMK's default composition and keybinding path is only enabled when
        // this is exactly the key-down mask.
        Int(NSEvent.EventTypeMask.keyDown.rawValue | NSEvent.EventTypeMask.flagsChanged.rawValue)
    }

    @objc(didCommandBySelector:client:)
    override func didCommand(by selector: Selector!, client sender: Any!) -> Bool {
        guard let selector else { return false }
        let active = !(snapshot?.rawInput.isEmpty ?? true)
        switch NSStringFromSelector(selector) {
        case "deleteBackward:":
            return active ? process(.backspace, client: sender) : false
        case "cancelOperation:":
            if active { clear(client: sender); return true }
            return false
        case "insertNewline:", "insertLineBreak:":
            return active ? process(.enter, client: sender) : false
        case "insertTab:":
            return active ? selectHighlighted(client: sender) : false
        default:
            return false
        }
    }

    @objc(composedString:)
    override func composedString(_ sender: Any!) -> Any! {
        snapshot?.preedit ?? ""
    }

    @objc(originalString:)
    override func originalString(_ sender: Any!) -> NSAttributedString! {
        NSAttributedString(string: snapshot?.rawInput ?? "")
    }

    @objc(selectionRange)
    override func selectionRange() -> NSRange {
        let preedit = snapshot?.preedit ?? ""
        let caret = min(max(snapshot?.caret ?? 0, 0), preedit.count)
        return NSRange(location: (String(preedit.prefix(caret)) as NSString).length, length: 0)
    }

    @objc(replacementRange)
    override func replacementRange() -> NSRange {
        guard let client = client() else {
            return NSRange(location: NSNotFound, length: 0)
        }
        return replacementRange(for: client)
    }

    @objc(candidates:)
    override func candidates(_ sender: Any!) -> [Any]! { displays }

    @objc(menu)
    override func menu() -> NSMenu! {
        let menu = NSMenu(title: "Gonnyu")
        let current = GannyuRegionStore.shared.currentID()
        for region in (try? GannyuEngine.availableRegions()) ?? [] {
            let item = NSMenuItem(title: region.nameZh, action: #selector(selectRegion(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = region.id
            item.state = region.id == current ? .on : .off
            menu.addItem(item)
        }
        return menu
    }

    @objc(commitComposition:)
    override func commitComposition(_ sender: Any!) {
        _ = selectHighlighted(client: sender)
    }

    @objc(candidateSelected:)
    override func candidateSelected(_ candidateString: NSAttributedString!) {
        guard let index = candidateIndex(for: candidateString) else { return }
        _ = select(index, client: client())
    }

    @objc(candidateSelectionChanged:)
    override func candidateSelectionChanged(_ candidateString: NSAttributedString!) {}

    private func processText(_ text: String, client sender: Any!) -> Bool {
        let active = !(snapshot?.rawInput.isEmpty ?? true)
        if snapshot?.asciiMode == true {
            guard let client = sender as? IMKTextInput else { return false }
            client.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
            return true
        }
        if active && (text == "," || text == "<") { return page(-1, client: sender) }
        if active && (text == "." || text == ">") { return page(1, client: sender) }
        if text == " " { return active ? process(.space, client: sender) : false }
        if active && text.count == 1, let line = Int(text), line > 0 {
            return selectLine(line - 1, client: sender)
        }
        return process(.text(text), client: sender)
    }

    private func process(_ event: GannyuKeyEvent, client sender: Any!) -> Bool {
        guard let engine else {
            log.error("Rime engine is unavailable while processing input")
            return false
        }
        do {
            let result = try engine.process(event)
            guard result.handled else { return false }
            render(result, client: sender)
            return true
        } catch {
            log.error("Rime engine failed to process input: \(String(describing: error), privacy: .public)")
            return false
        }
    }

    private func createEngineIfNeeded() {
        guard engine == nil else { return }
        do {
            engine = try GannyuEngine()
        } catch {
            log.error("Unable to create Rime engine: \(String(describing: error), privacy: .public)")
        }
    }

    private var diagnosticsEnabled: Bool {
        UserDefaults.standard.bool(forKey: "GannyuIMKDiagnostics")
    }

    private func page(_ direction: Int, client sender: Any!) -> Bool {
        guard let engine, let result = try? engine.changePage(direction: direction) else { return false }
        selectedLine = 0
        render(result, client: sender)
        return true
    }

    private func select(_ index: Int, client sender: Any!) -> Bool {
        guard let engine, let result = try? engine.selectCandidate(globalIndex: index) else { return false }
        render(result, client: sender)
        return result.handled
    }

    private func selectLine(_ line: Int, client sender: Any!) -> Bool {
        guard let candidate = snapshot?.candidates.sorted(by: { $0.pageIndex < $1.pageIndex })[safe: line] else { return false }
        return select(candidate.globalIndex, client: sender)
    }

    private func handleModifierChange(_ modifiers: NSEvent.ModifierFlags, client sender: Any!) -> Bool {
        let hasBlockingModifier = modifiers.contains(.command) || modifiers.contains(.control)
            || modifiers.contains(.option) || modifiers.contains(.function)
        let shiftDown = modifiers.contains(.shift)
        if shiftDown, !hasBlockingModifier {
            shiftOnlyPress = true
            return true
        }
        guard !shiftDown, shiftOnlyPress else {
            shiftOnlyPress = false
            return false
        }
        shiftOnlyPress = false
        guard let engine else { return false }
        if !(snapshot?.rawInput.isEmpty ?? true) { clear(client: sender) }
        guard let result = try? engine.setASCIIMode(!(snapshot?.asciiMode ?? false)) else { return false }
        render(result, client: sender)
        return true
    }

    private func selectHighlighted(client sender: Any!) -> Bool {
        guard let state = snapshot, !state.candidates.isEmpty else { return false }
        return selectLine(selectedLine, client: sender)
    }

    private func moveSelection(_ direction: Int, client sender: Any!) -> Bool {
        guard let state = snapshot, !state.candidates.isEmpty else { return false }
        let count = state.candidates.count
        let next = selectedLine + direction
        if next < 0, state.hasPreviousPage {
            guard page(-1, client: sender) else { return false }
            selectedLine = snapshot?.candidates.count ?? 1
            selectedLine = max(selectedLine - 1, 0)
            presentCandidates()
            return true
        }
        if next >= count, state.hasNextPage {
            guard page(1, client: sender) else { return false }
            selectedLine = 0
            presentCandidates()
            return true
        }
        guard next >= 0, next < count else { return false }
        selectedLine = next
        presentCandidates()
        return true
    }

    private func clear(client sender: Any!) {
        if let engine, let result = try? engine.clearComposition() {
            render(result, client: sender)
        } else {
            snapshot = nil
            displays = []
            candidateWindow?.hide()
            if let client = sender as? IMKTextInput { clearMarkedText(on: client) }
        }
    }

    private func render(_ result: GannyuSnapshot, client sender: Any!) {
        snapshot = result
        guard let client = sender as? IMKTextInput else {
            log.error("IMK client does not conform to IMKTextInput")
            return
        }
        if let commit = result.commitText, !commit.isEmpty {
            client.insertText(commit, replacementRange: replacementRange(for: client))
        }
        if result.rawInput.isEmpty {
            selectedLine = 0
            clearMarkedText(on: client)
            candidateWindow?.hide()
            displays = []
            return
        }
        let preedit = result.preedit.isEmpty ? result.rawInput : result.preedit
        let caret = min(max(result.caret, 0), preedit.count)
        client.setMarkedText(
            preedit,
            selectionRange: NSRange(location: (String(preedit.prefix(caret)) as NSString).length, length: 0),
            replacementRange: replacementRange(for: client)
        )
        selectedLine = min(selectedLine, max(result.candidates.count - 1, 0))
        presentCandidates()
    }

    private func presentCandidates() {
        let candidates = snapshot?.candidates.sorted(by: { $0.pageIndex < $1.pageIndex }) ?? []
        displays = candidates.enumerated().map { display(for: $0.element, selected: $0.offset == selectedLine) }
        guard !displays.isEmpty else { candidateWindow?.hide(); return }
        candidateWindow?.setCandidateData(displays)
        candidateWindow?.show(kIMKLocateCandidatesBelowHint)
    }

    private func replacementRange(for client: IMKTextInput) -> NSRange {
        let marked = client.markedRange()
        return marked.location == NSNotFound ? client.selectedRange() : marked
    }

    private func clearMarkedText(on client: IMKTextInput) {
        client.setMarkedText(
            "",
            selectionRange: NSRange(location: 0, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: NSNotFound)
        )
    }

    private func candidateIndex(for display: NSAttributedString?) -> Int? {
        guard let display else { return nil }
        let candidates = snapshot?.candidates.sorted(by: { $0.pageIndex < $1.pageIndex }) ?? []
        return zip(candidates, displays).first(where: { $0.1.string == display.string })?.0.globalIndex
    }

    private func display(for candidate: GannyuCandidate, selected: Bool) -> NSAttributedString {
        let primaryColor = selected ? NSColor.alternateSelectedControlTextColor : NSColor.labelColor
        let secondaryColor = selected ? NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.85) : NSColor.secondaryLabelColor
        let display = NSMutableAttributedString(
            string: "\(candidate.pageIndex + 1). \(candidate.text)",
            attributes: [.font: NSFont.systemFont(ofSize: 18), .foregroundColor: primaryColor]
        )
        if !candidate.annotation.isEmpty {
            display.append(NSAttributedString(
                string: "  \(candidate.annotation.replacingOccurrences(of: "\n", with: " "))",
                attributes: [
                    .font: NSFont.systemFont(ofSize: 12),
                    .foregroundColor: secondaryColor,
                ]
            ))
        }
        if selected { display.addAttribute(.backgroundColor, value: NSColor.selectedControlColor, range: NSRange(location: 0, length: display.length)) }
        return display
    }

    private func candidateLineNumber(_ keyCode: UInt16) -> Int? {
        switch Int(keyCode) {
        case kVK_ANSI_1, kVK_ANSI_Keypad1: return 0
        case kVK_ANSI_2, kVK_ANSI_Keypad2: return 1
        case kVK_ANSI_3, kVK_ANSI_Keypad3: return 2
        case kVK_ANSI_4, kVK_ANSI_Keypad4: return 3
        case kVK_ANSI_5, kVK_ANSI_Keypad5: return 4
        case kVK_ANSI_6, kVK_ANSI_Keypad6: return 5
        case kVK_ANSI_7, kVK_ANSI_Keypad7: return 6
        case kVK_ANSI_8, kVK_ANSI_Keypad8: return 7
        case kVK_ANSI_9, kVK_ANSI_Keypad9: return 8
        default: return nil
        }
    }

    @objc private func regionDidChange() {
        guard let region = GannyuRegionStore.shared.currentID() else { return }
        if let engine, let result = try? engine.switchRegion(region) {
            render(result, client: client())
        }
    }

    @objc private func selectRegion(_ sender: NSMenuItem) {
        guard let region = sender.representedObject as? String else { return }
        _ = GannyuRegionStore.shared.select(region)
    }
}

private extension Collection {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
