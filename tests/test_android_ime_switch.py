from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SERVICE = ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt"
METHOD = ROOT / "platforms/android/app/src/main/res/xml/method.xml"
JNI = ROOT / "platforms/android/app/src/main/cpp/jni_bridge.c"
RIME_ENGINE = ROOT / "engines/rime/gannyu_rime_engine.cpp"


def test_android_ime_switch_uses_the_standard_platform_path_with_fallback() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    method = METHOD.read_text(encoding="utf-8")

    assert 'android:supportsSwitchingToNextInputMethod="true"' in method
    assert "shouldOfferSwitchingToNextInputMethod()" in source
    assert "switchToNextInputMethod(false)" in source
    assert "showInputMethodPicker()" in source
    assert "Build.VERSION_CODES.P" in source
    assert 'private const val IME_SWITCH_KEY = "🌐"' in source
    assert 'contentDescription = "切换输入法"' in source
    assert "setOnLongClickListener" in source


def test_android_ime_switch_key_is_leftmost_on_all_pages() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    bottom = source.split("private fun renderBottomRow", 1)[1].split("private fun spacer", 1)[0]
    assert bottom.index("listOf(IME_SWITCH_KEY)") < bottom.index("listOf(mode, nav, \"空格\"")
    assert "KeyboardPage.NUMBERS" in source
    assert "KeyboardPage.SYMBOLS" in source
    assert "KeyboardPage.SYMBOLS_MORE" in source
    assert "IME_SWITCH_KEY)" in source.split("private val ACTION_KEYS", 1)[1].splitlines()[0]


def test_android_keyboard_uses_a_system_adaptive_neutral_palette_on_all_pages() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    action = (ROOT / "platforms/android/app/src/main/res/drawable/key_action.xml").read_text(encoding="utf-8")
    normal = (ROOT / "platforms/android/app/src/main/res/drawable/key_normal.xml").read_text(encoding="utf-8")
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")

    dark_action = (ROOT / "platforms/android/app/src/main/res/drawable-night/key_action.xml").read_text(encoding="utf-8")
    dark_normal = (ROOT / "platforms/android/app/src/main/res/drawable-night/key_normal.xml").read_text(encoding="utf-8")
    colors = (ROOT / "platforms/android/app/src/main/res/values/colors.xml").read_text(encoding="utf-8")
    dark_colors = (ROOT / "platforms/android/app/src/main/res/values-night/colors.xml").read_text(encoding="utf-8")
    assert "#FFB8BCC3" in action
    assert "#FFFFFFFF" in normal
    assert "#FF4A4E57" in dark_action
    assert "#FF34373D" in dark_normal
    assert "keyboard_panel" in colors
    assert "#FF1F2125" in dark_colors
    assert "<gradient" not in action
    assert 'android:background="@color/keyboard_panel"' in layout
    assert 'android:textColor="@color/keyboard_text"' in layout
    assert "private val keyTextColor: Int" in source
    assert "private val keySecondaryTextColor: Int" in source
    assert "key.label in ACTION_KEYS" in source
    assert "dp(46)" in source
    assert "FUNCTION_KEY_WIDTH_MULTIPLIER = 1.12f" in source
    assert "private fun functionKeyWidth(label: String, keyWidth: Int): Int" in source
    assert "private val FUNCTION_WIDTH_KEYS" in source


def test_android_bottom_row_places_comma_and_period_after_space() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    bottom = source.split("private fun renderBottomRow", 1)[1].split("private fun spacer", 1)[0]
    assert 'listOf(mode, nav, "空格", if (englishMode) "," else "，", if (englishMode) "." else "。", "↵")' in bottom


def test_android_candidates_keep_full_metadata_with_independent_widths() -> None:
    source = SERVICE.read_text(encoding="utf-8")

    assert "candidateMaxWidth" not in source
    assert "ellipsize = TextUtils.TruncateAt.END" not in source
    assert "marginEnd = dp(3)" in source
    assert "minimumWidth = dp(44)" in source
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")
    assert 'android:layout_height="39dp"' in layout
    assert "candidateExpandButton" in layout
    assert "candidateExpansionContainer" in layout
    assert "nativeChangeCandidatePage" in source
    assert "private fun loadMoreCandidates()" not in source
    assert "textSize = 15f" in source
    assert "setTypeface(Typeface.DEFAULT_BOLD)" in source
    assert 'key.label == "空格" -> ""' in source
    assert "private fun supportsKeyPreview" in source
    assert "private fun showKeyPreview" in source
    assert "keyPreview" in layout
    assert 'android:textSize="28sp"' in layout


def test_android_rime_regions_come_from_the_packaged_manifest() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    store = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/RimeResourceStore.kt").read_text(
        encoding="utf-8"
    )
    jni = JNI.read_text(encoding="utf-8")
    engine = RIME_ENGINE.read_text(encoding="utf-8")

    assert "availableRegions(context: Context)" in source
    assert "RimeResourceStore.regionList(context)" in source
    assert 'optJSONArray("regions")' in store
    assert "nativeRegionList" not in jni
    assert 'region_id && *region_id ? region_id : "lancong"' in engine


def test_android_setup_gates_picker_until_ime_is_enabled() -> None:
    setup = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/SetupActivity.kt").read_text(
        encoding="utf-8"
    )
    layout = (ROOT / "platforms/android/app/src/main/res/layout/activity_setup.xml").read_text(
        encoding="utf-8"
    )

    assert "showEnableImeInstructions" in setup
    assert "enabledInputMethodList" in setup
    assert "openImePickerButton.isEnabled = enabled" in setup
    assert "R.id.imeSetupStatus" not in setup
    assert "statusView.visibility = View.GONE" in setup
    assert layout.index("@+id/setupCard") < layout.index("@+id/regionCard")
