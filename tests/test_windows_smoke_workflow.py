from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_windows_workflow_runs_installer_smoke_test() -> None:
    workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
    script = (ROOT / "platforms/windows/smoke_installer.ps1").read_text(encoding="utf-8")
    assert "smoke_installer.ps1" in workflow
    assert "/uninstall" in script
    assert "GonnyuGeneralIME\\GannyuTextService.dll" in script
    assert "GonnyuGeneralIME\\tutorial.html" in script
    assert "-notin 0, 3010" in script


def test_windows_toolbar_stays_visible_without_candidates() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    candidate_update = source.split("void UpdateCandidateWindow()", 1)[1].split("void HideCandidateWindow", 1)[0]
    hide_candidate = source.split("void HideCandidateWindow()", 1)[1].split("void PaintCandidateWindow", 1)[0]
    assert "ShowWindow(statusWindow_, SW_HIDE)" not in candidate_update
    assert "statusWindow_" not in hide_candidate
    assert "STDMETHODIMP Deactivate() override" in source
    assert "std::wstring label = englishMode_ ? L\"英\" : L\"中\";" in source
