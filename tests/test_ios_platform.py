from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / "platforms" / "ios"


def test_ios_platform_declares_host_app_keyboard_extension_and_build_entrypoint() -> None:
    project = (IOS / "GonnyuInput.xcodeproj" / "project.pbxproj").read_text(encoding="utf-8")
    build = (IOS / "build.sh").read_text(encoding="utf-8")

    assert "GonnyuInput" in project
    assert "GonnyuKeyboard" in project
    assert "com.apple.product-type.app-extension" in project
    assert "Embed Keyboard Extension" in project
    assert "GannyuInput/Info.plist" in project
    assert "GannyuKeyboard/Info.plist" in project
    assert "PRODUCT_NAME = GonnyuInputMethod" in project
    assert "PRODUCT_NAME = GonnyuKeyboard" in project
    assert project.count('"-force_load"') == 2
    assert project.count('"$(BUILT_PRODUCTS_DIR)/libgannyu_input_ffi.a"') == 2
    assert "PRODUCT_MODULE_NAME = GonnyuInput" in project
    assert "PRODUCT_MODULE_NAME = GonnyuKeyboard" in project
    assert "Config/Signing.xcconfig" in project
    assert "DEVELOPMENT_TEAM = " not in project
    assert "A00000000000000000000003 /* KeyboardViewController.swift in Sources */" in project
    assert "A00000000000000000000004 /* GannyuAppleEngine.swift in Sources */" in project
    assert "build_ios_xcframework.sh" in build
    assert "prepare_resources.sh" in build
    assert 'ln -sfn ../rime-mobile/mobile-resources "$output_root/rime"' in build
    assert "cargo build" not in build
    assert "GANNYU_RESOURCE_KEY" not in build
    assert "generic/platform=iOS" in build
    assert "archive" in build
    assert "DEVELOPMENT_TEAM" in build
    assert "GANNYU_APP_GROUP" in build


def test_ios_keyboard_and_host_share_manifest_driven_region_selection() -> None:
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(encoding="utf-8")
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")
    host = (IOS / "Sources" / "GannyuInput" / "RegionSettingsViewController.swift").read_text(encoding="utf-8")
    extension_info = (IOS / "GannyuKeyboard" / "Info.plist").read_text(encoding="utf-8")

    assert 'url(forResource: "rime"' in support
    assert "resource-manifest.json" in support
    assert "var hasher = SHA256()" in support
    assert "read(upToCount: 1024 * 1024)" in support
    assert "UserDefaults(suiteName: group)" in support
    assert "?? .standard" not in support
    assert "regions.contains(where:" in support
    assert "GonnyuAppleEngine(" in keyboard
    assert "userDataDirectory: store.userDataDirectory" in keyboard
    assert "store.select(region.id, in: regions)" in host
    assert "com.apple.keyboard-service" in extension_info
    assert "RequestsOpenAccess" in extension_info
    assert "<true/>" in extension_info


def test_ios_keyboard_keeps_auxiliary_input_outside_candidate_selection() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")

    assert "private var keyboardPage: KeyboardPage = .letters" in keyboard
    for page in (".numbers", ".symbols", ".symbolsMore"):
        assert "keyboardPage = " + page in keyboard
    assert 'case "ABC":' in keyboard
    assert "keyboardPage != .letters" in keyboard
    assert "insertLiteral(key)" in keyboard
    assert "private var backspaceTimer: Timer?" in keyboard
    assert "private var englishMode = false" in keyboard
    assert "private var englishShift = false" in keyboard
    assert "backspacePressed" in keyboard
    assert "stopBackspaceRepeat" in keyboard
    assert "override func viewWillDisappear" in keyboard
    assert "Timer.scheduledTimer(withTimeInterval: 0.38" in keyboard
    assert "Timer.scheduledTimer(withTimeInterval: 0.055, repeats: true)" in keyboard
    assert "textDocumentProxy.insertText(key)" in keyboard
    assert "private final class KeyPreviewView" in keyboard
    assert "private let bubbleLayer = CAShapeLayer()" in keyboard
    assert "UIColor.secondarySystemBackground.resolvedColor" in keyboard
    assert "label.font = .systemFont(ofSize: 28, weight: .bold)" in keyboard
    assert "private func supportsKeyPreview" in keyboard
    assert "showKeyPreview" in keyboard
    assert 'label == "空格" ? ""' in keyboard


def test_ios_keyboard_matches_android_composition_and_default_candidate_rules() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(encoding="utf-8")

    assert 'case "分词":' in keyboard
    assert 'append("\'")' in keyboard
    assert "private func handleSpace()" in keyboard
    assert "if snapshot.rawInput.isEmpty" in keyboard
    assert "engine?.process(.space)" in keyboard
    assert "configuration.subtitle" in keyboard
    assert "开启“允许完全访问”后，用户词库才能保存" in keyboard
    assert "systemFont(ofSize: 12, weight: .bold)" in keyboard
    assert "heightAnchor.constraint(equalToConstant: 16)" in keyboard
    assert "candidateStack.spacing = 3" in keyboard
    assert "leading: 5, bottom: 1, trailing: 5" in keyboard
    assert "engine?.selectCandidate(globalIndex: index)" in keyboard
    assert "snapshot = (try? engine?.snapshot()) ?? .empty" in keyboard
    assert "gannyu_engine_process_key" in support
    assert "gannyu_engine_select_candidate" in support
    assert "gannyu_engine_change_page" in support
    assert "gannyu_engine_create" in support
    assert "shared_data_dir" in support
    assert "prebuilt_data_dir" in support
    assert "containerURL" in support
    assert "userDataDirectory" in support


def test_ios_keyboard_uses_compact_fixed_visuals_and_full_annotations() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(
        encoding="utf-8"
    )

    assert "traits.userInterfaceStyle == .dark" in keyboard
    assert "equalToConstant: 46" in keyboard
    assert "row.spacing = keySpacing" in keyboard
    assert "private let keySpacing: CGFloat = 6" in keyboard
    assert "case centered" in keyboard
    assert "case deleteExtended" in keyboard
    assert "case bottom" in keyboard
    assert "row.centerXAnchor.constraint(equalTo: container.centerXAnchor)" in keyboard
    assert "functionKeyWidthMultiplier: CGFloat = 1.12" in keyboard
    assert "private func functionKeyWidth(for label: String) -> CGFloat" in keyboard
    assert "case \"分词\":" in keyboard
    assert "multiplier: 1.6" in keyboard
    assert "overrideUserInterfaceStyle = .light" not in keyboard
    assert "traitCollectionDidChange" in keyboard
    assert "candidateScroll.backgroundColor = .clear" in keyboard
    assert "candidateRow.heightAnchor.constraint(equalToConstant: 39)" in keyboard
    assert "attributes.font = .systemFont(ofSize: 16, weight: .regular)" in keyboard
    assert "attributes.font = .systemFont(ofSize: 10)" in keyboard
    assert "configuration.titleLineBreakMode = .byClipping" in keyboard
    assert "configuration.subtitleLineBreakMode = .byClipping" in keyboard
    assert "button.setContentCompressionResistancePriority(.required, for: .horizontal)" in keyboard
    assert "private func loadMoreCandidates()" not in keyboard
    assert "candidateExpandedScroll.topAnchor.constraint(equalTo: view.topAnchor, constant: 20)" in keyboard
    assert "if candidateExpanded {\n            collapseCandidateExpansion()" in keyboard
    assert "lessThanOrEqualTo: candidateScroll.frameLayoutGuide.widthAnchor" not in keyboard
    assert "NSLayoutConstraint.activate(pendingWidthConstraints)" in keyboard
    assert keyboard.index("keyboardStack.addArrangedSubview(keyRow(\n            [\"🌐\"") < keyboard.index(
        "NSLayoutConstraint.activate(pendingWidthConstraints)"
    )
    assert "UIImage(systemName: \"globe\")" in keyboard
    assert '"空格", englishMode ? "," : "，"' in keyboard


def test_ios_host_matches_android_user_data_controls() -> None:
    host = (IOS / "Sources" / "GannyuInput" / "RegionSettingsViewController.swift").read_text(
        encoding="utf-8"
    )
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(
        encoding="utf-8"
    )

    assert "gannyu_engine_reset_user_data" in support
    assert "requestUserDataReset" in support
    assert "pendingUserDataResetRegionIDs" in support
    assert "清空当前地区学习数据" in host
    assert "清空全部地区学习数据" in host


def test_ios_host_uses_standard_settings_sections_and_offline_tutorial() -> None:
    host = (IOS / "Sources" / "GannyuInput" / "RegionSettingsViewController.swift").read_text(
        encoding="utf-8"
    )
    tutorial = (IOS / "Sources" / "GannyuInput" / "TutorialViewController.swift").read_text(
        encoding="utf-8"
    )
    project = (IOS / "GonnyuInput.xcodeproj" / "project.pbxproj").read_text(encoding="utf-8")

    assert "case setup" in host
    assert "case regions" in host
    assert "case userData" in host
    assert "case help" in host
    assert "设置 > 通用 > 键盘 > 键盘 > 添加新键盘…" in host
    assert "UIApplication.openSettingsURLString" in host
    assert "用户词库仅在本机离线存储。" in host
    assert "开启允许完全访问" in host
    assert "openAppSettings" in host
    assert "TutorialViewController()" in host
    assert "WKWebView" in tutorial
    assert 'url(forResource: "tutorial", withExtension: "html")' in tutorial
    assert "tutorial.html in Resources" in project
