import UIKit

final class KeyboardViewController: UIInputViewController {
    private let store = GonnyuAppleRegionStore()
    private var regions: [GonnyuAppleRegion] = []
    private var engine: GonnyuAppleEngine?
    private var regionID: String?
    private var snapshot = GonnyuAppleSnapshot.empty
    private var symbolPage = false
    private var englishMode = false
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
            name: GonnyuAppleRegionStore.didChange,
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
        view.backgroundColor = .systemGray6
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
        preeditLabel.backgroundColor = .clear
        preeditLabel.numberOfLines = 1
        root.addArrangedSubview(preeditLabel)

        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.backgroundColor = .clear
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
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
            ? ["🌐", "拼", "（", "）", "空格", "⌫", "⏎"]
            : ["🌐", englishMode ? "中" : "英", "123", "，", "空格", "。", "⏎"]))
    }

    private func keyRow(_ labels: [String]) -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 4
        row.distribution = .fillEqually
        for label in labels {
            let button = UIButton(type: .system)
            button.setTitle(label, for: .normal)
            button.setTitleColor(.label, for: .normal)
            button.titleLabel?.font = .systemFont(ofSize: 17, weight: .regular)
            button.backgroundColor = keyboardKeyColor(for: label)
            button.layer.cornerRadius = 5
            button.layer.cornerCurve = .continuous
            button.layer.shadowColor = UIColor.systemGray.cgColor
            button.layer.shadowOpacity = 0.28
            button.layer.shadowOffset = CGSize(width: 0, height: 1)
            button.layer.shadowRadius = 0
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

    private func keyboardKeyColor(for label: String) -> UIColor {
        switch label {
        case "🌐", "⌫", "⏎", "123", "拼", "分词", "英", "中":
            return .systemGray4
        default:
            return .systemBackground
        }
    }

    @objc private func keyPressed(_ sender: UIButton) {
        guard let key = sender.currentTitle else { return }
        switch key {
        case "🌐":
            advanceToNextInputMode()
        case "⌫":
            deleteBackward()
        case "英", "中":
            clearComposition()
            englishMode.toggle()
            render()
            renderKeyboard()
        case "空格":
            handleSpace()
        case "⏎":
            handleEnter()
        case "123":
            clearComposition()
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
            apply(try? engine?.process(.text(key)))
        default:
            if symbolPage {
                textDocumentProxy.insertText(key)
            } else if englishMode {
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
        apply(try? engine?.process(.text(text.lowercased())))
    }

    private func deleteBackward() {
        if snapshot.rawInput.isEmpty {
            textDocumentProxy.deleteBackward()
        } else {
            apply(try? engine?.process(.backspace))
        }
    }

    private func render() {
        preeditLabel.text = snapshot.preedit.isEmpty ? nil : snapshot.preedit
        candidateStack.arrangedSubviews.forEach {
            candidateStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        for (index, candidate) in snapshot.candidates.enumerated() {
            let button = UIButton(type: .system)
            var configuration = UIButton.Configuration.plain()
            configuration.baseForegroundColor = index == 0 ? .systemBlue : .label
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 2, leading: 8, bottom: 2, trailing: 8)
            configuration.title = candidate.text
            configuration.subtitle = candidate.annotation.isEmpty ? candidate.reading : candidate.annotation
            configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 16)
                return attributes
            }
            configuration.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 10)
                attributes.foregroundColor = .secondaryLabel
                return attributes
            }
            button.configuration = configuration
            button.accessibilityLabel = candidate.text
            button.accessibilityHint = candidate.annotation
            button.tag = candidate.globalIndex
            button.addTarget(self, action: #selector(candidatePressed(_:)), for: .touchUpInside)
            candidateStack.addArrangedSubview(button)
        }
    }

    @objc private func candidatePressed(_ sender: UIButton) {
        commitCandidate(at: sender.tag)
    }

    private func commitCandidate(at index: Int) {
        apply(try? engine?.selectCandidate(globalIndex: index))
    }

    private func handleSpace() {
        if snapshot.rawInput.isEmpty {
            textDocumentProxy.insertText(" ")
        } else {
            apply(try? engine?.process(.space))
        }
    }

    private func handleEnter() {
        if snapshot.rawInput.isEmpty {
            textDocumentProxy.insertText("\n")
        } else {
            apply(try? engine?.process(.enter))
        }
    }

    private func clearComposition() {
        if let updated = try? engine?.clearComposition() {
            apply(updated)
            return
        }
        snapshot = .empty
        render()
    }

    private func apply(_ updated: GonnyuAppleSnapshot?) {
        guard let updated else { return }
        if let commit = updated.commitText, !commit.isEmpty {
            textDocumentProxy.insertText(commit)
        }
        snapshot = updated
        render()
    }

    @objc private func regionDidChange() {
        reloadRegionIfNeeded(force: true)
    }

    private func reloadRegionIfNeeded(force: Bool) {
        guard let loadedRegions = try? GonnyuAppleEngine.regions() else { return }
        regions = loadedRegions
        let pendingResets = store.pendingUserDataResetRegionIDs()
        if !pendingResets.isEmpty {
            engine = nil
            regionID = nil
            do {
                for pendingRegion in pendingResets {
                    let resetEngine = try GonnyuAppleEngine(
                        regionID: pendingRegion,
                        userDataDirectory: store.userDataDirectory
                    )
                    _ = try resetEngine.clearUserData()
                }
                try store.finishPendingUserDataResets()
            } catch {
                // Keep the request files so the keyboard retries on its next activation.
            }
        }
        guard let selected = store.currentID(in: regions), force || selected != regionID else { return }
        engine = try? GonnyuAppleEngine(
            regionID: selected,
            userDataDirectory: store.userDataDirectory
        )
        regionID = selected
        snapshot = (try? engine?.snapshot()) ?? .empty
        render()
    }
}
