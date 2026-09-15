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
    assert 'mode_id="$bundle_id.Gan"' in install_script
    assert "<string>$mode_id</string>" in install_script


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
    assert "Gonnyu.icns" in plist
    assert "tsInputModeDisplayNameKey" in plist
    assert "tsInputMethodIconFileKey" in plist
    assert "tsInputModeMenuIconFileKey" in plist
    assert "tsInputModePaletteIconFileKey" in plist
    assert 'icon_resource="$repo_root/resources/Gonnyu.icns"' in (
        ROOT / "platforms/macos/build.sh"
    ).read_text(encoding="utf-8")
    assert (ROOT / "resources/Gonnyu.icns").is_file()
    assert (ROOT / "platforms/macos/Resources/en.lproj/InfoPlist.strings").is_file()
    assert (ROOT / "platforms/macos/Resources/zh-Hans.lproj/InfoPlist.strings").is_file()


def test_macos_uses_native_vertical_candidate_panel() -> None:
    panel = (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuCandidatePanel.swift"
    ).read_text(encoding="utf-8")
    controller = (
        ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift"
    ).read_text(encoding="utf-8")

    assert "NSPanel" in panel
    assert "NSVisualEffectView" in panel
    assert "rows.orientation = .vertical" in panel
    assert "systemFont(ofSize: 18)" in panel
    assert "systemFont(ofSize: 12)" in panel
    assert 'NSButton(title: "<"' in panel
    assert 'NSButton(title: ">"' in panel
    assert "pageLabel" not in panel
    assert "candidatePanel.present" in controller
    assert "candidatePanel.onPage" in controller
    host = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/main.swift").read_text(
        encoding="utf-8"
    )
    assert "guard let server = IMKServer" in host
    assert "self.server = server" in host
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
    assert ".keyDown, .flagsChanged" in controller
    assert "kVK_ANSI_KeypadEnter" in controller
    assert "override func didCommand" in controller
    assert "selectCandidate(globalIndex:" in engine
    assert "client.insertText" in controller
    assert "client.setMarkedText" in controller
    assert "gannyu_engine_create" in engine
    assert "gannyu_engine_process_key" in engine
    assert "gannyu_engine_change_page" in engine
    assert "currentID()" in engine
    assert "GannyuRegion.fallback" not in controller


def test_macos_region_selection_is_validated_against_embedded_catalog() -> None:
    engine = (
        ROOT / "platforms/macos/Sources/GannyuMacOSSupport/GannyuEngine.swift"
    ).read_text(encoding="utf-8")

    assert "availableRegions()" in engine
    assert "regions.contains(where:" in engine
    assert "UserDefaults.standard.set(resolved" in engine
