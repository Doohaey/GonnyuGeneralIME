import AppKit
import ApplicationServices
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
    private let candidatePanel = GannyuCandidatePanel()
    private let pageHint = GannyuPageHint()
    private let modeHint = GannyuModeHint()
    private var candidateWindow: IMKCandidates?
    private var lastCandidateAnchor: NSRect?
    private let fullwidthPunctuationKey = "org.doohaey.gonnyu.fullwidthPunctuation"
    private let log = Logger(subsystem: "org.doohaey.inputmethod.gonnyu.native", category: "input")

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        createEngineIfNeeded()
        if let server {
            let w = IMKCandidates(server: server, panelType: kIMKSingleColumnScrollingCandidatePanel, styleType: kIMKMain)
            w?.setAttributes([
                IMKCandidatesOpacityAttributeName: NSNumber(value: 0),
                IMKCandidatesSendServerKeyEventFirst: NSNumber(value: true),
            ])
            w?.setDismissesAutomatically(false)
            candidateWindow = w
        }
        candidatePanel.onSelect = { [weak self] index in
            guard let self else { return }
            _ = self.select(index, client: self.client())
        }
        candidatePanel.onPage = { [weak self] direction in
            guard let self else { return }
            _ = self.page(direction, client: self.client())
        }
        pageHint.onPage = { [weak self] direction in
            guard let self else { return }
            _ = self.page(direction, client: self.client())
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
        // Discard the stale anchor so it is not mistakenly reused when this
        // controller is next activated for the same or a different client.
        lastCandidateAnchor = nil
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
        let arrowKey = event.keyCode == kVK_UpArrow || event.keyCode == kVK_DownArrow
        if modifiers.contains(.command) || modifiers.contains(.control)
            || modifiers.contains(.option) || (modifiers.contains(.function) && !arrowKey) { return false }

        let active = !(snapshot?.rawInput.isEmpty ?? true)
        switch Int(event.keyCode) {
        case kVK_Space where modifiers.contains(.shift) && !active && snapshot?.asciiMode != true:
            setFullwidthPunctuation(!fullwidthPunctuation, client: sender)
            return true
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
        if diagnosticsEnabled {
            log.notice("IMK command received selector=\(NSStringFromSelector(selector), privacy: .public)")
        }
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
        case "moveUp:":
            return active ? moveSelection(-1, client: sender) : false
        case "moveDown:":
            return active ? moveSelection(1, client: sender) : false
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
        if !active, fullwidthPunctuation, let symbol = fullwidthSymbol(for: text) {
            guard let client = sender as? IMKTextInput else { return false }
            client.insertText(symbol, replacementRange: NSRange(location: NSNotFound, length: 0))
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

    private var fullwidthPunctuation: Bool {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: fullwidthPunctuationKey) != nil else { return true }
        return defaults.bool(forKey: fullwidthPunctuationKey)
    }

    private func setFullwidthPunctuation(_ enabled: Bool, client sender: Any!) {
        UserDefaults.standard.set(enabled, forKey: fullwidthPunctuationKey)
        showModeHint(title: enabled ? "全" : "半", client: sender)
    }

    private func fullwidthSymbol(for text: String) -> String? {
        guard text.count == 1 else { return nil }
        switch text {
        case ",": return "，"
        case ".": return "。"
        case "\\": return "、"
        case ";": return "；"
        case ":": return "："
        case "?": return "？"
        case "!": return "！"
        case "(": return "（"
        case ")": return "）"
        case "[": return "【"
        case "]": return "】"
        case "<": return "《"
        case ">": return "》"
        case "\"": return "＂"
        case "~": return "～"
        case "-": return "－"
        default: return nil
        }
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
        showModeHint(title: result.asciiMode ? "英" : "赣", client: sender)
        return true
    }

    private func showModeHint(title: String, client sender: Any!) {
        if let client = sender as? IMKTextInput, let rect = caretRect(for: client) {
            lastCandidateAnchor = rect
            modeHint.show(title: title, near: rect)
            return
        }
        if let anchor = lastCandidateAnchor {
            modeHint.show(title: title, near: anchor)
            return
        }
        // No cursor position available via any API; fall back to near the mouse
        // so the mode toggle is at least visible.
        let mouse = NSEvent.mouseLocation
        modeHint.show(title: title, near: NSRect(x: mouse.x, y: mouse.y - 4, width: 0, height: 18))
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
            candidatePanel.hide()
            pageHint.hide()
            modeHint.hide()
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
            candidatePanel.hide()
            pageHint.hide()
            modeHint.hide()
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
        modeHint.hide()
        guard let state = snapshot else { return }
        let candidates = state.candidates.sorted(by: { $0.pageIndex < $1.pageIndex })
        displays = candidates.enumerated().map { display(for: $0.element, line: $0.offset, selected: $0.offset == selectedLine) }
        guard !displays.isEmpty else {
            candidatePanel.hide()
            pageHint.hide()
            return
        }

        // Primary: firstRect via marked/selected range (standard Cocoa clients).
        if let client = client(), let anchor = caretRect(for: client) {
            lastCandidateAnchor = anchor
            pageHint.hide()
            candidatePanel.present(state, selectedLine: selectedLine, anchor: anchor)
            return
        }

        // Secondary: use the last successfully resolved anchor from this
        // controller's context (e.g. keep the panel in place while the client
        // briefly declines to report a position between keystrokes).
        if let anchor = lastCandidateAnchor {
            pageHint.hide()
            candidatePanel.present(state, selectedLine: selectedLine, anchor: anchor)
            return
        }

        // Last resort: position the panel near the current mouse cursor so it
        // is at least visible somewhere relevant on screen.  This path is
        // taken for clients (e.g. Terminal.app) that report no cursor position
        // through any available API.
        let mouse = NSEvent.mouseLocation
        let mouseAnchor = NSRect(x: mouse.x, y: mouse.y - 4, width: 0, height: 18)
        if mouseAnchor.intersectsAnyScreen {
            pageHint.hide()
            candidatePanel.present(state, selectedLine: selectedLine, anchor: mouseAnchor)
            return
        }

        candidatePanel.hide()
        pageHint.hide()
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

    private func display(for candidate: GannyuCandidate, line: Int, selected: Bool) -> NSAttributedString {
        let primaryColor = selected ? NSColor.alternateSelectedControlTextColor : NSColor.labelColor
        let secondaryColor = selected ? NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.85) : NSColor.secondaryLabelColor
        let display = NSMutableAttributedString(
            string: "\(line + 1). \(candidate.text)",
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

    private func caretRect(for client: IMKTextInput) -> NSRect? {
        // Primary: firstRect via marked/selected range.  Works for standard
        // Cocoa text views (NSTextView, WebKit, Electron, etc.).
        for range in [client.markedRange(), client.selectedRange()] where range.location != NSNotFound {
            var actualRange = NSRange(location: NSNotFound, length: 0)
            let rect = client.firstRect(forCharacterRange: range, actualRange: &actualRange)
            if diagnosticsEnabled {
                log.notice("IMK caret range=\(range.location, privacy: .public):\(range.length, privacy: .public) actual=\(actualRange.location, privacy: .public):\(actualRange.length, privacy: .public) rect=\(rect.origin.x, privacy: .public),\(rect.origin.y, privacy: .public),\(rect.width, privacy: .public),\(rect.height, privacy: .public)")
            }
            // Some clients return only a vertical caret line at x=0 when
            // they decline to expose their real insertion point.  That is
            // not an anchor; accepting it pins every auxiliary panel to the
            // screen's left/bottom edge.
            let hasHorizontalPosition = rect.origin.x > 1 || rect.width > 0
            guard rect.origin.x.isFinite, rect.origin.y.isFinite,
                  rect.width >= 0, rect.height > 0, hasHorizontalPosition,
                  rect.intersectsAnyScreen else { continue }
            return rect
        }

        // Secondary: lineHeightRectangle at character index 0.  Some clients
        // (including terminal emulators) implement this but not firstRect.
        var lineRect = NSRect.zero
        _ = client.attributes(forCharacterIndex: 0, lineHeightRectangle: &lineRect)
        if diagnosticsEnabled {
            log.notice("IMK lineHeightRect rect=\(lineRect.origin.x, privacy: .public),\(lineRect.origin.y, privacy: .public),\(lineRect.width, privacy: .public),\(lineRect.height, privacy: .public)")
        }
        if lineRect.origin.x.isFinite, lineRect.origin.y.isFinite, lineRect.height > 0,
           (lineRect.origin.x > 1 || lineRect.width > 0), lineRect.intersectsAnyScreen {
            return lineRect
        }

        // Tertiary: Accessibility API.  Covers terminal apps that expose
        // kAXBoundsForRangeParameterizedAttribute on the focused text element.
        if let axRect = caretRectViaAccessibility() {
            if diagnosticsEnabled {
                log.notice("IMK AX caret rect=\(axRect.origin.x, privacy: .public),\(axRect.origin.y, privacy: .public),\(axRect.width, privacy: .public),\(axRect.height, privacy: .public)")
            }
            return axRect
        }

        return nil
    }

    /// Returns the on-screen caret rect for the currently focused UI element
    /// using the macOS Accessibility API.  Coordinates are converted from
    /// Quartz (top-left origin) to AppKit screen space (bottom-left origin).
    private func caretRectViaAccessibility() -> NSRect? {
        guard let app = NSWorkspace.shared.frontmostApplication else { return nil }
        let axApp = AXUIElementCreateApplication(app.processIdentifier)

        var focusedRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(axApp, kAXFocusedUIElementAttribute as CFString, &focusedRef) == .success,
              let focusedRef else { return nil }
        // CFTypeRef to AXUIElement: safe because AXUIElement is a CF type.
        let focused = focusedRef as! AXUIElement  // swiftlint:disable:this force_cast

        // Try bounds for the selected text range first (precise cursor position).
        var selectedRangeRef: CFTypeRef?
        if AXUIElementCopyAttributeValue(focused, kAXSelectedTextRangeAttribute as CFString, &selectedRangeRef) == .success,
           let selectedRangeRef {
            var boundsRef: CFTypeRef?
            if AXUIElementCopyParameterizedAttributeValue(
                focused,
                kAXBoundsForRangeParameterizedAttribute as CFString,
                selectedRangeRef,
                &boundsRef
            ) == .success, let boundsRef {
                var cgRect = CGRect.zero
                if AXValueGetValue(boundsRef as! AXValue, AXValueType.cgRect, &cgRect), cgRect.height > 0 {  // swiftlint:disable:this force_cast
                    if let r = axRectToAppKit(cgRect), r.intersectsAnyScreen { return r }
                }
            }
        }

        // Fall back to the bounding rect of the focused element itself (coarse
        // but better than nothing — gives the correct quadrant of the screen).
        var posRef: CFTypeRef?
        var sizeRef: CFTypeRef?
        if AXUIElementCopyAttributeValue(focused, kAXPositionAttribute as CFString, &posRef) == .success,
           AXUIElementCopyAttributeValue(focused, kAXSizeAttribute as CFString, &sizeRef) == .success,
           let posRef, let sizeRef {
            var origin = CGPoint.zero
            var size = CGSize.zero
            if AXValueGetValue(posRef as! AXValue, AXValueType.cgPoint, &origin),  // swiftlint:disable:this force_cast
               AXValueGetValue(sizeRef as! AXValue, AXValueType.cgSize, &size),
               size.height > 0 {
                let cgRect = CGRect(x: origin.x, y: origin.y, width: size.width, height: size.height)
                if let r = axRectToAppKit(cgRect), r.intersectsAnyScreen { return r }
            }
        }

        return nil
    }

    /// Converts an Accessibility/Quartz rect (top-left origin) to AppKit screen
    /// coordinates (bottom-left origin).
    private func axRectToAppKit(_ rect: CGRect) -> NSRect? {
        guard let primary = NSScreen.screens.first else { return nil }
        return NSRect(
            x: rect.origin.x,
            y: primary.frame.height - rect.origin.y - rect.size.height,
            width: rect.size.width,
            height: max(rect.size.height, 14)
        )
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

private extension NSRect {
    var intersectsAnyScreen: Bool {
        NSScreen.screens.contains { $0.visibleFrame.intersects(self) }
    }
}

private final class GannyuPageHint: NSPanel {
    var onPage: ((Int) -> Void)?
    private let previous = NSButton(title: "<", target: nil, action: nil)
    private let next = NSButton(title: ">", target: nil, action: nil)

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 58, height: 24), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isOpaque = false
        backgroundColor = .clear
        level = .popUpMenu
        hasShadow = true
        let stack = NSStackView(views: [previous, next])
        stack.spacing = 1
        stack.edgeInsets = NSEdgeInsets(top: 2, left: 2, bottom: 2, right: 2)
        previous.target = self
        previous.action = #selector(previousPage)
        next.target = self
        next.action = #selector(nextPage)
        for button in [previous, next] {
            button.isBordered = true
            button.bezelStyle = .texturedRounded
            button.font = .systemFont(ofSize: 12, weight: .semibold)
            button.focusRingType = .none
        }
        contentView = stack
    }

    func show(near frame: NSRect, hasPreviousPage: Bool, hasNextPage: Bool) {
        previous.isEnabled = hasPreviousPage
        next.isEnabled = hasNextPage
        setFrameOrigin(NSPoint(x: frame.maxX - 62, y: frame.minY - 27))
        orderFrontRegardless()
    }

    func hide() { orderOut(nil) }
    @objc private func previousPage() { onPage?(-1) }
    @objc private func nextPage() { onPage?(1) }
}

private final class GannyuModeHint: NSPanel {
    private let label = NSTextField(labelWithString: "赣")
    private var generation = 0

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 30, height: 26), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isOpaque = false
        backgroundColor = .clear
        level = .popUpMenu
        hasShadow = true
        label.alignment = .center
        label.font = .systemFont(ofSize: 14, weight: .bold)
        label.textColor = .white
        label.wantsLayer = true
        label.layer?.backgroundColor = NSColor.controlAccentColor.cgColor
        label.layer?.cornerRadius = 6
        label.translatesAutoresizingMaskIntoConstraints = false
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 30, height: 26))
        content.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            label.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            label.topAnchor.constraint(equalTo: content.topAnchor),
            label.bottomAnchor.constraint(equalTo: content.bottomAnchor),
        ])
        contentView = content
    }

    func show(title: String, near rect: NSRect) {
        generation += 1
        let currentGeneration = generation
        label.stringValue = title
        setFrameOrigin(NSPoint(x: rect.maxX + 6, y: rect.minY))
        orderFrontRegardless()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
            guard let self, self.generation == currentGeneration else { return }
            self.hide()
        }
    }

    func hide() { orderOut(nil) }
}

private extension Collection {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
