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
    assert "if (ownProfile)" in callback
    assert "if (profileActive_)" in callback
    assert "UpdateStatusBar()" in callback
    assert "ShowWindow(statusWindow_, SW_HIDE)" in callback
    assert "profileActive_ = activated != FALSE" in callback
    assert "profileCookie_" in source
    assert "ResetShiftState()" in focus_callback
    assert "SetActiveContext(nullptr)" in focus_callback
    assert "ShowWindow(statusWindow_, SW_HIDE)" in focus_callback
    assert "UpdateStatusBar()" in focus_callback
    assert "profileActive_ && EnsureStatusBar()" in focus_callback


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

    assert "kSearchBoxIntegrationStyleGuid" in element
    assert "0xe6d1bd11" in source
    assert "*eaten = TRUE" in element
    assert "*show = TRUE" in element


def test_windows_ui_less_candidate_pages_are_bounded_and_host_configurable() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    element = source.split("class GannyuCandidateListUiElement", 1)[1].split("int ScaleForDpi", 1)[0]

    assert "RebuildDefaultPages()" in element
    assert "start += kVisibleCandidateCount" in element
    assert "std::copy(pageIndexes_.begin(), pageIndexes_.end(), index)" in element
    assert "STDMETHODIMP SetPageIndex(UINT *index, UINT count)" in element
    assert "std::upper_bound(pageIndexes_.begin(), pageIndexes_.end()" in element


def test_windows_ui_less_candidate_snapshot_keeps_its_owner_alive() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    element = source.split("class GannyuCandidateListUiElement", 1)[1].split("int ScaleForDpi", 1)[0]
    ui_update = source.split("void UpdateCandidateUiElement()", 1)[1].split("void EndCandidateUiElement", 1)[0]

    assert "IUnknown *owner" in element
    assert "owner_->AddRef()" in element
    assert "if (owner_) owner_->Release()" in element
    assert "std::vector<CandidateItem> items_" in element
    assert "UpdateSnapshot(candidates_, selectedIndex_)" in ui_update
    assert "generation == contextGeneration_" in ui_update


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


def test_windows_commits_and_compositions_move_the_caret_to_the_range_end() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    insert = source.split("class InsertTextEditSession", 1)[1].split("struct CompositionState", 1)[0]
    composition = source.split("class CompositionEditSession", 1)[1].split("class SelectionRectEditSession", 1)[0]

    assert "HRESULT SetSelectionAtRangeEnd" in source
    assert "TF_ANCHOR_END" in source
    assert "SetSelectionAtRangeEnd(context_, editCookie, range)" in insert
    assert composition.count("SetSelectionAtRangeEnd(context_, editCookie, range)") >= 4
    assert "state_->context != context_" in composition


def test_windows_focus_changes_isolate_composition_state_and_hide_stale_ui() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    context = source.split("void SetActiveContext(ITfContext *context)", 1)[1].split("LONG refs_", 1)[0]

    assert "RequestCompositionEdit(activeContext_, CompositionEditAction::Cancel)" in context
    assert "ClearInputModel()" in context
    assert "++contextGeneration_" in context
    assert "compositionState_ = std::make_shared<CompositionState>()" in context
    assert "unsigned long long contextGeneration_" in source


def test_windows_candidate_popup_uses_the_anchor_monitor_work_area() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    update = source.split("void UpdateCandidateWindow()", 1)[1].split("void HideCandidateWindow", 1)[0]

    assert "MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST)" in update
    assert "workArea = monitorInfo.rcWork" in update
    assert "anchor.top - popupSize_.cy" in update


def test_windows_regular_apps_keep_the_native_candidate_path() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    commit = source.split("bool CommitText(", 1)[1].split("bool RequestCompositionEdit", 1)[0]
    ui_update = source.split("void UpdateCandidateUiElement()", 1)[1].split("void EndCandidateUiElement", 1)[0]

    assert "if (uiLessMode_" in commit
    assert "if (!uiLessMode_)" in ui_update
    assert "InsertTextEditSession" in commit


def test_windows_enables_composition_only_for_ui_less_threads() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    activation = source.split("STDMETHODIMP ActivateEx", 1)[1].split("STDMETHODIMP Deactivate", 1)[0]
    refresh = source.split("void RefreshCandidates()", 1)[1].split("void UpdateCandidateUiElement", 1)[0]

    assert "IID_ITfThreadMgr2" in activation
    assert "GetActiveFlags" in activation
    assert "TF_TMF_UIELEMENTENABLEDONLY" in activation
    assert "uiLessMode_ = (activeFlags" in activation
    assert "uiLessMode_ && activeContext_" in refresh


def test_windows_toolbar_clicks_use_drawn_button_rectangles() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    assert "auto drawButton" in source
    assert "PtInRect(&englishButtonRect_, point)" in source
    assert "PtInRect(&punctuationButtonRect_, point)" in source
    assert "PtInRect(&regionButtonRect_, point)" in source
    assert "PtInRect(&userDataButtonRect_, point)" in source
    assert "PtInRect(&tutorialButtonRect_, point)" in source
