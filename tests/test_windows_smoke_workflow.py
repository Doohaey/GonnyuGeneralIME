from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_windows_workflow_runs_installer_smoke_test() -> None:
    workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
    script = (ROOT / "platforms/windows/smoke_installer.ps1").read_text(encoding="utf-8")
    assert "smoke_installer.ps1" in workflow
    assert "/uninstall" in script
    assert "GonnyuGeneralIME\\x64\\GannyuTextService.dll" in script
    assert "GonnyuGeneralIME\\x86\\GannyuTextService.dll" in script
    assert "GonnyuGeneralIME\\tutorial.html" in script
    assert "-notin 0, 3010" in script
    assert "Get-PeMachine" in script
    assert "0x8664" in script
    assert "0x014c" in script
    assert 'Assert-ComRegistration "64"' in script
    assert 'Assert-ComRegistration "32"' in script
    assert 'Assert-ComRegistrationAbsent "64"' in script
    assert 'Assert-ComRegistrationAbsent "32"' in script
    assert 'if: always()' in workflow
    assert "windows-installer-smoke-logs" in workflow


def test_windows_installer_builds_and_registers_both_process_architectures() -> None:
    workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
    build = (ROOT / "platforms/windows/build_installer.bat").read_text(encoding="utf-8")
    cmake = (ROOT / "platforms/windows/GannyuTextService/CMakeLists.txt").read_text(encoding="utf-8")
    installer = (ROOT / "platforms/windows/Installer.wxs").read_text(encoding="utf-8")

    assert "rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc" in workflow
    assert "--target x86_64-pc-windows-msvc" in build
    assert "--target i686-pc-windows-msvc" in build
    assert "cmake-x64" in build
    assert "cmake-x86" in build
    assert ":load_vs" in build
    assert "-arch=x64" not in build
    assert "-arch=%~1" in build
    assert "DllPathX64" in build
    assert "DllPathX86" in build
    assert "GANNYU_FFI_LIBRARY" in cmake
    assert "oleaut32" in cmake
    assert installer.count('Bitness="always64"') == 1
    assert installer.count('Bitness="always32"') == 1
    assert 'Directory="System64Folder"' in installer
    assert 'Directory="SystemFolder"' in installer
    assert "GannyuTextServiceDllX64" in installer
    assert "GannyuTextServiceDllX86" in installer
    assert "RegisterGannyuTextServiceX64" in installer
    assert "RegisterGannyuTextServiceX86" in installer


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
    document_focus = source.split("STDMETHODIMP OnSetFocus(ITfDocumentMgr", 1)[1].split("STDMETHODIMP OnPushContext", 1)[0]
    focus_callback = source.split("STDMETHODIMP OnSetFocus(BOOL", 1)[1].split("STDMETHODIMP OnTestKeyDown", 1)[0]
    activation = source.split("STDMETHODIMP ActivateEx", 1)[1].split("STDMETHODIMP Deactivate", 1)[0]
    deactivation = source.split("STDMETHODIMP Deactivate", 1)[1].split("STDMETHODIMP OnActivated", 1)[0]
    assert "public ITfActiveLanguageProfileNotifySink" in source
    assert "IID_ITfActiveLanguageProfileNotifySink" in source
    assert "CLSID_GannyuTextService" in callback
    assert "GannyuProfileGuid" in callback
    assert "if (ownProfile)" in callback
    assert "if (profileActive_ && foregroundFocused_)" in callback
    assert "UpdateStatusBar()" in callback
    assert "DestroyCandidateWindow()" in callback
    assert "profileActive_ = activated != FALSE" in callback
    assert "profileCookie_" in source
    assert "foregroundFocused_ = IsCurrentThreadForeground()" in activation
    assert "foregroundFocused_ && EnsureStatusBar()" in activation
    assert "DestroyCandidateWindow()" in deactivation
    assert "documentMgr != nullptr && IsCurrentThreadForeground()" in document_focus
    assert "DestroyCandidateWindow()" in document_focus
    assert "ResetShiftState()" in focus_callback
    assert "SetActiveContext(nullptr)" in focus_callback
    assert "DestroyCandidateWindow()" in focus_callback
    assert "UpdateStatusBar()" in focus_callback
    assert "profileActive_ && EnsureStatusBar()" in focus_callback
    assert "GetWindowThreadProcessId(foreground, nullptr) == GetCurrentThreadId()" in source


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
    assert "GUID_TFCAT_TIPCAP_COMLESS" in categories
    assert "GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT" in categories
    assert "GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT" in categories


def test_windows_registration_is_idempotent_across_bitness_views() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    registration = source.split("HRESULT RegisterTextServiceProfile", 1)[1].split("HRESULT UnregisterTextServiceProfile", 1)[0]

    assert "RegisterCategory(CLSID_GannyuTextService" in registration
    assert "if (hr == TF_E_ALREADY_EXISTS)" in registration
    assert "hr = S_OK;" in registration


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
        "GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT",
    ):
        assert unsupported not in categories


def test_windows_syncs_the_tsf_input_mode_compartments() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    installer = (ROOT / "platforms/windows/Installer.wxs").read_text(encoding="utf-8")

    assert "public ITfCompartmentEventSink" in source
    assert "STDMETHODIMP OnChange(REFGUID compartment) override" in source
    assert "void AdviseInputModeCompartments()" in source
    assert "void UnadviseInputModeCompartments()" in source
    assert "SetKeyboardOpen(TRUE);" in source
    assert "SetKeyboardConversionMode(englishMode_);" in source
    assert "TF_CONVERSIONMODE_NATIVE" in source
    assert "SetKeyboardOpen(englishMode_ ? FALSE : TRUE)" not in source
    assert source.count("const bool valid = SUCCEEDED(get) && value.vt == VT_I4;") >= 2
    assert "GUID_COMPARTMENT_KEYBOARD_OPENCLOSE" in source
    assert "GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION" in source
    assert 'Condition=\'NOT (REMOVE="ALL")\'' in installer


def test_windows_ui_less_candidates_follow_searchbox_contract() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    element = source.split("class GannyuCandidateListUiElement", 1)[1].split("int ScaleForDpi", 1)[0]

    assert "kSearchBoxIntegrationStyleGuid" in element
    assert "0xe6d1bd11" in source
    assert "*eaten = TRUE" in element
    assert "*show = TRUE" in element
    assert "TF_CLUIE_DOCUMENTMGR" in element
    assert "if (show_) show_(show);" in element


def test_windows_ui_less_candidates_apply_the_show_state_after_beginning() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    ui_update = source.split("void UpdateCandidateUiElement()", 1)[1].split("void EndCandidateUiElement", 1)[0]

    assert "candidateUi_->Show(show);" in ui_update
    assert "candidateUiShown_ = show;" in ui_update
    assert "UpdateCandidateWindow();" in ui_update
    assert "HideCandidateWindow();" in ui_update


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


def test_windows_composition_tracks_buffer_caret_and_commits_at_range_end() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    insert = source.split("class InsertTextEditSession", 1)[1].split("struct CompositionState", 1)[0]
    composition = source.split("class CompositionEditSession", 1)[1].split("class SelectionRectEditSession", 1)[0]

    assert "HRESULT SetSelectionAtRangeEnd" in source
    assert "HRESULT SetSelectionAtRangeOffset" in source
    assert "TF_ANCHOR_END" in source
    assert "SetSelectionAtRangeEnd(context_, editCookie, range)" in insert
    assert composition.count("SetSelectionAtRangeOffset(context_, editCookie, range, caret_)") == 2
    assert composition.count("SetSelectionAtRangeEnd(context_, editCookie, range)") >= 2
    assert "state_->context != context_" in composition


def test_windows_buffer_supports_middle_editing() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")

    assert "size_t cursor_ = 0" in source
    assert "buffer_.insert(cursor_" in source
    assert "buffer_.erase(cursor_ - 1, 1)" in source
    assert "buffer_.erase(cursor_, 1)" in source
    assert "key == VK_LEFT || key == VK_RIGHT" in source
    assert "preeditCursor_" in source


def test_windows_rime_page_keys_include_minus_and_equal() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")

    assert "key == VK_OEM_COMMA || key == VK_OEM_MINUS" in source
    assert "key == VK_OEM_PERIOD || key == VK_OEM_PLUS" in source


def test_windows_candidate_enumerator_is_safe_for_x86_size_types() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    enumerator = source.split("class GannyuCandidateEnum", 1)[1].split("class GannyuCandidateList", 1)[0]

    assert "const size_t skipped = std::min(remaining, static_cast<size_t>(count));" in enumerator
    assert "return skipped == static_cast<size_t>(count) ? S_OK : S_FALSE;" in enumerator


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
    assert "uiLessMode_ = (effectiveFlags & TF_TMF_UIELEMENTENABLEDONLY) != 0" in activation
    assert "immersiveMode_ = (effectiveFlags & TF_TMF_IMMERSIVEMODE) != 0" in activation
    assert "TF_TMF_UIELEMENTENABLEDONLY | TF_TMF_IMMERSIVEMODE" not in activation
    assert "uiLessMode_ && activeContext_" in refresh


def test_windows_immersive_popup_is_owned_by_the_active_view() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    ensure = source.split("bool EnsureCandidateWindow()", 1)[1].split("bool EnsureStatusBar", 1)[0]

    assert "immersiveMode_ && TryGetContextViewWindow(&ownerWindow)" in ensure
    assert "GetAncestor(ownerWindow, GA_ROOT)" in ensure
    assert "ownerProcessId != GetCurrentProcessId()" in ensure
    assert "GWLP_HWNDPARENT" in ensure


def test_windows_toolbar_clicks_use_drawn_button_rectangles() -> None:
    source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    assert "auto drawButton" in source
    assert "PtInRect(&englishButtonRect_, point)" in source
    assert "PtInRect(&punctuationButtonRect_, point)" in source
    assert "PtInRect(&regionButtonRect_, point)" in source
    assert "PtInRect(&userDataButtonRect_, point)" in source
    assert "PtInRect(&tutorialButtonRect_, point)" in source
