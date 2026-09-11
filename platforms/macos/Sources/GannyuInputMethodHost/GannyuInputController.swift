import AppKit
import Carbon.HIToolbox
import InputMethodKit
import GannyuMacOSSupport

@objc(GannyuInputController)
final class GannyuInputController: IMKInputController {
    private lazy var engine: GannyuEngine? = {
        let env = ProcessInfo.processInfo.environment
        do {
            return try GannyuEngine(
                manifestPath: env["GANNYU_MANIFEST"],
                regionID: env["GANNYU_REGION_ID"]
                    ?? GannyuRegionStore.shared.currentID(manifestPath: env["GANNYU_MANIFEST"])
            )
        } catch {
            return nil
        }
    }()
    private var buffer = ""
    private var bufferCursor = 0
    private var currentCandidates: [GannyuRetrievedCandidate] = []
    private var currentCandidateDisplays: [NSAttributedString] = []
    private var candidateIdentifierToIndex: [Int: Int] = [:]
    private var displayedCandidateIndices: [Int] = []
    private var candidatePage = 0
    private var accumulatedText = ""
    private var accumulatedReading = ""
    private var accumulatedMandarinReading = ""
    private var isActive = false
    private lazy var candidatesWindow: IMKCandidates? = {
        guard let server = server() else {
            return nil
        }
        let window = IMKCandidates(server: server, panelType: kIMKSingleColumnScrollingCandidatePanel)
        window?.setAttributes([
            IMKCandidatesSendServerKeyEventFirst: NSNumber(value: true),
            NSAttributedString.Key.font.rawValue: NSFont.systemFont(ofSize: 18),
        ])
        window?.setSelectionKeys([
            NSNumber(value: kVK_ANSI_1),
            NSNumber(value: kVK_ANSI_2),
            NSNumber(value: kVK_ANSI_3),
            NSNumber(value: kVK_ANSI_4),
            NSNumber(value: kVK_ANSI_5),
            NSNumber(value: kVK_ANSI_6),
            NSNumber(value: kVK_ANSI_7),
            NSNumber(value: kVK_ANSI_8),
            NSNumber(value: kVK_ANSI_9),
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

    @objc(inputText:client:)
    override func inputText(_ string: String!, client sender: Any!) -> Bool {
        return processText(string, client: sender)
    }

    @objc(didCommandBySelector:client:)
    override func didCommand(by selector: Selector!, client sender: Any!) -> Bool {
        guard let selector else {
            return false
        }
        switch NSStringFromSelector(selector) {
        case "deleteBackward:":
            return handleDelete(client: sender)
        case "moveLeft:", "moveBackward:":
            guard hasComposition else { return false }
            return selectCandidate(by: -1)
        case "moveRight:", "moveForward:":
            guard hasComposition else { return false }
            return selectCandidate(by: 1)
        case "cancelOperation:":
            guard hasComposition else { return false }
            clearComposition(client: sender)
            return true
        case "insertNewline:", "insertLineBreak:":
            return commitRawBuffer(client: sender)
        case "insertTab:":
            guard hasComposition else { return false }
            return commitSelectedCandidate(client: sender)
        default:
            return false
        }
    }

    private func processText(_ string: String!, client sender: Any!) -> Bool {
        guard let string else {
            return false
        }
        let normalized = string.lowercased()
        if normalized.isEmpty {
            return false
        }
        if normalized == " " {
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
            return processSymbol(normalized, client: sender)
        }
        insertTextIntoBuffer(normalized)
        refreshCandidates(client: sender)
        return true
    }

    @objc(composedString:)
    override func composedString(_ sender: Any!) -> Any! {
        guard !buffer.isEmpty else {
            return ""
        }
        return currentPreeditDisplay()
    }

    @objc(originalString:)
    override func originalString(_ sender: Any!) -> NSAttributedString! {
        NSAttributedString(string: buffer)
    }

    @objc(selectionRange)
    override func selectionRange() -> NSRange {
        let prefix = String(buffer.prefix(bufferCursor))
        let prefixDisplay = (try? engine?.formatPreedit(prefix)) ?? prefix
        return NSRange(location: nsLength(of: prefixDisplay), length: 0)
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
        currentCandidateDisplays
    }

    @objc(menu)
    override func menu() -> NSMenu! {
        let menu = NSMenu(title: "Gonnyu")
        let manifest = ProcessInfo.processInfo.environment["GANNYU_MANIFEST"]
        for region in availableRegions {
            let item = NSMenuItem(title: region.nameZh, action: #selector(selectRegionFromMenu(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = region.id
            item.state = region.id == GannyuRegionStore.shared.currentID(manifestPath: manifest) ? .on : .off
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
        guard let index = candidateIndex(forCandidateSelection: candidateString) else {
            return
        }
        commitCandidate(at: index, client: client())
    }

    @objc(candidateSelectionChanged:)
    override func candidateSelectionChanged(_ candidateString: NSAttributedString!) {
        guard let index = candidateIndex(forCandidateSelection: candidateString) else {
            hideAnnotation()
            return
        }
        showAnnotation(for: currentCandidates[index])
    }

    private func refreshCandidates(client sender: Any!) {
        bufferCursor = min(max(bufferCursor, 0), buffer.count)
        do {
            currentCandidates = try engine?.retrieveCandidates(buffer) ?? []
        } catch {
            currentCandidates = []
        }
        currentCandidateDisplays = currentCandidates.map(candidateDisplay(for:))
        candidateIdentifierToIndex = [:]
        candidatePage = 0
        if !hasComposition {
            clearMarkedText(client: sender)
            hideCandidates()
            return
        }
        updateComposition(client: sender)
        if currentCandidates.isEmpty {
            hideCandidates()
            return
        }
        showCandidatePage()
        if let first = currentCandidates.first {
            showAnnotation(for: first)
        }
    }

    private func commitCandidate(at index: Int, client sender: Any!) {
        guard currentCandidates.indices.contains(index) else { return }
        let candidate = currentCandidates[index]
        recordCandidateSelection(candidate)
        let count = max(0, min(candidate.consumedBytes, buffer.utf8.count))
        if count > 0 && count < buffer.utf8.count {
            insertCommittedText(candidate.text, client: sender)
            buffer = String(decoding: buffer.utf8.dropFirst(count), as: UTF8.self)
            bufferCursor = buffer.count
            refreshCandidates(client: sender)
            return
        }
        commitText(candidate.text, client: sender)
    }

    private func commitText(_ text: String, client sender: Any!, saveLearning: Bool = true) {
        guard let client = sender as? NSTextInputClient else {
            buffer = ""
            bufferCursor = 0
            currentCandidates = []
            currentCandidateDisplays = []
            candidateIdentifierToIndex = [:]
            hideCandidates()
            return
        }
        client.insertText(text, replacementRange: currentReplacementRange(for: client))
        if saveLearning {
            saveAccumulatedUserWord()
        } else {
            clearLearningState()
        }
        buffer = ""
        bufferCursor = 0
        currentCandidates = []
        currentCandidateDisplays = []
        candidateIdentifierToIndex = [:]
        displayedCandidateIndices = []
        clearMarkedText(client: client)
        hideCandidates()
    }

    private var availableRegions: [GannyuRegion] {
        let manifest = ProcessInfo.processInfo.environment["GANNYU_MANIFEST"]
        return (try? GannyuEngine.availableRegions(manifestPath: manifest)) ?? []
    }

    private func clearComposition(client sender: Any!) {
        buffer = ""
        bufferCursor = 0
        currentCandidates = []
        currentCandidateDisplays = []
        candidateIdentifierToIndex = [:]
        displayedCandidateIndices = []
        clearLearningState()
        clearMarkedText(client: sender)
        hideCandidates()
    }

    private func clearMarkedText(client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            return
        }
        client.unmarkText()
    }

    private func updateComposition(client sender: Any!) {
        guard let client = sender as? NSTextInputClient, hasComposition else {
            return
        }
        let display = currentPreeditDisplay()
        client.setMarkedText(
            display,
            selectedRange: NSRange(location: nsLength(of: display), length: 0),
            replacementRange: currentReplacementRange(for: client)
        )
    }

    private func shouldAppend(_ string: String) -> Bool {
        string.unicodeScalars.allSatisfy {
            CharacterSet.lowercaseLetters.contains($0)
                || CharacterSet.uppercaseLetters.contains($0)
                || $0 == "'"
        }
    }

    private func processSymbol(_ symbol: String, client sender: Any!) -> Bool {
        guard !symbol.isEmpty else { return false }
        if hasComposition {
            if let selected = selectedCandidateIndex() ?? firstCandidateIndexOnPage() {
                commitCandidate(at: selected, client: sender)
            } else {
                _ = commitRawBuffer(client: sender)
            }
        }
        let output = chinesePunctuation(for: symbol) ?? symbol
        guard let client = sender as? NSTextInputClient else { return false }
        client.insertText(output, replacementRange: client.selectedRange())
        return true
    }

    private func chinesePunctuation(for symbol: String) -> String? {
        [
            ",": "，", ".": "。", "\\": "、", ";": "；", ":": "：", "?": "？", "!": "！",
            "(": "（", ")": "）", "[": "【", "]": "】", "<": "《", ">": "》", "\"": "“",
            "~": "～", "-": "－",
        ][symbol]
    }

    private var hasComposition: Bool {
        !buffer.isEmpty
    }

    private func commitRawBuffer(client sender: Any!) -> Bool {
        guard hasComposition else { return false }
        commitText(buffer, client: sender, saveLearning: false)
        return true
    }

    private func recordCandidateSelection(_ candidate: GannyuRetrievedCandidate) {
        engine?.boostUserWord(candidate.text)
        accumulatedText += candidate.text
        if let reading = candidate.reading, !reading.isEmpty {
            accumulatedReading += accumulatedReading.isEmpty ? reading : " \(reading)"
        }
        if let reading = candidate.mandarinReading, !reading.isEmpty {
            accumulatedMandarinReading += accumulatedMandarinReading.isEmpty ? reading : " \(reading)"
        }
    }

    private func saveAccumulatedUserWord() {
        engine?.saveUserWord(
            accumulatedText,
            reading: accumulatedReading,
            mandarinReading: accumulatedMandarinReading.isEmpty ? nil : accumulatedMandarinReading
        )
        clearLearningState()
    }

    private func clearLearningState() {
        accumulatedText = ""
        accumulatedReading = ""
        accumulatedMandarinReading = ""
    }

    private func handleDelete(client sender: Any!) -> Bool {
        guard hasComposition else {
            return false
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
        let consumedBytes = firstCandidateIndexOnPage().map { currentCandidates[$0].consumedBytes } ?? 0
        return (try? engine?.formatPreedit(buffer, consumedBytes: consumedBytes)) ?? buffer
    }

    private func currentReplacementRange(for client: NSTextInputClient) -> NSRange {
        let markedRange = client.markedRange()
        if markedRange.location != NSNotFound {
            return markedRange
        }
        return client.selectedRange()
    }

    private func insertCommittedText(_ text: String, client sender: Any!) {
        guard let client = sender as? NSTextInputClient else {
            return
        }
        client.insertText(text, replacementRange: currentReplacementRange(for: client))
    }

    private func showCandidatePage() {
        displayedCandidateIndices = candidatePageRange()
        candidatesWindow?.setCandidateData(displayedCandidateIndices.map { currentCandidateDisplays[$0] })
        candidatesWindow?.update()
        candidatesWindow?.show(kIMKLocateCandidatesBelowHint)
        rebuildCandidateIdentifierMap()
    }

    private func candidatePageRange() -> [Int] {
        let start = candidatePage * 9
        guard start < currentCandidates.count else { return [] }
        return Array(start..<min(start + 9, currentCandidates.count))
    }

    private func changeCandidatePage(by delta: Int, client sender: Any!) -> Bool {
        guard hasComposition, !currentCandidates.isEmpty else { return false }
        let next = candidatePage + delta
        guard next >= 0, next < (currentCandidates.count + 8) / 9 else { return true }
        candidatePage = next
        showCandidatePage()
        updateComposition(client: sender)
        return true
    }

    private func selectCandidate(by delta: Int) -> Bool {
        guard hasComposition, !displayedCandidateIndices.isEmpty, let candidatesWindow else { return false }
        let current = selectedCandidateIndex() ?? displayedCandidateIndices[0]
        guard let offset = displayedCandidateIndices.firstIndex(of: current) else { return false }
        let next = (offset + delta + displayedCandidateIndices.count) % displayedCandidateIndices.count
        let identifier = candidatesWindow.candidateStringIdentifier(currentCandidateDisplays[displayedCandidateIndices[next]])
        guard identifier != NSNotFound else { return false }
        return candidatesWindow.selectCandidate(withIdentifier: identifier)
    }

    private func firstCandidateIndexOnPage() -> Int? {
        displayedCandidateIndices.first
    }

    private func rebuildCandidateIdentifierMap() {
        guard let candidatesWindow else {
            candidateIdentifierToIndex = [:]
            return
        }
        candidateIdentifierToIndex = Dictionary(
            uniqueKeysWithValues: displayedCandidateIndices.compactMap { index in
                let display = currentCandidateDisplays[index]
                let identifier = candidatesWindow.candidateStringIdentifier(display)
                guard identifier != NSNotFound else {
                    return nil
                }
                return (identifier, index)
            }
        )
    }

    private func hideCandidates() {
        hideAnnotation()
        candidatesWindow?.hide()
    }

    private func nsLength(of string: String) -> Int {
        (string as NSString).length
    }

    private func candidateDisplay(for candidate: GannyuRetrievedCandidate) -> NSAttributedString {
        let display = NSMutableAttributedString(
            string: candidate.text,
            attributes: [
                .font: NSFont.systemFont(ofSize: 18),
                .foregroundColor: NSColor.labelColor,
            ]
        )
        if let annotation = annotationText(for: candidate), !annotation.isEmpty {
            display.append(NSAttributedString(
                string: "\n\(annotation.replacingOccurrences(of: "\n", with: " "))",
                attributes: [
                    .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize),
                    .foregroundColor: NSColor.secondaryLabelColor,
                ]
            ))
        }
        return display
    }

    private func showAnnotation(for _: GannyuRetrievedCandidate) {
        hideAnnotation()
    }

    private func hideAnnotation() {
        candidatesWindow?.hideChild()
    }

    private func annotationText(for candidate: GannyuRetrievedCandidate) -> String? {
        let annotation = candidate.annotation.trimmingCharacters(in: .whitespacesAndNewlines)
        if !annotation.isEmpty {
            return annotation
        }
        return candidate.reading?.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func candidateIndex(for string: String) -> Int? {
        guard string.count == 1, let scalar = string.unicodeScalars.first else {
            return nil
        }
        guard CharacterSet.decimalDigits.contains(scalar) else {
            return nil
        }
        if scalar == "0" {
            return displayedCandidateIndices.count > 9 ? displayedCandidateIndices[9] : nil
        }
        guard let value = Int(string), value > 0 else {
            return nil
        }
        let offset = value - 1
        guard displayedCandidateIndices.indices.contains(offset) else { return nil }
        return displayedCandidateIndices[offset]
    }

    private func candidateLineNumber(forKeyCode keyCode: Int) -> Int? {
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

    private func commitSelectedCandidate(client sender: Any!) -> Bool {
        guard let index = selectedCandidateIndex() else {
            return false
        }
        commitCandidate(at: index, client: sender)
        return true
    }

    private func commitCandidateAtDisplayedLine(_ lineNumber: Int, client sender: Any!) -> Bool {
        guard
            let candidatesWindow,
            candidatesWindow.isVisible(),
            let index = candidateIndexForDisplayedLine(lineNumber)
        else {
            if currentCandidates.indices.contains(lineNumber) {
                commitCandidate(at: lineNumber, client: sender)
                return true
            }
            return false
        }
        let identifier = candidatesWindow.candidateIdentifier(atLineNumber: lineNumber)
        guard identifier != NSNotFound else {
            return false
        }
        _ = candidatesWindow.selectCandidate(withIdentifier: identifier)
        commitCandidate(at: index, client: sender)
        return true
    }

    private func candidateIndexForDisplayedLine(_ lineNumber: Int) -> Int? {
        guard
            let candidatesWindow,
            candidatesWindow.isVisible()
        else {
            return currentCandidates.indices.contains(lineNumber) ? lineNumber : nil
        }
        let identifier = candidatesWindow.candidateIdentifier(atLineNumber: lineNumber)
        guard identifier != NSNotFound else {
            return nil
        }
        return candidateIndex(forCandidateIdentifier: identifier)
    }

    private func candidateIndex(forCandidateIdentifier identifier: Int) -> Int? {
        guard identifier != NSNotFound else {
            return nil
        }
        if let index = candidateIdentifierToIndex[identifier] {
            return index
        }
        return nil
    }

    private func selectedCandidateIndex() -> Int? {
        guard let candidatesWindow else {
            return nil
        }
        let identifier = candidatesWindow.selectedCandidate()
        if let index = candidateIndex(forCandidateIdentifier: identifier) {
            return index
        }
        return candidateIndex(forDisplayedCandidateText: candidatesWindow.selectedCandidateString()?.string)
    }

    private func candidateIndex(forCandidateSelection candidateString: NSAttributedString?) -> Int? {
        if let index = selectedCandidateIndex() {
            return index
        }
        return candidateIndex(forDisplayedCandidateText: candidateString?.string)
    }

    private func candidateIndex(forDisplayedCandidateText text: String?) -> Int? {
        guard let text else {
            return nil
        }
        if let index = displayedCandidateIndices.first(where: { currentCandidateDisplays[$0].string == text }) {
            return index
        }
        return currentCandidates.firstIndex(where: { $0.text == text })
    }

    @objc private func regionDidChange() {
        engine = nil
        buffer = ""
        bufferCursor = 0
        currentCandidates = []
        currentCandidateDisplays = []
        candidateIdentifierToIndex = [:]
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
        guard let raw = item?.representedObject as? String else { return }
        _ = GannyuRegionStore.shared.select(
            raw,
            manifestPath: ProcessInfo.processInfo.environment["GANNYU_MANIFEST"]
        )
    }
}
