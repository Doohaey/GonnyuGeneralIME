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


def test_windows_final_text_commit_allows_tsf_default_composition() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    session = source.split("class InsertTextEditSession", 1)[1].split("struct CompositionState", 1)[0]

    assert "InsertTextAtSelection(" in session
    assert "TF_IAS_NO_DEFAULT_COMPOSITION" not in session.replace("// composition: TF_IAS_NO_DEFAULT_COMPOSITION", "")
    assert "            0," in session


def test_windows_registers_its_ui_less_candidate_capabilities() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    categories = source.split("static const GUID kSupportedCategories[]", 1)[1].split("};", 1)[0]

    assert "GUID_TFCAT_TIP_KEYBOARD" in categories
    assert "GUID_TFCAT_TIPCAP_UIELEMENTENABLED" in categories
    assert "GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT" in categories


def test_windows_search_provider_wiring_is_present() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    categories = source.split("static const GUID kSupportedCategories[]", 1)[1].split("};", 1)[0]
    assert "ITfFnSearchCandidateProvider" in source
    assert "ITfFunctionProvider" in source
    assert "ITfCandidateList" in source
    assert "IEnumTfCandidates" in source
    assert "AdviseSingleSink(clientId_, IID_ITfFunctionProvider" in source
    assert "GetSearchCandidates" in source
    for unsupported in (
        "GUID_TFCAT_TIPCAP_SECUREMODE",
        "GUID_TFCAT_TIPCAP_COMLESS",
        "GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT",
        "GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT",
    ):
        assert unsupported not in categories


def test_windows_ui_less_candidates_follow_searchbox_contract() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    element = source.split("class GannyuCandidateListUiElement", 1)[1].split("int ScaleForDpi", 1)[0]

    assert "GUID_INTEGRATIONSTYLE_SEARCHBOX" in element
    assert "*eaten = TRUE" in element
    assert "*show = TRUE" in element


def test_windows_preedit_is_a_real_tsf_composition() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    session = source.split("class CompositionEditSession", 1)[1].split("class SelectionRectEditSession", 1)[0]

    assert "public ITfCompositionSink" in source
    assert "IID_ITfCompositionSink" in source
    assert "ITfContextComposition" in session
    assert "TF_IAS_NO_DEFAULT_COMPOSITION" in session
    assert "StartComposition" in session
    assert "EndComposition" in session
    assert "CompositionEditAction::Update" in source
    assert "CompositionEditAction::Commit" in source
    assert "CompositionEditAction::Cancel" in source


def test_windows_toolbar_clicks_use_drawn_button_rectangles() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    assert "auto drawButton" in source
    assert "PtInRect(&englishButtonRect_, point)" in source
    assert "PtInRect(&punctuationButtonRect_, point)" in source
    assert "PtInRect(&regionButtonRect_, point)" in source
    assert "PtInRect(&userDataButtonRect_, point)" in source
    assert "PtInRect(&tutorialButtonRect_, point)" in source
