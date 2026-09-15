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
    assert "build_resources.py" in build
    assert '--output "$output_root/rime"' in build
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
    assert "textDocumentProxy.insertText(key)" in keyboard


def test_ios_keyboard_matches_android_composition_and_default_candidate_rules() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(encoding="utf-8")

    assert 'case "分词":' in keyboard
    assert 'append("\'")' in keyboard
    assert "private func handleSpace()" in keyboard
    assert "if snapshot.rawInput.isEmpty" in keyboard
    assert "engine?.process(.space)" in keyboard
    assert "configuration.subtitle" in keyboard
    assert "systemFont(ofSize: 10, weight: .regular)" in keyboard
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

    assert "UIColor(red: 0.82, green: 0.83, blue: 0.84, alpha: 1)" in keyboard
    assert "equalToConstant: 46" in keyboard
    assert "row.spacing = keySpacing" in keyboard
    assert "private let keySpacing: CGFloat = 6" in keyboard
    assert "case centered" in keyboard
    assert "case deleteExtended" in keyboard
    assert "case bottom" in keyboard
    assert "row.centerXAnchor.constraint(equalTo: container.centerXAnchor)" in keyboard
    assert "functionKeyWidthMultiplier: CGFloat = 1.12" in keyboard
    assert "private func functionKeyWidth(for label: String) -> CGFloat" in keyboard
    assert "case \"🌐\", \"英\", \"中\", \"123\", \"ABC\", \"符号\", \"更多\", \"常用\", \"⇧\", \"分词\":" in keyboard
    assert "multiplier: 1.6" in keyboard
    assert "overrideUserInterfaceStyle = .light" in keyboard
    assert "candidateScroll.backgroundColor = .clear" in keyboard
    assert "candidateRow.heightAnchor.constraint(equalToConstant: 39)" in keyboard
    assert "attributes.font = .systemFont(ofSize: 16)" in keyboard
    assert "attributes.font = .systemFont(ofSize: 10)" in keyboard
    assert "configuration.titleLineBreakMode = expanded ? .byWordWrapping : .byClipping" in keyboard
    assert "configuration.subtitleLineBreakMode = expanded ? .byWordWrapping : .byClipping" in keyboard
    assert "button.setContentCompressionResistancePriority(.required, for: .horizontal)" in keyboard
    assert "private func loadMoreCandidates()" in keyboard
    assert "candidateExpandedHeightConstraint?.constant = 154" in keyboard
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
