from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_windows_workflow_runs_installer_smoke_test() -> None:
    workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
    script = (ROOT / "platforms/windows/smoke_installer.ps1").read_text(encoding="utf-8")
    assert "smoke_installer.ps1" in workflow
    assert "/uninstall" in script
    assert "GonnyuGeneralIME\\GannyuTextService.dll" in script
    assert "-notin 0, 3010" in script
