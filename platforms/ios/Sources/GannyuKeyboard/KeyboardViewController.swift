import UIKit

final class KeyboardViewController: UIInputViewController {
    private let store = GannyuAppleRegionStore()
    private var regions: [GannyuAppleRegion] = []
    private var engine: GannyuAppleEngine?
    private var regionID: String?
    private var buffer = ""
    private var candidates: [GannyuAppleCandidate] = []
    private var accumulatedText = ""
    private var accumulatedReadings: [String] = []
    private var accumulatedMandarinReadings: [String] = []
    private var symbolPage = false
    private var backspaceTimer: Timer?
    private let preeditLabel = UILabel()
    private let candidateScroll = UIScrollView()
    private let candidateStack = UIStackView()
    private let keyboardStack = UIStackView()

    override func viewDidLoad() {
        super.viewDidLoad()
        buildKeyboard()
        reloadRegionIfNeeded(force: true)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(regionDidChange),
            name: GannyuAppleRegionStore.didChange,
            object: nil
        )
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        reloadRegionIfNeeded(force: false)
    }

    deinit {
        stopBackspaceRepeat()
        NotificationCenter.default.removeObserver(self)
    }

    private func buildKeyboard() {
        let root = UIStackView()
        root.axis = .vertical
        root.spacing = 6
        root.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
            root.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
            root.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
            root.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -8),
        ])

        preeditLabel.font = .preferredFont(forTextStyle: .body)
        preeditLabel.textColor = .secondaryLabel
        preeditLabel.numberOfLines = 1
        root.addArrangedSubview(preeditLabel)

        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 48).isActive = true
        root.addArrangedSubview(candidateScroll)

        candidateStack.axis = .horizontal
        candidateStack.spacing = 6
        candidateStack.alignment = .fill
        candidateStack.translatesAutoresizingMaskIntoConstraints = false
        candidateScroll.addSubview(candidateStack)
        NSLayoutConstraint.activate([
            candidateStack.leadingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.leadingAnchor),
            candidateStack.trailingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.trailingAnchor),
            candidateStack.topAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.topAnchor),
            candidateStack.bottomAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.bottomAnchor),
            candidateStack.heightAnchor.constraint(equalTo: candidateScroll.frameLayoutGuide.heightAnchor),
        ])

        keyboardStack.axis = .vertical
        keyboardStack.spacing = 6
        root.addArrangedSubview(keyboardStack)
        renderKeyboard()
    }

    private func renderKeyboard() {
        keyboardStack.arrangedSubviews.forEach {
            keyboardStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        let rows: [[String]]
        if symbolPage {
            rows = [
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
                ["【", "】", "“", "”", "〈", "〉", "《", "》", "：", "；"],
                ["，", "、", "。", "？", "！", "…", "—", "～", "·", "／"],
            ]
        } else {
            rows = [
                "qwertyuiop".map(String.init),
                "asdfghjkl".map(String.init),
                ["分词"] + "zxcvbnm".map(String.init) + ["⌫"],
            ]
        }
        for row in rows {
            keyboardStack.addArrangedSubview(keyRow(row))
        }
        keyboardStack.addArrangedSubview(keyRow(symbolPage
            ? ["🌐", "拼", "（", "）", "空格", "“", "⌫", "⏎"]
            : ["🌐", "123", "，", "空格", "。", "⏎"]))
    }

    private func keyRow(_ labels: [String]) -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 4
        row.distribution = .fillEqually
        for label in labels {
            let button = UIButton(type: .system)
            button.setTitle(label, for: .normal)
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
            if label == "⌫" {
                button.addTarget(self, action: #selector(backspacePressed(_:)), for: .touchDown)
                button.addTarget(self, action: #selector(stopBackspaceRepeat), for: [.touchUpInside, .touchUpOutside, .touchCancel])
            } else {
                button.addTarget(self, action: #selector(keyPressed(_:)), for: .touchUpInside)
            }
            row.addArrangedSubview(button)
        }
        return row
    }

    @objc private func keyPressed(_ sender: UIButton) {
        guard let key = sender.currentTitle else { return }
        switch key {
        case "🌐":
            advanceToNextInputMode()
        case "⌫":
            deleteBackward()
        case "空格":
            handleSpace()
        case "⏎":
            commitRawBuffer()
            textDocumentProxy.insertText("\n")
        case "123":
            // Clear any in-progress composition so the default candidate (or any
            // accumulated input) is not accidentally committed when the first
            // symbol/digit is pressed on the numeric sub-keyboard.
            buffer = ""
            candidates = []
            clearLearningState()
            render()
            symbolPage = true
            renderKeyboard()
        case "ABC":
            symbolPage = false
            renderKeyboard()
        case "拼":
            symbolPage = false
            renderKeyboard()
        case "分词":
            append("'")
        case "，", "、", "。", "？", "！", "：", "；":
            commitComposingIfNeeded()
            textDocumentProxy.insertText(key)
        default:
            if symbolPage {
                textDocumentProxy.insertText(key)
            } else {
                append(key)
            }
        }
    }

    @objc private func backspacePressed(_ sender: UIButton) {
        deleteBackward()
        backspaceTimer = Timer.scheduledTimer(withTimeInterval: 0.38, repeats: false) { [weak self] _ in
            guard let self else { return }
            self.backspaceTimer = Timer.scheduledTimer(withTimeInterval: 0.055, repeats: true) { [weak self] _ in
                self?.deleteBackward()
            }
        }
    }

    @objc private func stopBackspaceRepeat() {
        backspaceTimer?.invalidate()
        backspaceTimer = nil
    }

    private func append(_ text: String) {
        buffer += text.lowercased()
        refreshComposition()
    }

    private func deleteBackward() {
        if buffer.isEmpty {
            textDocumentProxy.deleteBackward()
        } else {
            buffer.removeLast()
            refreshComposition()
        }
    }

    private func refreshComposition() {
        candidates = (try? engine?.candidates(for: buffer)) ?? []
        render()
    }

    private func render() {
        preeditLabel.text = buffer.isEmpty ? nil : ((try? engine?.formatPreedit(buffer)) ?? buffer)
        candidateStack.arrangedSubviews.forEach {
            candidateStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        for (index, candidate) in candidates.enumerated() {
            let button = UIButton(type: .system)
            var configuration = UIButton.Configuration.plain()
            configuration.title = candidate.text
            configuration.subtitle = candidate.annotation.isEmpty ? candidate.reading : candidate.annotation
            configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 18)
                return attributes
            }
            configuration.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 11)
                attributes.foregroundColor = .secondaryLabel
                return attributes
            }
            button.configuration = configuration
            button.accessibilityLabel = candidate.text
            button.accessibilityHint = candidate.annotation
            button.tag = index
            button.addTarget(self, action: #selector(candidatePressed(_:)), for: .touchUpInside)
            candidateStack.addArrangedSubview(button)
        }
    }

    @objc private func candidatePressed(_ sender: UIButton) {
        commitCandidate(at: sender.tag)
    }

    private func commitCandidate(at index: Int) {
        guard candidates.indices.contains(index) else {
            return
        }
        let candidate = candidates[index]
        engine?.boost(candidate.text)
        recordSelection(candidate)
        textDocumentProxy.insertText(candidate.text)
        let consumed = max(0, min(candidate.consumedBytes, buffer.utf8.count))
        buffer = consumed < buffer.utf8.count
            ? String(decoding: buffer.utf8.dropFirst(consumed), as: UTF8.self)
            : ""
        if buffer.isEmpty {
            saveAccumulatedUserWord()
        }
        refreshComposition()
    }

    private func commitRawBuffer() {
        guard !buffer.isEmpty else { return }
        textDocumentProxy.insertText(buffer)
        buffer = ""
        clearLearningState()
        refreshComposition()
    }

    private func handleSpace() {
        if buffer.isEmpty {
            textDocumentProxy.insertText(" ")
        } else {
            commitCandidate(at: 0)
        }
    }

    private func commitComposingIfNeeded() {
        guard !buffer.isEmpty else { return }
        if candidates.isEmpty {
            commitRawBuffer()
        } else {
            commitCandidate(at: 0)
        }
    }

    private func recordSelection(_ candidate: GannyuAppleCandidate) {
        accumulatedText += candidate.text
        if let reading = candidate.reading, !reading.isEmpty {
            accumulatedReadings.append(reading)
        }
        if let reading = candidate.mandarinReading, !reading.isEmpty {
            accumulatedMandarinReadings.append(reading)
        }
    }

    private func saveAccumulatedUserWord() {
        engine?.saveUserWord(
            accumulatedText,
            reading: accumulatedReadings.joined(separator: " "),
            mandarinReading: accumulatedMandarinReadings.isEmpty
                ? nil
                : accumulatedMandarinReadings.joined(separator: " ")
        )
        clearLearningState()
    }

    private func clearLearningState() {
        accumulatedText = ""
        accumulatedReadings = []
        accumulatedMandarinReadings = []
    }

    @objc private func regionDidChange() {
        reloadRegionIfNeeded(force: true)
    }

    private func reloadRegionIfNeeded(force: Bool) {
        guard let loadedRegions = try? GannyuAppleEngine.regions() else { return }
        regions = loadedRegions
        guard let selected = store.currentID(in: regions), force || selected != regionID else { return }
        engine = try? GannyuAppleEngine(
            regionID: selected,
            userDataDirectory: store.userDataDirectory
        )
        regionID = selected
        buffer = ""
        clearLearningState()
        refreshComposition()
    }
}
