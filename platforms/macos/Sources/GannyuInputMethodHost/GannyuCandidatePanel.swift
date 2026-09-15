import AppKit
import GannyuMacOSSupport

final class GannyuCandidatePanel: NSObject {
    var onSelect: ((Int) -> Void)?
    var onPage: ((Int) -> Void)?

    private let panel: NSPanel
    private let content = NSVisualEffectView()
    private let rows = NSStackView()
    private let footer = NSStackView()
    private let previous = NSButton(title: "<", target: nil, action: nil)
    private let next = NSButton(title: ">", target: nil, action: nil)
    private var width: NSLayoutConstraint!

    override init() {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 280, height: 100),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        super.init()
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .popUpMenu
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false

        content.material = .popover
        content.blendingMode = .withinWindow
        content.state = .active
        content.wantsLayer = true
        content.layer?.cornerRadius = 10
        content.layer?.masksToBounds = true
        panel.contentView = content

        rows.orientation = .vertical
        rows.alignment = .leading
        rows.spacing = 2
        rows.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(rows)

        footer.orientation = .horizontal
        footer.alignment = .centerY
        footer.distribution = .equalSpacing
        footer.spacing = 0
        footer.translatesAutoresizingMaskIntoConstraints = false
        configureFooterButton(previous, action: #selector(previousPage))
        configureFooterButton(next, action: #selector(nextPage))
        footer.addArrangedSubview(previous)
        footer.addArrangedSubview(next)
        content.addSubview(footer)

        width = content.widthAnchor.constraint(equalToConstant: 280)
        NSLayoutConstraint.activate([
            width,
            rows.topAnchor.constraint(equalTo: content.topAnchor, constant: 8),
            rows.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 8),
            rows.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -8),
            footer.topAnchor.constraint(equalTo: rows.bottomAnchor, constant: 5),
            footer.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
            footer.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -12),
            footer.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -6),
            previous.widthAnchor.constraint(equalToConstant: 24),
            next.widthAnchor.constraint(equalToConstant: 24),
        ])
    }

    func present(_ snapshot: GannyuSnapshot, anchor: NSRect) {
        let candidates = snapshot.candidates.sorted { $0.pageIndex < $1.pageIndex }
        guard !candidates.isEmpty else { hide(); return }
        rebuildRows(candidates, highlighted: snapshot.highlightedIndex)
        previous.isEnabled = snapshot.hasPreviousPage
        next.isEnabled = snapshot.hasNextPage
        width.constant = panelWidth(for: candidates)
        content.layoutSubtreeIfNeeded()
        let height = content.fittingSize.height
        let frame = NSRect(x: 0, y: 0, width: width.constant, height: height)
        panel.setFrame(frame, display: true)
        position(frame, at: anchor)
        panel.orderFrontRegardless()
    }

    func hide() {
        panel.orderOut(nil)
    }

    private func rebuildRows(_ candidates: [GannyuCandidate], highlighted: Int?) {
        rows.arrangedSubviews.forEach { view in
            rows.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        for candidate in candidates {
            let selected = candidate.pageIndex == highlighted
            let button = CandidateRowButton(candidate: candidate, selected: selected)
            button.target = self
            button.action = #selector(selectCandidate(_:))
            rows.addArrangedSubview(button)
            button.widthAnchor.constraint(equalTo: rows.widthAnchor).isActive = true
        }
    }

    private func panelWidth(for candidates: [GannyuCandidate]) -> CGFloat {
        let primary = NSFont.systemFont(ofSize: 18, weight: .regular)
        let secondary = NSFont.systemFont(ofSize: 12)
        let widest = candidates.reduce(CGFloat(0)) { result, candidate in
            let title = "\(candidate.pageIndex + 1). \(candidate.text)" as NSString
            let note = candidate.annotation.replacingOccurrences(of: "\n", with: " ") as NSString
            return max(result, title.size(withAttributes: [.font: primary]).width,
                       note.size(withAttributes: [.font: secondary]).width + 24)
        }
        return min(max(widest + 32, 240), 440)
    }

    private func position(_ frame: NSRect, at anchor: NSRect) {
        let screen = NSScreen.screens.first(where: { $0.visibleFrame.intersects(anchor) }) ?? NSScreen.main
        guard let visible = screen?.visibleFrame else { return }
        let x = min(max(anchor.minX, visible.minX + 4), visible.maxX - frame.width - 4)
        var y = anchor.minY - frame.height - 6
        if y < visible.minY + 4 { y = min(anchor.maxY + 6, visible.maxY - frame.height - 4) }
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    private func configureFooterButton(_ button: NSButton, action: Selector) {
        button.target = self
        button.action = action
        button.isBordered = false
        button.bezelStyle = .inline
        button.font = .systemFont(ofSize: 14, weight: .medium)
        button.focusRingType = .none
    }

    @objc private func selectCandidate(_ sender: CandidateRowButton) {
        onSelect?(sender.globalIndex)
    }

    @objc private func previousPage() {
        onPage?(-1)
    }

    @objc private func nextPage() {
        onPage?(1)
    }
}

private final class CandidateRowButton: NSButton {
    let globalIndex: Int

    init(candidate: GannyuCandidate, selected: Bool) {
        globalIndex = candidate.globalIndex
        super.init(frame: .zero)
        isBordered = false
        bezelStyle = .regularSquare
        focusRingType = .none
        alignment = .left
        imagePosition = .noImage
        lineBreakMode = .byTruncatingTail
        contentTintColor = selected ? .alternateSelectedControlTextColor : .labelColor
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.backgroundColor = selected ? NSColor.selectedContentBackgroundColor.cgColor : NSColor.clear.cgColor

        let primaryColor = selected ? NSColor.alternateSelectedControlTextColor : NSColor.labelColor
        let secondaryColor = selected ? NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.85) : NSColor.secondaryLabelColor
        let title = NSMutableAttributedString(
            string: "\(candidate.pageIndex + 1). \(candidate.text)",
            attributes: [.font: NSFont.systemFont(ofSize: 18), .foregroundColor: primaryColor]
        )
        if !candidate.annotation.isEmpty {
            title.append(NSAttributedString(
                string: "\n    \(candidate.annotation.replacingOccurrences(of: "\n", with: " "))",
                attributes: [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: secondaryColor]
            ))
        }
        attributedTitle = title
        heightAnchor.constraint(equalToConstant: candidate.annotation.isEmpty ? 31 : 51).isActive = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
