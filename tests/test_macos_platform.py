from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_macos_platform_exposes_build_and_smoke_entrypoints() -> None:
    build_script = (ROOT / "platforms/macos/build.sh").read_text(encoding="utf-8")
    smoke_script = (ROOT / "platforms/macos/smoke.sh").read_text(encoding="utf-8")
    bundle_smoke_script = (ROOT / "platforms/macos/bundle_smoke.sh").read_text(encoding="utf-8")
    host_script = (ROOT / "platforms/macos/run_host.sh").read_text(encoding="utf-8")
    install_script = (ROOT / "platforms/macos/install_local.sh").read_text(encoding="utf-8")

    assert 'cargo build -p gannyu-input-ffi --release' in build_script
    assert 'swift build --package-path "$script_dir" -c release' in build_script
    assert "Info.plist.template" in build_script
    assert "GonnyuInputMethod.app" in build_script
    assert "GannyuMacOSSmoke" in smoke_script
    assert "--manifest" in smoke_script
    assert "plutil -lint" in bundle_smoke_script
    assert "GANNYU_IMK_SELFTEST=1" in bundle_smoke_script
    assert 'Contents/MacOS/GannyuInputMethodHost' in host_script
    assert "~/Library/Input Methods" in install_script or 'Library/Input Methods' in install_script


def test_macos_package_declares_host_and_smoke_targets() -> None:
    package = (ROOT / "platforms/macos/Package.swift").read_text(encoding="utf-8")
    assert '.executable(name: "GannyuInputMethodHost"' in package
    assert '.executable(name: "GannyuMacOSSmoke"' in package
    assert '.systemLibrary(' in package
    assert 'link "gannyu_input_ffi"' in (
        ROOT / "platforms/macos/Sources/CGannyuInput/module.modulemap"
    ).read_text(encoding="utf-8")
    assert '#include "../../../../crates/ffi/include/gannyu_input.h"' in (
        ROOT / "platforms/macos/Sources/CGannyuInput/gannyu_input.h"
    ).read_text(encoding="utf-8")
    assert "GannyuInputController" in (
        ROOT / "platforms/macos/Info.plist.template"
    ).read_text(encoding="utf-8")
    assert "tsInputMethodCharacterRepertoireKey" in (
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
    assert "override func didCommand" in controller
    assert "commitCandidate(at: 0" in controller
    assert "client.insertText" in controller
    assert "client.setMarkedText" in controller
    assert "retrieveCandidates" in engine
    assert "formatPreedit" in engine
    assert "currentID(manifestPath:" in engine
    assert "GannyuRegion.fallback" not in controller


def test_macos_region_selection_is_validated_against_embedded_catalog() -> None:
    engine = (
        ROOT / "platforms/macos/Sources/GannyuMacOSSupport/GannyuEngine.swift"
    ).read_text(encoding="utf-8")

    assert "availableRegions(manifestPath:" in engine
    assert "regions.contains(where:" in engine
    assert "UserDefaults.standard.set(resolved" in engine
