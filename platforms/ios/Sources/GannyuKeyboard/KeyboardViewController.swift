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

private enum KeyRowLayout {
    case reference
    case centered
    case deleteExtended
    case bottom
}

/// The control owns the complete logical key area. `faceView` draws the
/// smaller key cap inside that area, so visible gaps never become touch gaps.
private final class KeyboardKeyButton: UIButton {
    private let faceView = UIView()
    var faceInsets: UIEdgeInsets = .zero {
        didSet { setNeedsLayout() }
    }

    var faceFrame: CGRect { bounds.inset(by: faceInsets) }

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        faceView.isUserInteractionEnabled = false
        faceView.layer.cornerRadius = 5
        faceView.layer.cornerCurve = .continuous
        faceView.layer.shadowColor = UIColor(red: 0.50, green: 0.51, blue: 0.53, alpha: 1).cgColor
        faceView.layer.shadowOpacity = 0.28
        faceView.layer.shadowOffset = CGSize(width: 0, height: 1)
        faceView.layer.shadowRadius = 0
        insertSubview(faceView, at: 0)
    }

    required init?(coder: NSCoder) { nil }

    func setFaceColor(_ color: UIColor) {
        faceView.backgroundColor = color
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        faceView.frame = faceFrame
    }

    override func contentRect(forBounds bounds: CGRect) -> CGRect {
        bounds.inset(by: faceInsets)
    }
}

/// Lays out the old visual key frames, then expands their logical controls to
/// the midpoints between neighbouring key caps. The logical frames therefore
/// tile the complete row with no ownerless pixels.
private final class KeyboardKeyRowView: UIView {
    let buttons: [KeyboardKeyButton]
    private let labels: [String]
    private let layout: KeyRowLayout
    private let gap: CGFloat
    private let keyHeight: CGFloat
    private let topVisualInset: CGFloat
    private let bottomVisualInset: CGFloat
    private let widthMultiplier: (String) -> CGFloat

    init(
        labels: [String],
        buttons: [KeyboardKeyButton],
        layout: KeyRowLayout,
        gap: CGFloat,
        keyHeight: CGFloat,
        topVisualInset: CGFloat,
        bottomVisualInset: CGFloat,
        widthMultiplier: @escaping (String) -> CGFloat
    ) {
        self.labels = labels
        self.buttons = buttons
        self.layout = layout
        self.gap = gap
        self.keyHeight = keyHeight
        self.topVisualInset = topVisualInset
        self.bottomVisualInset = bottomVisualInset
        self.widthMultiplier = widthMultiplier
        super.init(frame: .zero)
        buttons.forEach(addSubview)
    }

    required init?(coder: NSCoder) { nil }

    override func layoutSubviews() {
        super.layoutSubviews()
        let visualFrames = makeVisualFrames(width: bounds.width)
        guard visualFrames.count == buttons.count else { return }
        for index in buttons.indices {
            let visual = visualFrames[index]
            let minX = index == 0 ? bounds.minX : (visualFrames[index - 1].maxX + visual.minX) / 2
            let maxX = index == buttons.index(before: buttons.endIndex)
                ? bounds.maxX
                : (visual.maxX + visualFrames[index + 1].minX) / 2
            let logical = CGRect(x: minX, y: 0, width: maxX - minX, height: bounds.height)
            let button = buttons[index]
            button.frame = logical
            button.faceInsets = UIEdgeInsets(
                top: topVisualInset,
                left: visual.minX - logical.minX,
                bottom: bottomVisualInset,
                right: logical.maxX - visual.maxX
            )
        }
    }

    /// Returns true only if the logical controls form an exact, gap-free row.
    func hasContinuousLogicalCoverage(tolerance: CGFloat = 0.01) -> Bool {
        guard let first = buttons.first, let last = buttons.last else { return false }
        guard abs(first.frame.minX - bounds.minX) <= tolerance,
              abs(last.frame.maxX - bounds.maxX) <= tolerance else { return false }
        return zip(buttons, buttons.dropFirst()).allSatisfy { pair in
            abs(pair.0.frame.maxX - pair.1.frame.minX) <= tolerance
        }
    }

    private func makeVisualFrames(width: CGFloat) -> [CGRect] {
        guard !labels.isEmpty else { return [] }
        let referenceWidth = max(0, (width - gap * 9) / 10)
        let widths: [CGFloat]
        let originX: CGFloat
        switch layout {
        case .reference:
            widths = Array(repeating: max(0, (width - gap * CGFloat(labels.count - 1)) / CGFloat(labels.count)), count: labels.count)
            originX = 0
        case .centered:
            widths = labels.map { referenceWidth * widthMultiplier($0) }
            let contentWidth = widths.reduce(0, +) + gap * CGFloat(max(0, labels.count - 1))
            originX = max(0, (width - contentWidth) / 2)
        case .deleteExtended:
            let fixed = labels.dropLast().map { referenceWidth * widthMultiplier($0) }
            let remaining = max(0, width - fixed.reduce(0, +) - gap * CGFloat(max(0, labels.count - 1)))
            widths = fixed + [remaining]
            originX = 0
        case .bottom:
            let fixedWidth = zip(labels, labels.indices).reduce(CGFloat.zero) { result, pair in
                pair.0 == "空格" ? result : result + referenceWidth * widthMultiplier(pair.0)
            }
            let flexible = max(0, width - fixedWidth - gap * CGFloat(max(0, labels.count - 1)))
            widths = labels.map { $0 == "空格" ? flexible : referenceWidth * widthMultiplier($0) }
            originX = 0
        }

        var x = originX
        return widths.map { itemWidth in
            defer { x += itemWidth + gap }
            return CGRect(x: x, y: topVisualInset, width: itemWidth, height: keyHeight)
        }
    }
}

/// A single owner for every touch in the keyboard rectangle. The key-cap
/// buttons are display/action objects only; UIKit never has to hit-test the
/// visual gaps between them. A touch keeps the key chosen at touch-down until
/// release, matching the ownership model used by mature custom keyboards.
private final class KeyboardTouchStackView: UIStackView {
    func nearestButton(to point: CGPoint) -> KeyboardKeyButton? {
        guard bounds.contains(point) else { return nil }
        var nearest: (button: KeyboardKeyButton, distanceSquared: CGFloat)?
        for case let row as KeyboardKeyRowView in arrangedSubviews {
            for button in row.buttons {
                let frame = row.convert(button.frame, to: self)
                let dx = max(0, max(frame.minX - point.x, point.x - frame.maxX))
                let dy = max(0, max(frame.minY - point.y, point.y - frame.maxY))
                let distanceSquared = dx * dx + dy * dy
                if nearest == nil || distanceSquared < nearest!.distanceSquared {
                    nearest = (button, distanceSquared)
                }
            }
        }
        return nearest?.button
    }
}

/// The sole UIKit hit-test target for the complete keyboard extension. Visual
/// descendants remain display-only; the controller routes these raw touches
/// to keys, candidates, scrolling, and candidate controls by geometry.
private final class UnifiedInputTouchView: UIView {
    var onBegan: ((UITouch, CGPoint) -> Void)?
    var onMoved: ((UITouch, CGPoint) -> Void)?
    var onEnded: ((UITouch, CGPoint, Bool) -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        // Custom keyboard extensions can discard touches on fully transparent
        // pixels before UIKit hit-testing reaches this unified touch owner.
        // A near-invisible fill keeps visual transparency while preserving the
        // complete key-gap and candidate touch surface.
        backgroundColor = UIColor(white: 0.5, alpha: 0.01)
        isMultipleTouchEnabled = true
        isUserInteractionEnabled = true
    }

    required init?(coder: NSCoder) { nil }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, alpha >= 0.01, isUserInteractionEnabled, bounds.contains(point) else {
            return nil
        }
        return self
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { onBegan?(touch, touch.location(in: self)) }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { onMoved?(touch, touch.location(in: self)) }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { onEnded?(touch, touch.location(in: self), false) }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { onEnded?(touch, touch.location(in: self), true) }
    }
}

private final class CandidateCollectionViewCell: UICollectionViewCell {
    static let reuseIdentifier = "CandidateCollectionViewCell"
    private let titleLabel = UILabel()
    private let subtitleLabel = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        titleLabel.font = .systemFont(ofSize: 18, weight: .regular)
        titleLabel.lineBreakMode = .byClipping
        subtitleLabel.font = .systemFont(ofSize: 10)
        let labels = UIStackView(arrangedSubviews: [titleLabel, subtitleLabel])
        labels.axis = .vertical
        labels.alignment = .leading
        labels.isUserInteractionEnabled = false
        labels.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(labels)
        NSLayoutConstraint.activate([
            labels.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 5),
            // The final 3pt belongs to this cell's hit area and visually
            // replaces the old, untappable inter-item spacing.
            labels.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -8),
            labels.topAnchor.constraint(greaterThanOrEqualTo: contentView.topAnchor, constant: 1),
            labels.bottomAnchor.constraint(lessThanOrEqualTo: contentView.bottomAnchor, constant: -1),
            labels.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
        ])
    }

    required init?(coder: NSCoder) { nil }

    func configure(title: String, subtitle: String, expanded: Bool, textColor: UIColor, secondaryColor: UIColor) {
        titleLabel.text = title
        titleLabel.textColor = textColor
        subtitleLabel.text = subtitle
        subtitleLabel.textColor = secondaryColor
        subtitleLabel.numberOfLines = expanded ? 0 : 1
        subtitleLabel.lineBreakMode = expanded ? .byCharWrapping : .byClipping
        isAccessibilityElement = true
        accessibilityTraits = .button
        accessibilityLabel = title
        accessibilityHint = subtitle
    }
}

final class KeyboardViewController: UIInputViewController {
    private enum KeyboardPage {
        case letters
        case numbers
        case symbols
        case symbolsMore
    }

    private enum UnifiedTouchTarget {
        case key(KeyboardKeyButton)
        case candidate(UICollectionView, IndexPath?)
        case expandCandidates
        case collapseCandidates
        case none
    }

    private struct UnifiedTouchState {
        let target: UnifiedTouchTarget
        let start: CGPoint
        let initialContentOffset: CGPoint
        var maximumDistance: CGFloat
    }

    private lazy var store = GonnyuAppleRegionStore(preferSharedStorage: hasFullAccess)
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
    private let candidateLayout = UICollectionViewFlowLayout()
    private lazy var candidateCollection = UICollectionView(frame: .zero, collectionViewLayout: candidateLayout)
    private let candidateExpandButton = UIButton(type: .system)
    private let candidateExpandedLayout = UICollectionViewFlowLayout()
    private lazy var candidateExpandedCollection = UICollectionView(frame: .zero, collectionViewLayout: candidateExpandedLayout)
    private let candidateExpandedCloseButton = UIButton(type: .system)
    private var candidateExpanded = false
    private var expandedCandidates: [GonnyuAppleCandidate] = []
    private var expandedLayoutWidth: CGFloat = 0
    private let keyboardStack = KeyboardTouchStackView(frame: .zero)
    private let unifiedTouchView = UnifiedInputTouchView(frame: .zero)
    private var unifiedTouches: [UITouch: UnifiedTouchState] = [:]
    private let keyPreviewView = KeyPreviewView(frame: .zero)
    private var keyboardRowViews: [KeyboardKeyRowView] = []
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
    private let keyHeight: CGFloat = UIDevice.current.userInterfaceIdiom == .pad ? 69 : 46

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
        endSession()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        keyboardRowViews.forEach { $0.layoutIfNeeded() }
        if candidateExpanded && candidateExpandedCollection.bounds.width != expandedLayoutWidth {
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
        // Keep the complete remote keyboard surface rendered. The unified
        // touch owner below covers the same bounds, including visual gaps.
        // Keep the keyboard surface visually transparent. Touch ownership is
        // provided independently by UnifiedInputTouchView below.
        view.backgroundColor = .clear
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

        candidateLayout.scrollDirection = .horizontal
        candidateLayout.minimumLineSpacing = 0
        candidateLayout.minimumInteritemSpacing = 0
        candidateCollection.showsHorizontalScrollIndicator = false
        candidateCollection.backgroundColor = .clear
        candidateCollection.dataSource = self
        candidateCollection.delegate = self
        candidateCollection.register(CandidateCollectionViewCell.self, forCellWithReuseIdentifier: CandidateCollectionViewCell.reuseIdentifier)
        candidateRow.addArrangedSubview(candidateCollection)

        candidateExpandButton.setTitle("⌄", for: .normal)
        candidateExpandButton.setTitleColor(fixedTextColor, for: .normal)
        candidateExpandButton.titleLabel?.font = .systemFont(ofSize: 17, weight: .medium)
        candidateExpandButton.backgroundColor = actionKeyColor
        candidateExpandButton.layer.cornerRadius = 5
        candidateExpandButton.widthAnchor.constraint(equalToConstant: 32).isActive = true
        candidateExpandButton.addTarget(self, action: #selector(toggleCandidateExpansion), for: .touchUpInside)
        candidateRow.addArrangedSubview(candidateExpandButton)
        candidateExpandedLayout.scrollDirection = .vertical
        candidateExpandedLayout.minimumLineSpacing = 0
        candidateExpandedLayout.minimumInteritemSpacing = 0
        candidateExpandedCollection.showsVerticalScrollIndicator = true
        candidateExpandedCollection.backgroundColor = keyboardPanelColor
        candidateExpandedCollection.dataSource = self
        candidateExpandedCollection.delegate = self
        candidateExpandedCollection.contentInset = UIEdgeInsets(top: 3, left: 3, bottom: 3, right: 3)
        candidateExpandedCollection.translatesAutoresizingMaskIntoConstraints = false
        candidateExpandedCollection.isHidden = true
        candidateExpandedCollection.register(CandidateCollectionViewCell.self, forCellWithReuseIdentifier: CandidateCollectionViewCell.reuseIdentifier)
        view.addSubview(candidateExpandedCollection)

        candidateExpandedCloseButton.setTitle("⌃", for: .normal)
        candidateExpandedCloseButton.setTitleColor(fixedTextColor, for: .normal)
        candidateExpandedCloseButton.backgroundColor = actionKeyColor
        candidateExpandedCloseButton.layer.cornerRadius = 5
        candidateExpandedCloseButton.translatesAutoresizingMaskIntoConstraints = false
        candidateExpandedCloseButton.addTarget(self, action: #selector(toggleCandidateExpansion), for: .touchUpInside)
        view.addSubview(candidateExpandedCloseButton)
        NSLayoutConstraint.activate([
            candidateExpandedCollection.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 6),
            candidateExpandedCollection.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -6),
            candidateExpandedCollection.topAnchor.constraint(equalTo: view.topAnchor, constant: 20),
            candidateExpandedCollection.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -7),
            candidateExpandedCloseButton.topAnchor.constraint(equalTo: view.topAnchor, constant: 24),
            candidateExpandedCloseButton.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -10),
            candidateExpandedCloseButton.widthAnchor.constraint(equalToConstant: 32),
            candidateExpandedCloseButton.heightAnchor.constraint(equalToConstant: 32),
        ])
        view.bringSubviewToFront(candidateExpandedCloseButton)

        keyboardStack.axis = .vertical
        keyboardStack.spacing = 0
        keyboardStack.translatesAutoresizingMaskIntoConstraints = false
        root.addArrangedSubview(keyboardStack)
        renderKeyboard()

        unifiedTouchView.translatesAutoresizingMaskIntoConstraints = false
        unifiedTouchView.onBegan = { [weak self] touch, point in
            self?.unifiedTouchBegan(touch, at: point)
        }
        unifiedTouchView.onMoved = { [weak self] touch, point in
            self?.unifiedTouchMoved(touch, to: point)
        }
        unifiedTouchView.onEnded = { [weak self] touch, point, cancelled in
            self?.unifiedTouchEnded(touch, at: point, cancelled: cancelled)
        }
        view.addSubview(unifiedTouchView)
        NSLayoutConstraint.activate([
            unifiedTouchView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            unifiedTouchView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            unifiedTouchView.topAnchor.constraint(equalTo: view.topAnchor),
            unifiedTouchView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    private func unifiedTouchBegan(_ touch: UITouch, at point: CGPoint) {
        let target: UnifiedTouchTarget
        var initialOffset = CGPoint.zero
        if candidateExpanded {
            let closeFrame = candidateExpandedCloseButton.convert(candidateExpandedCloseButton.bounds, to: view)
            let collectionFrame = candidateExpandedCollection.convert(candidateExpandedCollection.bounds, to: view)
            if closeFrame.contains(point) {
                target = .collapseCandidates
            } else if collectionFrame.contains(point) {
                let local = candidateExpandedCollection.convert(point, from: view)
                target = .candidate(candidateExpandedCollection, candidateExpandedCollection.indexPathForItem(at: local))
                initialOffset = candidateExpandedCollection.contentOffset
            } else {
                target = .none
            }
        } else {
            let expandFrame = candidateExpandButton.convert(candidateExpandButton.bounds, to: view)
            let candidateFrame = candidateCollection.convert(candidateCollection.bounds, to: view)
            let keyboardFrame = keyboardStack.convert(keyboardStack.bounds, to: view)
            if !candidateExpandButton.isHidden && expandFrame.contains(point) {
                target = .expandCandidates
            } else if candidateFrame.contains(point) {
                let local = candidateCollection.convert(point, from: view)
                target = .candidate(candidateCollection, candidateCollection.indexPathForItem(at: local))
                initialOffset = candidateCollection.contentOffset
            } else if keyboardFrame.contains(point) {
                let local = keyboardStack.convert(point, from: view)
                let button = keyboardStack.nearestButton(to: local)
                target = button.map(UnifiedTouchTarget.key) ?? .none
                button?.isHighlighted = true
                button?.sendActions(for: .touchDown)
            } else {
                target = .none
            }
        }

        unifiedTouches[touch] = UnifiedTouchState(
            target: target,
            start: point,
            initialContentOffset: initialOffset,
            maximumDistance: 0
        )
    }

    private func unifiedTouchMoved(_ touch: UITouch, to point: CGPoint) {
        guard var state = unifiedTouches[touch] else { return }
        let dx = point.x - state.start.x
        let dy = point.y - state.start.y
        state.maximumDistance = max(state.maximumDistance, hypot(dx, dy))
        if case .candidate(let collection, _) = state.target, state.maximumDistance >= 8 {
            collection.layoutIfNeeded()
            let inset = collection.adjustedContentInset
            if collection === candidateCollection {
                let minimum = -inset.left
                let maximum = max(minimum, collection.contentSize.width - collection.bounds.width + inset.right)
                let x = min(maximum, max(minimum, state.initialContentOffset.x - dx))
                collection.setContentOffset(CGPoint(x: x, y: state.initialContentOffset.y), animated: false)
            } else {
                let minimum = -inset.top
                let maximum = max(minimum, collection.contentSize.height - collection.bounds.height + inset.bottom)
                let y = min(maximum, max(minimum, state.initialContentOffset.y - dy))
                collection.setContentOffset(CGPoint(x: state.initialContentOffset.x, y: y), animated: false)
            }
        }
        unifiedTouches[touch] = state
    }

    private func unifiedTouchEnded(_ touch: UITouch, at point: CGPoint, cancelled: Bool) {
        guard var state = unifiedTouches.removeValue(forKey: touch) else { return }
        state.maximumDistance = max(
            state.maximumDistance,
            hypot(point.x - state.start.x, point.y - state.start.y)
        )

        switch state.target {
        case .key(let button):
            button.isHighlighted = false
            button.sendActions(for: cancelled ? .touchCancel : .touchUpInside)
        case .candidate(let collection, let startIndex):
            let outcome = !cancelled && state.maximumDistance < 8 ? "tap" : "pan"
            let localEnd = collection.convert(point, from: view)
            let indexPath = collection.indexPathForItem(at: localEnd) ?? startIndex
            if outcome == "tap", let indexPath {
                let candidates = collection === candidateCollection ? snapshot.candidates : expandedCandidates
                if candidates.indices.contains(indexPath.item) {
                    commitCandidate(at: candidates[indexPath.item].globalIndex)
                }
            }
        case .expandCandidates:
            if !cancelled && state.maximumDistance < 8 { toggleCandidateExpansion() }
        case .collapseCandidates:
            if !cancelled && state.maximumDistance < 8 { collapseCandidateExpansion() }
        case .none:
            break
        }

    }

    private func renderKeyboard() {
        hideKeyPreview()
        keyboardStack.arrangedSubviews.forEach {
            keyboardStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        keyboardRowViews.removeAll()
        var rows: [([String], KeyRowLayout)] = []
        switch keyboardPage {
        case .letters:
            rows = [
                ("qwertyuiop".map(String.init), .reference),
                ("asdfghjkl".map(String.init), .centered),
                ([englishMode ? "⇧" : "分词"] + "zxcvbnm".map(String.init) + ["⌫"], .deleteExtended),
            ]
        case .numbers:
            rows = [
                (["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"], .reference),
                (["-", "/", ":", ";", "(", ")", "¥", "&", "@", "\""], .reference),
                ([".", ",", "?", "!", "'", "%", "＋", "⌫"], .deleteExtended),
            ]
        case .symbols:
            rows = [
                (["【", "】", "“", "”", "〈", "〉", "《", "》", "：", "；"], .reference),
                (["，", "、", "。", "？", "！", "…", "—", "～", "·", "／"], .reference),
                (["更多", "（", "）", "[", "]", "{", "}", "#", "⌫"], .deleteExtended),
            ]
        case .symbolsMore:
            rows = [
                (["+", "−", "=", "×", "÷", "<", ">", "^", "~", "_"], .reference),
                (["@", "#", "$", "¥", "€", "£", "&", "*", "\\", "|"], .reference),
                (["常用", "!", "?", "'", "\"", ":", ";", "／", "⌫"], .deleteExtended),
            ]
        }
        rows.append((
            [keyboardPage == .letters ? (englishMode ? "中" : "英") : (keyboardPage == .numbers ? "符号" : "123"),
             keyboardPage == .letters ? "123" : "ABC", "空格", englishMode ? "," : "，",
             englishMode ? "." : "。", "⏎"],
            .bottom
        ))

        for (index, rowSpec) in rows.enumerated() {
            let row = keyRow(
                rowSpec.0,
                layout: rowSpec.1,
                topVisualInset: index == 0 ? 0 : keySpacing / 2,
                bottomVisualInset: index == rows.count - 1 ? 0 : keySpacing / 2
            )
            keyboardRowViews.append(row)
            keyboardStack.addArrangedSubview(row)
        }
    }

    private func keyRow(
        _ labels: [String],
        layout: KeyRowLayout,
        topVisualInset: CGFloat,
        bottomVisualInset: CGFloat
    ) -> KeyboardKeyRowView {
        let buttons = labels.map(makeKeyButton)
        let row = KeyboardKeyRowView(
            labels: labels,
            buttons: buttons,
            layout: layout,
            gap: keySpacing,
            keyHeight: keyHeight,
            topVisualInset: topVisualInset,
            bottomVisualInset: bottomVisualInset,
            widthMultiplier: { [weak self] label in self?.functionKeyWidth(for: label) ?? 1 }
        )
        row.heightAnchor.constraint(equalToConstant: keyHeight + topVisualInset + bottomVisualInset).isActive = true
        return row
    }

    private func makeKeyButton(_ label: String) -> KeyboardKeyButton {
        let button = KeyboardKeyButton(frame: .zero)
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
        button.setFaceColor(keyboardKeyColor(for: label))
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
        let visualBounds = (sender as? KeyboardKeyButton)?.faceFrame ?? sender.bounds
        let keyFrame = sender.convert(visualBounds, to: view)
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

    private func functionKeyWidth(for label: String) -> CGFloat {
        switch label {
        case "分词", "⇧":
            return 1.5
        case "⏎":
            return 1.6
        case "🌐", "英", "中", "123", "ABC", "符号", "更多", "常用":
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
        preeditLabel.text = snapshot.preedit
        candidateExpandButton.isHidden = snapshot.candidates.isEmpty
        candidateExpandButton.setTitle(candidateExpanded ? "⌃" : "⌄", for: .normal)
        candidateCollection.reloadData()
        candidateCollection.setContentOffset(.zero, animated: false)
        renderExpandedCandidates()
    }

    private func candidateSubtitle(_ candidate: GonnyuAppleCandidate, expanded: Bool) -> String {
        let subtitle = candidate.annotation.isEmpty ? candidate.reading ?? "" : candidate.annotation
        return expanded ? subtitle.replacingOccurrences(of: " / ", with: "/\n") : subtitle
    }

    private func candidateWidth(_ candidate: GonnyuAppleCandidate) -> CGFloat {
        let titleWidth = (candidate.text as NSString).size(withAttributes: [.font: UIFont.systemFont(ofSize: 18)]).width
        return max(44, titleWidth + 10)
    }

    private func compactCandidateWidth(_ candidate: GonnyuAppleCandidate) -> CGFloat {
        let titleWidth = (candidate.text as NSString).size(withAttributes: [.font: UIFont.systemFont(ofSize: 18)]).width
        let subtitle = candidateSubtitle(candidate, expanded: false)
        let subtitleWidth = (subtitle as NSString).size(withAttributes: [.font: UIFont.systemFont(ofSize: 10)]).width
        return ceil(max(titleWidth, subtitleWidth) + 10)
    }

    private func candidateHeight(_ candidate: GonnyuAppleCandidate, width: CGFloat) -> CGFloat {
        let subtitle = candidateSubtitle(candidate, expanded: true)
        guard !subtitle.isEmpty else { return 37 }
        let subtitleFont = UIFont.systemFont(ofSize: 10)
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byCharWrapping
        let subtitleHeight = (subtitle as NSString).boundingRect(
            with: CGSize(width: max(1, width - 10), height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: subtitleFont, .paragraphStyle: paragraph],
            context: nil
        ).height
        return max(37, ceil(UIFont.systemFont(ofSize: 18).lineHeight + subtitleHeight + 6))
    }

    private func renderExpandedCandidates() {
        guard candidateExpanded else {
            candidateExpandedCollection.isHidden = true
            candidateExpandedCloseButton.isHidden = true
            candidateExpandedCollection.reloadData()
            return
        }
        candidateExpandedCloseButton.isHidden = false
        candidateExpandedCollection.isHidden = false
        view.bringSubviewToFront(candidateExpandedCollection)
        view.bringSubviewToFront(candidateExpandedCloseButton)
        expandedLayoutWidth = candidateExpandedCollection.bounds.width
        candidateExpandedLayout.invalidateLayout()
        candidateExpandedCollection.reloadData()
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

    private func commitCandidate(at index: Int) {
        // An expanded list belongs to one immutable candidate snapshot.  Once a
        // candidate is consumed, hide that snapshot before the engine produces
        // its next (possibly shorter) composition.
        if candidateExpanded {
            collapseCandidateExpansion()
        }
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

    private func endSession() {
        stopBackspaceRepeat()
        hideKeyPreview()
        candidateExpanded = false
        expandedCandidates.removeAll()
        _ = try? engine?.clearComposition()
        snapshot = .empty
        if isViewLoaded {
            render()
        }
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

extension KeyboardViewController: UICollectionViewDataSource, UICollectionViewDelegateFlowLayout {
    func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
        collectionView === candidateCollection ? snapshot.candidates.count : expandedCandidates.count
    }

    func collectionView(
        _ collectionView: UICollectionView,
        cellForItemAt indexPath: IndexPath
    ) -> UICollectionViewCell {
        let cell = collectionView.dequeueReusableCell(
            withReuseIdentifier: CandidateCollectionViewCell.reuseIdentifier,
            for: indexPath
        )
        guard let candidateCell = cell as? CandidateCollectionViewCell else { return cell }
        let expanded = collectionView === candidateExpandedCollection
        let candidates = expanded ? expandedCandidates : snapshot.candidates
        guard candidates.indices.contains(indexPath.item) else { return candidateCell }
        let candidate = candidates[indexPath.item]
        candidateCell.configure(
            title: candidate.text,
            subtitle: candidateSubtitle(candidate, expanded: expanded),
            expanded: expanded,
            textColor: fixedTextColor,
            secondaryColor: fixedSecondaryTextColor
        )
        return candidateCell
    }

    func collectionView(
        _ collectionView: UICollectionView,
        layout collectionViewLayout: UICollectionViewLayout,
        sizeForItemAt indexPath: IndexPath
    ) -> CGSize {
        let expanded = collectionView === candidateExpandedCollection
        let candidates = expanded ? expandedCandidates : snapshot.candidates
        guard candidates.indices.contains(indexPath.item) else { return .zero }
        let candidate = candidates[indexPath.item]
        let contentWidth = expanded ? candidateWidth(candidate) : compactCandidateWidth(candidate)
        // The compact candidate row is constrained to 39pt.  Use that stable
        // value even before the collection view's first layout pass.
        let height = expanded ? candidateHeight(candidate, width: contentWidth) : 39
        return CGSize(width: contentWidth + 3, height: height)
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
        let candidates = collectionView === candidateCollection ? snapshot.candidates : expandedCandidates
        guard candidates.indices.contains(indexPath.item) else { return }
        let candidate = candidates[indexPath.item]
        commitCandidate(at: candidate.globalIndex)
    }

}
