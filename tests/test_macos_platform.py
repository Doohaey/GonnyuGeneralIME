from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_macos_platform_exposes_build_and_smoke_entrypoints() -> None:
    build_script = (ROOT / "platforms/macos/build.sh").read_text(encoding="utf-8")
    smoke_script = (ROOT / "platforms/macos/smoke.sh").read_text(encoding="utf-8")
    bundle_smoke_script = (ROOT / "platforms/macos/bundle_smoke.sh").read_text(encoding="utf-8")
    host_script = (ROOT / "platforms/macos/run_host.sh").read_text(encoding="utf-8")
    install_script = (ROOT / "platforms/macos/install_local.sh").read_text(encoding="utf-8")

    assert 'cargo build -p gannyu-input-ffi --release' not in build_script
    assert 'build_rime_engine.sh' in build_script
    assert 'prepare_rime_resources.sh' in build_script
    assert 'Contents/Resources/rime' in build_script
    assert 'swift build --package-path "$script_dir" -c release' in build_script
    assert '--arch arm64 --arch x86_64' in build_script
    assert 'lipo "$bundle_root/Contents/MacOS/GannyuInputMethodHost" -verify_arch arm64 x86_64' in build_script
    assert "Info.plist.template" in build_script
    assert "GonnyuInputMethod.app" in build_script
    assert 'requested_signing_identity="${GANNYU_MACOS_SIGN_IDENTITY:-}"' in build_script
    assert 'export GANNYU_MACOS_SIGN_IDENTITY="$requested_signing_identity"' in build_script
    assert "GannyuMacOSSmoke" in smoke_script
    assert "--input" in smoke_script
    assert "plutil -lint" in bundle_smoke_script
    assert "GANNYU_IMK_SELFTEST=1" in bundle_smoke_script
    assert 'Contents/MacOS/GannyuInputMethodHost' in host_script
    assert "~/Library/Input Methods" in install_script or 'Library/Input Methods' in install_script
    assert "defaults export com.apple.HIToolbox" in install_script
    assert 'mktemp -d "${TMPDIR:-/private/tmp}/gonnyu-imk-install.XXXXXX"' in install_script
    assert 'trap \'rm -rf "$staging_dir"\' EXIT' in install_script
    assert 'lsregister" -u "$bundle"' in install_script
    assert 'lsregister" -f "$target_bundle"' in install_script
    assert 'source_id="$bundle_id.Gan"' in install_script
    assert 'InputSourceKind</key><string>Input Mode</string>' in install_script
    assert 'entry_mode" == "$bundle_id.Gan"' in install_script


def test_macos_package_declares_host_and_smoke_targets() -> None:
    package = (ROOT / "platforms/macos/Package.swift").read_text(encoding="utf-8")
    assert '.executable(name: "GannyuInputMethodHost"' in package
    assert '.executable(name: "GannyuMacOSSmoke"' in package
    assert '.systemLibrary(' in package
    assert 'link "gannyu_input_ffi"' not in (
        ROOT / "platforms/macos/Sources/CGannyuInput/module.modulemap"
    ).read_text(encoding="utf-8")
    assert '#include "../../../../crates/ffi/include/gannyu_input.h"' in (
        ROOT / "platforms/macos/Sources/CGannyuInput/gannyu_input.h"
    ).read_text(encoding="utf-8")
    assert "GannyuInputController" in (
        ROOT / "platforms/macos/Info.plist.template"
    ).read_text(encoding="utf-8")
    plist = (ROOT / "platforms/macos/Info.plist.template").read_text(encoding="utf-8")
    assert "Gonnyu 赣语键盘" in plist
    assert "Gonny.icns" in plist
    assert "TISIconIsTemplate" in plist
    assert "tsInputMethodIconFileKey" in plist
    assert "ComponentInputModeDict" in plist
    assert 'icon_resource="$repo_root/resources/icon.png"' in (
        ROOT / "platforms/macos/build.sh"
    ).read_text(encoding="utf-8")
    assert (ROOT / "resources/icon.png").is_file()
    assert "iconutil -c icns" in (
        ROOT / "platforms/macos/build.sh"
    ).read_text(encoding="utf-8")
    assert (ROOT / "platforms/macos/Resources/en.lproj/InfoPlist.strings").is_file()
    assert (ROOT / "platforms/macos/Resources/zh-Hans.lproj/InfoPlist.strings").is_file()


def test_macos_installer_handles_relocated_user_input_method() -> None:
    package = (ROOT / "platforms/macos/package.sh").read_text(encoding="utf-8")
    common = (ROOT / "platforms/macos/Scripts/common.sh").read_text(encoding="utf-8")
    preinstall = (ROOT / "platforms/macos/Scripts/preinstall.template").read_text(encoding="utf-8")
    postinstall = (ROOT / "platforms/macos/Scripts/postinstall").read_text(encoding="utf-8")

    assert 'cp "$script_dir/Scripts/common.sh" "$stage_dir/scripts/common.sh"' in package
    assert "gonny_user_target" in common
    assert "Users/$console_user" in common
    assert "Detailed log:" in common
    assert "source \"$scripts_dir/common.sh\"" in preinstall
    assert "source \"$scripts_dir/common.sh\"" in postinstall
    assert "gonnyu_resolve_target" in preinstall
    assert "gonnyu_resolve_target" in postinstall


def test_macos_uses_custom_candidates_with_native_positioning_fallback() -> None:
    panel = (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuCandidatePanel.swift"
    ).read_text(encoding="utf-8")
    controller = (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift"
    ).read_text(encoding="utf-8")

    assert "IMKCandidates" not in controller
    assert "kVK_ANSI_9" in controller
    assert "selectedLine" in controller
    assert "moveSelection" in controller
    assert "candidate.text" in controller
    assert "GannyuPageHint" in controller
    assert "GannyuModeHint" in controller
    assert 'NSButton(title: "<"' in controller
    assert 'NSButton(title: ">"' in controller
    assert "candidatePanel.present(state, selectedLine: selectedLine, anchor: anchor)" in controller
    assert "candidatePanel.hide()" in controller
    assert "validCaretRect" in controller
    assert "rect.width >= 0" in controller
    assert "rect.height > 0" in controller
    assert "rect.intersectsAnyScreen" in controller
    assert "return active ? moveSelection(-1, client: sender) : false" in controller
    assert "return active ? moveSelection(1, client: sender) : false" in controller
    assert "candidateLineNumber(event.keyCode)" in controller
    assert "private func showModeHint" in controller
    assert "lastCandidateAnchor" in controller
    assert "candidateWindow?.setCandidateData([NSAttributedString(" not in controller
    assert 'case "moveUp:"' in controller
    assert 'case "moveDown:"' in controller
    assert "fullwidthPunctuation" in controller
    assert "fullwidthSymbol" in controller
    assert "rows.spacing = 0" in panel
    assert "selectedLine: Int" in panel
    assert 'NSButton(title: "<"' in panel
    assert 'NSButton(title: ">"' in panel
    host = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/main.swift").read_text(
        encoding="utf-8"
    )
    assert "guard let server = IMKServer" in host
    assert "self.server = server" in host
    assert "kIMKSingleColumnScrollingCandidatePanel" in host
    assert "IMKCandidatesOpacityAttributeName" in host
    assert "IMKCandidatesSendServerKeyEventFirst" in host
    assert "tsInputMethodCharacterRepertoireKey" in (
        ROOT / "platforms/macos/Info.plist.template"
    ).read_text(encoding="utf-8")
    assert "<key>LSBackgroundOnly</key>" not in (
        ROOT / "platforms/macos/Info.plist.template"
    ).read_text(encoding="utf-8")
    assert "@objc(GannyuInputController)" in (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift"
    ).read_text(encoding="utf-8")


def test_macos_controller_wires_minimal_input_loop() -> None:
    controller = (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift"
    ).read_text(encoding="utf-8")
    engine = (
        ROOT / "platforms/macos/Sources/GannyuMacOSSupport/GannyuEngine.swift"
    ).read_text(encoding="utf-8")

    assert "override func inputText" in controller
    assert "override func handle" in controller
    assert "@objc(handleEvent:client:)" in controller
    assert "override func recognizedEvents" in controller
    assert "charactersIgnoringModifiers" in controller
    assert "Unable to create Rime engine" in controller
    assert "Rime engine failed to process input" in controller
    assert "EventTypeMask.keyDown.rawValue" in controller
    assert "IMK inputText received" in controller
    assert "IMK keyDown received" in controller
    assert "kVK_ANSI_KeypadEnter" in controller
    assert "kVK_ANSI_Comma" in controller
    assert "kVK_ANSI_Period" in controller
    assert "kVK_ANSI_Minus" in controller
    assert "kVK_ANSI_Equal" in controller
    assert "kVK_LeftArrow" in controller
    assert "kVK_RightArrow" in controller
    assert "process(.moveLeft" in controller
    assert "process(.moveRight" in controller
    assert "process(.deleteForward" in controller
    assert "IMKCandidates" not in controller
    assert "flagsChanged.rawValue" in controller
    assert "setASCIIMode" in controller
    assert "override func didCommand" in controller
    assert "selectCandidate(globalIndex:" in engine
    assert "client.insertText" in controller
    assert "client.setMarkedText" in controller
    assert "as? IMKTextInput" in controller
    assert "for client: IMKTextInput" in controller
    assert "as? NSTextInputClient" not in controller
    assert "gannyu_engine_create" in engine
    assert "gannyu_engine_process_key" in engine
    assert "gannyu_engine_change_page" in engine
    assert "gannyu_engine_set_ascii_mode" in engine
    assert 'case moveLeft' in engine
    assert 'case moveRight' in engine
    assert 'case deleteForward' in engine
    assert "currentID()" in engine
    assert "GannyuRegion.fallback" not in controller


def test_macos_keeps_the_transparent_imk_candidate_window_with_the_server() -> None:
    host = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/main.swift").read_text(
        encoding="utf-8"
    )

    assert "private var candidateWindow: IMKCandidates?" in host
    assert "candidateWindow = IMKCandidates(" in host
    assert "IMKCandidatesOpacityAttributeName" in host
    assert "IMKCandidatesSendServerKeyEventFirst" in host


def test_macos_build_reuses_complete_rime_caches_until_their_inputs_change() -> None:
    build_script = (ROOT / "platforms/macos/build.sh").read_text(encoding="utf-8")

    assert "GANNYU_MACOS_FORCE_RIME_REBUILD" in build_script
    assert "reusing cached macOS librime engine" in build_script
    assert "reusing cached macOS Rime resources" in build_script
    assert '"$repo_root/engines/rime/gannyu_rime_engine.cpp"' in build_script
    assert "dependencies/cache/macos/rime-engine" in build_script
    assert "build/rime-macos-resources" in build_script

def test_macos_region_selection_is_validated_against_embedded_catalog() -> None:
    engine = (
        ROOT / "platforms/macos/Sources/GannyuMacOSSupport/GannyuEngine.swift"
    ).read_text(encoding="utf-8")

    assert "availableRegions()" in engine
    assert "regions.contains(where:" in engine
    assert "UserDefaults.standard.set(resolved" in engine
