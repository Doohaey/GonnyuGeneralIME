from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_macos_platform_exposes_build_and_smoke_entrypoints() -> None:
    build_script = (ROOT / "platforms/macos/build.sh").read_text(encoding="utf-8")
    smoke_script = (ROOT / "platforms/macos/smoke.sh").read_text(encoding="utf-8")
    host_script = (ROOT / "platforms/macos/run_host.sh").read_text(encoding="utf-8")

    assert 'cargo build -p gannyu-input-ffi --release' in build_script
    assert 'swift build --package-path "$script_dir" -c release' in build_script
    assert "GannyuMacOSSmoke" in smoke_script
    assert "--manifest" in smoke_script
    assert "GannyuInputMethodHost" in host_script


def test_macos_package_declares_host_and_smoke_targets() -> None:
    package = (ROOT / "platforms/macos/Package.swift").read_text(encoding="utf-8")
    assert '.executable(name: "GannyuInputMethodHost"' in package
    assert '.executable(name: "GannyuMacOSSmoke"' in package
    assert '.systemLibrary(' in package
    assert 'link "gannyu_input_ffi"' in (
        ROOT / "platforms/macos/Sources/CGannyuInput/module.modulemap"
    ).read_text(encoding="utf-8")
