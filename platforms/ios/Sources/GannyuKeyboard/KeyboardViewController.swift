import UIKit

private final class KeyPreviewView: UIView {
    private let label = UILabel()
    private let bubbleLayer = CAShapeLayer()

    var text: String? {
        get { label.text }
        set { label.text = newValue }
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        isUserInteractionEnabled = false
        backgroundColor = .clear
        layer.shadowColor = UIColor.black.cgColor
        layer.shadowOpacity = 0.18
        layer.shadowOffset = CGSize(width: 0, height: 2)
        layer.shadowRadius = 5
        layer.insertSublayer(bubbleLayer, at: 0)
        updateColors()
        label.font = .systemFont(ofSize: 28, weight: .bold)
        label.textColor = UIColor(red: 0.10, green: 0.10, blue: 0.12, alpha: 1)
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor),
            label.trailingAnchor.constraint(equalTo: trailingAnchor),
            label.topAnchor.constraint(equalTo: topAnchor),
            label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -9),
        ])
    }

    required init?(coder: NSCoder) { nil }

    override func layoutSubviews() {
        super.layoutSubviews()
        let body = CGRect(x: 0, y: 0, width: bounds.width, height: bounds.height - 9)
        let path = UIBezierPath(roundedRect: body, cornerRadius: 8)
        let arrow = UIBezierPath()
        arrow.move(to: CGPoint(x: bounds.midX - 8, y: body.maxY - 1))
        arrow.addLine(to: CGPoint(x: bounds.midX, y: bounds.maxY))
        arrow.addLine(to: CGPoint(x: bounds.midX + 8, y: body.maxY - 1))
        arrow.close()
        path.append(arrow)
        bubbleLayer.frame = bounds
        bubbleLayer.path = path.cgPath
        layer.shadowPath = path.cgPath
    }

    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        if traitCollection.hasDifferentColorAppearance(comparedTo: previousTraitCollection) {
            updateColors()
        }
    }

    private func updateColors() {
        bubbleLayer.fillColor = UIColor.secondarySystemBackground.resolvedColor(with: traitCollection).cgColor
        label.textColor = .label
    }
}

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
    private let candidateRow = UIStackView()
    private let candidateScroll = UIScrollView()
    private let candidateStack = UIStackView()
    private let candidateExpandButton = UIButton(type: .system)
    private let candidateExpandedScroll = UIScrollView()
    private let candidateExpandedStack = UIStackView()
    private let candidateExpandedCloseButton = UIButton(type: .system)
    private var candidateExpanded = false
    private var expandedCandidates: [GonnyuAppleCandidate] = []
    private var expandedLayoutWidth: CGFloat = 0
    private let keyboardStack = UIStackView()
    private let keyPreviewView = KeyPreviewView(frame: .zero)
    private weak var referenceKeyButton: UIButton?
    private var pendingWidthConstraints: [NSLayoutConstraint] = []
    private let keyboardPanelColor = UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(red: 0.12, green: 0.13, blue: 0.15, alpha: 1)
            : UIColor(red: 0.82, green: 0.83, blue: 0.84, alpha: 1)
    }
    private let normalKeyColor = UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(red: 0.20, green: 0.22, blue: 0.25, alpha: 1)
            : .white
    }
    private let actionKeyColor = UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(red: 0.33, green: 0.35, blue: 0.39, alpha: 1)
            : UIColor(red: 0.72, green: 0.74, blue: 0.76, alpha: 1)
    }
    private let fixedTextColor = UIColor.label
    private let fixedSecondaryTextColor = UIColor.secondaryLabel
    private let keySpacing: CGFloat = 6
    private let functionKeyWidthMultiplier: CGFloat = 1.12

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

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // A keyboard extension can be dismissed while a delete key is held.
        // Never let its repeat timer survive that transition.
        stopBackspaceRepeat()
        hideKeyPreview()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if candidateExpanded && candidateExpandedScroll.bounds.width != expandedLayoutWidth {
            renderExpandedCandidates()
        }
    }

    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        if traitCollection.hasDifferentColorAppearance(comparedTo: previousTraitCollection) {
            renderKeyboard()
            render()
        }
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
        keyPreviewView.isHidden = true
        view.addSubview(keyPreviewView)

        preeditLabel.font = .systemFont(ofSize: 12, weight: .bold)
        preeditLabel.textColor = fixedTextColor
        preeditLabel.backgroundColor = .clear
        preeditLabel.numberOfLines = 1
        preeditLabel.heightAnchor.constraint(equalToConstant: 16).isActive = true
        root.addArrangedSubview(preeditLabel)

        candidateRow.axis = .horizontal
        candidateRow.spacing = 3
        candidateRow.alignment = .fill
        candidateRow.heightAnchor.constraint(equalToConstant: 39).isActive = true
        root.addArrangedSubview(candidateRow)

        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.backgroundColor = .clear
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateRow.addArrangedSubview(candidateScroll)

        candidateExpandButton.setTitle("⌄", for: .normal)
        candidateExpandButton.setTitleColor(fixedTextColor, for: .normal)
        candidateExpandButton.titleLabel?.font = .systemFont(ofSize: 17, weight: .medium)
        candidateExpandButton.backgroundColor = actionKeyColor
        candidateExpandButton.layer.cornerRadius = 5
        candidateExpandButton.widthAnchor.constraint(equalToConstant: 32).isActive = true
        candidateExpandButton.addTarget(self, action: #selector(toggleCandidateExpansion), for: .touchUpInside)
        candidateRow.addArrangedSubview(candidateExpandButton)

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

        candidateExpandedScroll.showsVerticalScrollIndicator = true
        candidateExpandedScroll.backgroundColor = keyboardPanelColor
        candidateExpandedScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateExpandedScroll.isHidden = true
        view.addSubview(candidateExpandedScroll)

        candidateExpandedCloseButton.setTitle("⌃", for: .normal)
        candidateExpandedCloseButton.setTitleColor(fixedTextColor, for: .normal)
        candidateExpandedCloseButton.backgroundColor = actionKeyColor
        candidateExpandedCloseButton.layer.cornerRadius = 5
        candidateExpandedCloseButton.translatesAutoresizingMaskIntoConstraints = false
        candidateExpandedCloseButton.addTarget(self, action: #selector(toggleCandidateExpansion), for: .touchUpInside)
        view.addSubview(candidateExpandedCloseButton)

        candidateExpandedStack.axis = .vertical
        candidateExpandedStack.spacing = 3
        candidateExpandedStack.translatesAutoresizingMaskIntoConstraints = false
        candidateExpandedScroll.addSubview(candidateExpandedStack)
        NSLayoutConstraint.activate([
            candidateExpandedScroll.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 6),
            candidateExpandedScroll.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -6),
            candidateExpandedScroll.topAnchor.constraint(equalTo: view.topAnchor, constant: 20),
            candidateExpandedScroll.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -7),
            candidateExpandedCloseButton.topAnchor.constraint(equalTo: view.topAnchor, constant: 24),
            candidateExpandedCloseButton.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -10),
            candidateExpandedCloseButton.widthAnchor.constraint(equalToConstant: 32),
            candidateExpandedCloseButton.heightAnchor.constraint(equalToConstant: 32),
            candidateExpandedStack.leadingAnchor.constraint(equalTo: candidateExpandedScroll.contentLayoutGuide.leadingAnchor, constant: 3),
            candidateExpandedStack.trailingAnchor.constraint(equalTo: candidateExpandedScroll.contentLayoutGuide.trailingAnchor, constant: -3),
            candidateExpandedStack.topAnchor.constraint(equalTo: candidateExpandedScroll.contentLayoutGuide.topAnchor, constant: 3),
            candidateExpandedStack.bottomAnchor.constraint(equalTo: candidateExpandedScroll.contentLayoutGuide.bottomAnchor, constant: -3),
            candidateExpandedStack.widthAnchor.constraint(equalTo: candidateExpandedScroll.frameLayoutGuide.widthAnchor, constant: -6),
        ])
        view.bringSubviewToFront(candidateExpandedCloseButton)

        keyboardStack.axis = .vertical
        keyboardStack.spacing = 6
        root.addArrangedSubview(keyboardStack)
        renderKeyboard()
    }

    private func renderKeyboard() {
        hideKeyPreview()
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
                [".", ",", "?", "!", "'", "%", "＋", "⌫"],
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
            let title = label == "空格" ? "" : (label == "⏎" ? (englishMode ? "return" : "换行")
                : (englishMode && englishShift && label.count == 1 && label.first?.isLetter == true
                    ? label.uppercased() : label))
            button.setTitle(title, for: .normal)
            if label == "空格" {
                button.accessibilityLabel = "空格"
            }
        }
        button.setTitleColor(fixedTextColor, for: .normal)
        button.titleLabel?.font = .systemFont(
            ofSize: label.count > 2 ? 12 : (keyboardKeyColor(for: label) == actionKeyColor ? 15 : 23),
            weight: label.count == 1 && label.first?.isLetter == true ? .bold : .regular
        )
        button.backgroundColor = keyboardKeyColor(for: label)
        button.layer.cornerRadius = 5
        button.layer.cornerCurve = .continuous
        button.layer.shadowColor = UIColor(red: 0.50, green: 0.51, blue: 0.53, alpha: 1).cgColor
        button.layer.shadowOpacity = 0.28
        button.layer.shadowOffset = CGSize(width: 0, height: 1)
        button.layer.shadowRadius = 0
        button.heightAnchor.constraint(equalToConstant: 46).isActive = true
        if supportsKeyPreview(label) {
            button.addTarget(self, action: #selector(showKeyPreview(_:)), for: .touchDown)
            button.addTarget(self, action: #selector(hideKeyPreview), for: [.touchUpInside, .touchUpOutside, .touchCancel, .touchDragExit])
        }
        if label == "⌫" {
            button.addTarget(self, action: #selector(backspacePressed(_:)), for: .touchDown)
            button.addTarget(self, action: #selector(stopBackspaceRepeat), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        } else {
            button.addTarget(self, action: #selector(keyPressed(_:)), for: .touchUpInside)
        }
        return button
    }

    private func supportsKeyPreview(_ label: String) -> Bool {
        label.count == 1 && !["🌐", "⌫", "⇧", "⏎"].contains(label)
    }

    @objc private func showKeyPreview(_ sender: UIButton) {
        guard let label = sender.title(for: .normal), !label.isEmpty else { return }
        let keyFrame = sender.convert(sender.bounds, to: view)
        let width: CGFloat = 58
        let height: CGFloat = 66
        let centerX = min(max(keyFrame.midX, width / 2 + 3), view.bounds.width - width / 2 - 3)
        keyPreviewView.frame = CGRect(
            x: centerX - width / 2,
            y: max(0, keyFrame.minY - height - 5),
            width: width,
            height: height
        )
        keyPreviewView.text = label
        keyPreviewView.isHidden = false
        view.bringSubviewToFront(keyPreviewView)
    }

    @objc private func hideKeyPreview() {
        keyPreviewView.isHidden = true
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
        case "分词":
            return 1.5
        case "🌐", "英", "中", "123", "ABC", "符号", "更多", "常用", "⇧":
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
        preeditLabel.text = snapshot.preedit.isEmpty
            ? (hasFullAccess ? nil : "开启“允许完全访问”后，用户词库才能保存")
            : snapshot.preedit
        candidateExpandButton.isHidden = snapshot.candidates.isEmpty
        candidateExpandButton.setTitle(candidateExpanded ? "⌃" : "⌄", for: .normal)
        candidateStack.arrangedSubviews.forEach {
            candidateStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        for candidate in snapshot.candidates {
            candidateStack.addArrangedSubview(makeCandidateButton(candidate, expanded: false))
        }
        renderExpandedCandidates()
    }

    private func makeCandidateButton(_ candidate: GonnyuAppleCandidate, expanded: Bool) -> UIButton {
        let button = UIButton(type: .system)
        var configuration = UIButton.Configuration.plain()
        configuration.baseForegroundColor = fixedTextColor
        configuration.contentInsets = NSDirectionalEdgeInsets(top: 1, leading: 5, bottom: 1, trailing: 5)
        configuration.title = candidate.text
        configuration.subtitle = candidate.annotation.isEmpty ? candidate.reading : candidate.annotation
        configuration.titleLineBreakMode = .byClipping
        configuration.subtitleLineBreakMode = .byClipping
        configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
            var attributes = $0
            attributes.font = .systemFont(ofSize: 16, weight: .regular)
            return attributes
        }
        configuration.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
            var attributes = $0
            attributes.font = .systemFont(ofSize: 10)
            attributes.foregroundColor = self.fixedSecondaryTextColor
            return attributes
        }
        button.configuration = configuration
        button.titleLabel?.numberOfLines = 1
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        button.accessibilityLabel = candidate.text
        button.accessibilityHint = candidate.annotation
        button.tag = candidate.globalIndex
        button.addTarget(self, action: #selector(candidatePressed(_:)), for: .touchUpInside)
        return button
    }

    private func candidateWidth(_ candidate: GonnyuAppleCandidate, maximum: CGFloat) -> CGFloat {
        let subtitle = candidate.annotation.isEmpty ? candidate.reading ?? "" : candidate.annotation
        let titleWidth = (candidate.text as NSString).size(withAttributes: [.font: UIFont.systemFont(ofSize: 16)]).width
        let subtitleWidth = (subtitle as NSString).size(withAttributes: [.font: UIFont.systemFont(ofSize: 10)]).width
        return max(44, max(titleWidth, subtitleWidth) + 10)
    }

    private func renderExpandedCandidates() {
        candidateExpandedStack.arrangedSubviews.forEach {
            candidateExpandedStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        guard candidateExpanded else {
            candidateExpandedScroll.isHidden = true
            candidateExpandedCloseButton.isHidden = true
            return
        }
        candidateExpandedCloseButton.isHidden = false
        view.bringSubviewToFront(candidateExpandedCloseButton)
        let available = max(44, candidateExpandedScroll.bounds.width - 6)
        candidateExpandedScroll.isHidden = false
        expandedLayoutWidth = candidateExpandedScroll.bounds.width
        var row = makeExpandedCandidateRow()
        var usedWidth: CGFloat = 0
        for candidate in expandedCandidates {
            let width = candidateWidth(candidate, maximum: available)
            if usedWidth > 0 && usedWidth + 3 + width > available {
                candidateExpandedStack.addArrangedSubview(row)
                row = makeExpandedCandidateRow()
                usedWidth = 0
            }
            let button = makeCandidateButton(candidate, expanded: true)
            button.widthAnchor.constraint(equalToConstant: width).isActive = true
            row.addArrangedSubview(button)
            usedWidth += (usedWidth == 0 ? 0 : 3) + width
        }
        if !row.arrangedSubviews.isEmpty {
            candidateExpandedStack.addArrangedSubview(row)
        }
    }

    private func makeExpandedCandidateRow() -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 3
        row.alignment = .top
        return row
    }

    @objc private func toggleCandidateExpansion() {
        if candidateExpanded {
            collapseCandidateExpansion()
            return
        }
        guard !snapshot.candidates.isEmpty else { return }
        candidateExpanded = true
        expandedCandidates = snapshot.candidates
        render()
    }

    private func collapseCandidateExpansion() {
        candidateExpanded = false
        expandedCandidates.removeAll()
        render()
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
        let compositionChanged = snapshot.rawInput != updated.rawInput || updated.commitText != nil
        if let commit = updated.commitText, !commit.isEmpty {
            textDocumentProxy.insertText(commit)
        }
        snapshot = updated
        if compositionChanged {
            candidateExpanded = false
            expandedCandidates.removeAll()
        }
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
        candidateExpanded = false
        expandedCandidates.removeAll()
        render()
    }
}
