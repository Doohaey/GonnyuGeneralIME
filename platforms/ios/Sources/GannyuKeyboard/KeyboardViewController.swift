import UIKit

final class KeyboardViewController: UIInputViewController {
    private enum KeyboardPage {
        case letters
        case numbers
        case symbols
        case symbolsMore
    }

    private enum KeyRowLayout {
        case reference
        case centered
        case deleteExtended
        case bottom
    }

    private let store = GonnyuAppleRegionStore()
    private var regions: [GonnyuAppleRegion] = []
    private var engine: GonnyuAppleEngine?
    private var regionID: String?
    private var snapshot = GonnyuAppleSnapshot.empty
    private var keyboardPage: KeyboardPage = .letters
    private var englishMode = false
    private var englishShift = false
    private var backspaceTimer: Timer?
    private let preeditLabel = UILabel()
    private let candidateScroll = UIScrollView()
    private let candidateStack = UIStackView()
    private let keyboardStack = UIStackView()
    private weak var referenceKeyButton: UIButton?
    private var pendingWidthConstraints: [NSLayoutConstraint] = []
    private let keyboardPanelColor = UIColor(red: 0.82, green: 0.83, blue: 0.84, alpha: 1)
    private let normalKeyColor = UIColor(red: 1, green: 1, blue: 1, alpha: 1)
    private let actionKeyColor = UIColor(red: 0.72, green: 0.74, blue: 0.76, alpha: 1)
    private let fixedTextColor = UIColor(red: 0.10, green: 0.10, blue: 0.12, alpha: 1)
    private let fixedSecondaryTextColor = UIColor(red: 0.38, green: 0.40, blue: 0.44, alpha: 1)
    private let keySpacing: CGFloat = 6
    private let functionKeyWidthMultiplier: CGFloat = 1.12

    override func viewDidLoad() {
        super.viewDidLoad()
        overrideUserInterfaceStyle = .light
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
        view.backgroundColor = keyboardPanelColor
        let root = UIStackView()
        root.axis = .vertical
        root.spacing = 4
        root.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 6),
            root.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -6),
            root.topAnchor.constraint(equalTo: view.topAnchor, constant: 4),
            root.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -7),
        ])

        preeditLabel.font = .systemFont(ofSize: 10, weight: .regular)
        preeditLabel.textColor = fixedSecondaryTextColor
        preeditLabel.backgroundColor = .clear
        preeditLabel.numberOfLines = 1
        preeditLabel.heightAnchor.constraint(equalToConstant: 16).isActive = true
        root.addArrangedSubview(preeditLabel)

        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.backgroundColor = .clear
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        root.addArrangedSubview(candidateScroll)

        candidateStack.axis = .horizontal
        candidateStack.spacing = 3
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
        NSLayoutConstraint.deactivate(pendingWidthConstraints)
        pendingWidthConstraints.removeAll()
        keyboardStack.arrangedSubviews.forEach {
            keyboardStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        referenceKeyButton = nil
        switch keyboardPage {
        case .letters:
            keyboardStack.addArrangedSubview(keyRow("qwertyuiop".map(String.init), layout: .reference))
            keyboardStack.addArrangedSubview(keyRow("asdfghjkl".map(String.init), layout: .centered))
            keyboardStack.addArrangedSubview(keyRow(
                [englishMode ? "⇧" : "分词"] + "zxcvbnm".map(String.init) + ["⌫"],
                layout: .deleteExtended
            ))
        case .numbers:
            keyboardStack.addArrangedSubview(keyRow(
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["-", "/", ":", ";", "(", ")", "¥", "&", "@", "\""],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["符号", ".", ",", "?", "!", "'", "%", "＋", "⌫"],
                layout: .deleteExtended
            ))
        case .symbols:
            keyboardStack.addArrangedSubview(keyRow(
                ["【", "】", "“", "”", "〈", "〉", "《", "》", "：", "；"],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["，", "、", "。", "？", "！", "…", "—", "～", "·", "／"],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["更多", "（", "）", "[", "]", "{", "}", "#", "⌫"],
                layout: .deleteExtended
            ))
        case .symbolsMore:
            keyboardStack.addArrangedSubview(keyRow(
                ["+", "−", "=", "×", "÷", "<", ">", "^", "~", "_"],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["@", "#", "$", "¥", "€", "£", "&", "*", "\\", "|"],
                layout: .reference
            ))
            keyboardStack.addArrangedSubview(keyRow(
                ["常用", "!", "?", "'", "\"", ":", ";", "／", "⌫"],
                layout: .deleteExtended
            ))
        }
        keyboardStack.addArrangedSubview(keyRow(
            ["🌐", keyboardPage == .letters ? (englishMode ? "中" : "英") : (keyboardPage == .numbers ? "符号" : "123"),
             keyboardPage == .letters ? "123" : "ABC", "空格", englishMode ? "," : "，",
             englishMode ? "." : "。", "⏎"],
            layout: .bottom
        ))
        NSLayoutConstraint.activate(pendingWidthConstraints)
    }

    private func keyRow(_ labels: [String], layout: KeyRowLayout) -> UIView {
        let container = UIView()
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = keySpacing
        row.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(row)
        let buttons = labels.map(makeKeyButton)
        buttons.forEach(row.addArrangedSubview)
        NSLayoutConstraint.activate([
            row.topAnchor.constraint(equalTo: container.topAnchor),
            row.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])

        switch layout {
        case .reference:
            row.distribution = .fillEqually
            NSLayoutConstraint.activate([
                row.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                row.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            ])
            if referenceKeyButton == nil {
                referenceKeyButton = buttons.first
            }
        case .centered:
            constrainToReferenceWidth(buttons)
            NSLayoutConstraint.activate([
                row.centerXAnchor.constraint(equalTo: container.centerXAnchor),
                row.leadingAnchor.constraint(greaterThanOrEqualTo: container.leadingAnchor),
                row.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor),
            ])
        case .deleteExtended:
            constrainToReferenceWidth(Array(buttons.dropLast()))
            buttons.last?.setContentHuggingPriority(.defaultLow, for: .horizontal)
            NSLayoutConstraint.activate([
                row.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                row.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            ])
        case .bottom:
            constrainBottomRow(buttons, labels: labels)
            NSLayoutConstraint.activate([
                row.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                row.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            ])
        }
        return container
    }

    private func makeKeyButton(_ label: String) -> UIButton {
        let button = UIButton(type: .system)
        button.accessibilityIdentifier = label
        if label == "🌐" {
            button.setImage(UIImage(systemName: "globe"), for: .normal)
            button.tintColor = fixedTextColor
            button.accessibilityLabel = "切换输入法"
        } else {
            let title = label == "⏎" ? (englishMode ? "return" : "换行")
                : (englishMode && englishShift && label.count == 1 && label.first?.isLetter == true
                    ? label.uppercased() : label)
            button.setTitle(title, for: .normal)
        }
        button.setTitleColor(fixedTextColor, for: .normal)
        button.titleLabel?.font = .systemFont(
            ofSize: label.count > 2 ? 12 : (keyboardKeyColor(for: label) == actionKeyColor ? 15 : 20),
            weight: .regular
        )
        button.backgroundColor = keyboardKeyColor(for: label)
        button.layer.cornerRadius = 5
        button.layer.cornerCurve = .continuous
        button.layer.shadowColor = UIColor(red: 0.50, green: 0.51, blue: 0.53, alpha: 1).cgColor
        button.layer.shadowOpacity = 0.28
        button.layer.shadowOffset = CGSize(width: 0, height: 1)
        button.layer.shadowRadius = 0
        button.heightAnchor.constraint(equalToConstant: 46).isActive = true
        if label == "⌫" {
            button.addTarget(self, action: #selector(backspacePressed(_:)), for: .touchDown)
            button.addTarget(self, action: #selector(stopBackspaceRepeat), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        } else {
            button.addTarget(self, action: #selector(keyPressed(_:)), for: .touchUpInside)
        }
        return button
    }

    private func constrainToReferenceWidth(_ buttons: [UIButton]) {
        guard let referenceKeyButton else { return }
        buttons.forEach { button in
            pendingWidthConstraints.append(button.widthAnchor.constraint(
                equalTo: referenceKeyButton.widthAnchor,
                multiplier: functionKeyWidth(for: button.accessibilityIdentifier ?? "")
            ))
        }
    }

    private func constrainBottomRow(_ buttons: [UIButton], labels: [String]) {
        guard let referenceKeyButton else { return }
        for (button, label) in zip(buttons, labels) {
            switch label {
            case "空格":
                button.setContentHuggingPriority(.defaultLow, for: .horizontal)
            case "⏎":
                pendingWidthConstraints.append(button.widthAnchor.constraint(equalTo: referenceKeyButton.widthAnchor, multiplier: 1.6))
            default:
                pendingWidthConstraints.append(button.widthAnchor.constraint(
                    equalTo: referenceKeyButton.widthAnchor,
                    multiplier: functionKeyWidth(for: label)
                ))
            }
        }
    }

    private func functionKeyWidth(for label: String) -> CGFloat {
        switch label {
        case "🌐", "英", "中", "123", "ABC", "符号", "更多", "常用", "⇧", "分词":
            return functionKeyWidthMultiplier
        default:
            return 1
        }
    }

    private func keyboardKeyColor(for label: String) -> UIColor {
        switch label {
        case "🌐", "⌫", "⏎", "123", "ABC", "符号", "更多", "常用", "⇧", "分词", "英", "中":
            return actionKeyColor
        default:
            return normalKeyColor
        }
    }

    @objc private func keyPressed(_ sender: UIButton) {
        guard let key = sender.accessibilityIdentifier else { return }
        switch key {
        case "🌐":
            advanceToNextInputMode()
        case "⌫":
            deleteBackward()
        case "英", "中":
            clearComposition()
            englishMode.toggle()
            englishShift = false
            render()
            renderKeyboard()
        case "⇧":
            englishShift.toggle()
            renderKeyboard()
        case "空格":
            handleSpace()
        case "⏎":
            handleEnter()
        case "123":
            englishShift = false
            keyboardPage = .numbers
            renderKeyboard()
        case "符号":
            keyboardPage = .symbols
            renderKeyboard()
        case "更多":
            keyboardPage = .symbolsMore
            renderKeyboard()
        case "常用":
            keyboardPage = .symbols
            renderKeyboard()
        case "ABC":
            keyboardPage = .letters
            renderKeyboard()
        case "分词":
            append("'")
        default:
            if keyboardPage != .letters || ["，", "、", "。", "？", "！", "：", "；", ",", "."].contains(key) {
                insertLiteral(key)
            } else if englishMode {
                textDocumentProxy.insertText(englishShift ? key.uppercased() : key)
                if englishShift {
                    englishShift = false
                    renderKeyboard()
                }
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

    private func insertLiteral(_ key: String) {
        if snapshot.rawInput.isEmpty {
            textDocumentProxy.insertText(key)
        } else if let updated = try? engine?.process(.text(key)) {
            apply(updated)
        }
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
            configuration.baseForegroundColor = fixedTextColor
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 2, leading: 5, bottom: 2, trailing: 5)
            configuration.title = candidate.text
            configuration.subtitle = candidate.annotation.isEmpty ? candidate.reading : candidate.annotation
            configuration.titleLineBreakMode = .byClipping
            configuration.subtitleLineBreakMode = .byClipping
            configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 16)
                return attributes
            }
            configuration.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
                var attributes = $0
                attributes.font = .systemFont(ofSize: 10)
                attributes.foregroundColor = UIColor(red: 0.38, green: 0.40, blue: 0.44, alpha: 1)
                return attributes
            }
            button.configuration = configuration
            button.setContentCompressionResistancePriority(.required, for: .horizontal)
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
