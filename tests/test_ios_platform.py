from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / "platforms" / "ios"


def test_ios_platform_declares_host_app_keyboard_extension_and_build_entrypoint() -> None:
    project = (IOS / "GannyuInput.xcodeproj" / "project.pbxproj").read_text(encoding="utf-8")
    build = (IOS / "build.sh").read_text(encoding="utf-8")

    assert "GannyuInput" in project
    assert "GannyuKeyboard" in project
    assert "com.apple.product-type.app-extension" in project
    assert "Embed Keyboard Extension" in project
    assert "GannyuInput/Info.plist" in project
    assert "GannyuKeyboard/Info.plist" in project
    assert "PRODUCT_NAME = GonnyuInputMethod" in project
    assert "PRODUCT_NAME = GonnyuKeyboard" in project
    assert "files = (A00000000000000000000003, A00000000000000000000004, );" in project
    assert "aarch64-apple-ios" in build
    assert "aarch64-apple-ios-sim" in build
    assert "x86_64-apple-ios" in build
    assert "-create-xcframework" in build
    assert "lipo -create" in build
    assert "generic/platform=iOS" in build
    assert "archive" in build
    assert "DEVELOPMENT_TEAM" in build
    assert "GANNYU_APP_GROUP" in build


def test_ios_keyboard_and_host_share_manifest_driven_region_selection() -> None:
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(encoding="utf-8")
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")
    host = (IOS / "Sources" / "GannyuInput" / "RegionSettingsViewController.swift").read_text(encoding="utf-8")
    extension_info = (IOS / "GannyuKeyboard" / "Info.plist").read_text(encoding="utf-8")

    assert "gannyu_region_list" in support
    assert "UserDefaults(suiteName: group)" in support
    assert "?? .standard" not in support
    assert "regions.contains(where:" in support
    assert "GannyuAppleEngine(" in keyboard
    assert "userDataDirectory: store.userDataDirectory" in keyboard
    assert "store.select(region.id, in: regions)" in host
    assert "com.apple.keyboard-service" in extension_info
    assert "RequestsOpenAccess" in extension_info
    assert "<true/>" in extension_info


def test_ios_keyboard_keeps_symbol_input_outside_candidate_selection() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")

    assert "private var symbolPage = false" in keyboard
    assert "symbolPage = true" in keyboard
    assert "symbolPage = false" in keyboard
    assert '? ["🌐", "（", "）", "空格", "“", "⌫", "⏎", "拼"]' in keyboard
    assert "textDocumentProxy.insertText(key)" in keyboard


def test_ios_keyboard_matches_android_composition_and_default_candidate_rules() -> None:
    keyboard = (IOS / "Sources" / "GannyuKeyboard" / "KeyboardViewController.swift").read_text(encoding="utf-8")
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(encoding="utf-8")

    assert 'case "分词":' in keyboard
    assert 'append("\'")' in keyboard
    assert "private func handleSpace()" in keyboard
    assert "if buffer.isEmpty" in keyboard
    assert "private func commitComposingIfNeeded()" in keyboard
    assert "configuration.subtitle" in keyboard
    assert "candidate.consumedBytes" in keyboard
    assert "saveAccumulatedUserWord()" in keyboard
    assert "gannyu_pipeline_user_dict_add" in support
    assert "gannyu_pipeline_create_with_user_data_dir" in support
    assert "containerURL" in support
    assert "userDataDirectory" in support


def test_ios_host_matches_android_user_data_controls() -> None:
    host = (IOS / "Sources" / "GannyuInput" / "RegionSettingsViewController.swift").read_text(
        encoding="utf-8"
    )
    support = (IOS / "Sources" / "GannyuAppleSupport" / "GannyuAppleEngine.swift").read_text(
        encoding="utf-8"
    )

    assert "GannyuAppleUserDataScope" in support
    assert "gannyu_pipeline_user_data_clear" in support
    assert "清空用户词" in host
    assert "清空学习词频" in host
    assert "清空全部用户数据" in host
