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
    private let candidatePanel = GannyuCandidatePanel()
    private let log = Logger(subsystem: "org.doohaey.inputmethod.gonnyu.native", category: "input")

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        createEngineIfNeeded()
        candidatePanel.onSelect = { [weak self] index in
            guard let self else { return }
            _ = self.select(index, client: self.client())
        }
        candidatePanel.onPage = { [weak self] direction in
            guard let self else { return }
            _ = self.page(direction, client: self.client())
        }
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
        return processText(string, client: sender)
    }

    @objc(handleEvent:client:)
    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, event.type == .keyDown else { return false }
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
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
            return active ? process(.enter, client: sender) : false
        case kVK_Tab:
            return active ? selectHighlighted(client: sender) : false
        case kVK_UpArrow:
            return active ? page(-1, client: sender) : false
        case kVK_DownArrow:
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
        Int(NSEvent.EventTypeMask([.keyDown, .flagsChanged]).rawValue)
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
        guard let client = client() as? NSTextInputClient else {
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
        if active && text == "<" { return page(-1, client: sender) }
        if active && text == ">" { return page(1, client: sender) }
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

    private func page(_ direction: Int, client sender: Any!) -> Bool {
        guard let engine, let result = try? engine.changePage(direction: direction) else { return false }
        render(result, client: sender)
        return true
    }

    private func select(_ index: Int, client sender: Any!) -> Bool {
        guard let engine, let result = try? engine.selectCandidate(globalIndex: index) else { return false }
        render(result, client: sender)
        return result.handled
    }

    private func selectLine(_ line: Int, client sender: Any!) -> Bool {
        guard let candidate = snapshot?.candidates.first(where: { $0.pageIndex == line }) else { return false }
        return select(candidate.globalIndex, client: sender)
    }

    private func selectHighlighted(client sender: Any!) -> Bool {
        guard let state = snapshot, !state.candidates.isEmpty else { return false }
        let line = state.highlightedIndex ?? 0
        return selectLine(line, client: sender)
    }

    private func clear(client sender: Any!) {
        if let engine, let result = try? engine.clearComposition() {
            render(result, client: sender)
        } else {
            snapshot = nil
            displays = []
            candidatePanel.hide()
            (sender as? NSTextInputClient)?.unmarkText()
        }
    }

    private func render(_ result: GannyuSnapshot, client sender: Any!) {
        snapshot = result
        guard let client = sender as? NSTextInputClient else { return }
        if let commit = result.commitText, !commit.isEmpty {
            client.insertText(commit, replacementRange: replacementRange(for: client))
        }
        if result.rawInput.isEmpty {
            client.unmarkText()
            candidatePanel.hide()
            displays = []
            return
        }
        let preedit = result.preedit.isEmpty ? result.rawInput : result.preedit
        let caret = min(max(result.caret, 0), preedit.count)
        client.setMarkedText(
            preedit,
            selectedRange: NSRange(location: (String(preedit.prefix(caret)) as NSString).length, length: 0),
            replacementRange: replacementRange(for: client)
        )
        displays = result.candidates.map { candidate in
            let display = NSMutableAttributedString(
                string: candidate.text,
                attributes: [.font: NSFont.systemFont(ofSize: 18), .foregroundColor: NSColor.labelColor]
            )
            if !candidate.annotation.isEmpty {
                display.append(NSAttributedString(
                    string: "\n\(candidate.annotation.replacingOccurrences(of: "\n", with: " "))",
                    attributes: [
                        .font: NSFont.systemFont(ofSize: 12),
                        .foregroundColor: NSColor.secondaryLabelColor,
                    ]
                ))
            }
            return display
        }
        guard !displays.isEmpty else { candidatePanel.hide(); return }
        candidatePanel.present(result, anchor: candidateAnchor(for: client))
    }

    private func replacementRange(for client: NSTextInputClient) -> NSRange {
        let marked = client.markedRange()
        return marked.location == NSNotFound ? client.selectedRange() : marked
    }

    private func candidateIndex(for display: NSAttributedString?) -> Int? {
        guard let display else { return nil }
        return snapshot?.candidates.first(where: { $0.text == display.string })?.globalIndex
    }

    private func candidateAnchor(for client: NSTextInputClient) -> NSRect {
        let marked = client.markedRange()
        let range = marked.location == NSNotFound ? client.selectedRange() : marked
        let rect = client.firstRect(forCharacterRange: range, actualRange: nil)
        if !rect.isEmpty { return rect }
        return NSRect(origin: NSEvent.mouseLocation, size: .zero)
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
