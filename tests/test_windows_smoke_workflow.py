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
    assert "drawButton(englishMode_ ? L\"英\" : L\"中\", englishButtonRect_);" in source


def test_windows_toolbar_tracks_active_input_profile() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    callback = source.split("STDMETHODIMP OnActivated(", 1)[1].split("STDMETHODIMP OnInitDocumentMgr", 1)[0]
    focus_callback = source.split("STDMETHODIMP OnSetFocus(BOOL", 1)[1].split("STDMETHODIMP OnTestKeyDown", 1)[0]
    assert "public ITfActiveLanguageProfileNotifySink" in source
    assert "IID_ITfActiveLanguageProfileNotifySink" in source
    assert "CLSID_GannyuTextService" in callback
    assert "GannyuProfileGuid" in callback
    assert "if (activated)" in callback
    assert "UpdateStatusBar()" in callback
    assert "ShowWindow(statusWindow_, SW_HIDE)" in callback
    assert "profileCookie_" in source
    assert "statusWindow_" not in focus_callback


def test_windows_toolbar_clicks_use_drawn_button_rectangles() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    assert "auto drawButton" in source
    assert "PtInRect(&englishButtonRect_, point)" in source
    assert "PtInRect(&punctuationButtonRect_, point)" in source
    assert "PtInRect(&regionButtonRect_, point)" in source
    assert "PtInRect(&userDataButtonRect_, point)" in source
    assert "PtInRect(&tutorialButtonRect_, point)" in source
