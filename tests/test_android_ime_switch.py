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


def test_android_keyboard_uses_the_fixed_neutral_palette_on_all_pages() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    action = (ROOT / "platforms/android/app/src/main/res/drawable/key_action.xml").read_text(encoding="utf-8")
    normal = (ROOT / "platforms/android/app/src/main/res/drawable/key_normal.xml").read_text(encoding="utf-8")
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")

    assert "#FFB8BCC3" in action
    assert "#FFFFFFFF" in normal
    assert "<gradient" not in action
    assert 'android:background="#D1D3D6"' in layout
    assert 'android:textColor="#636871"' in layout
    assert "private const val KEY_TEXT        = 0xFF1B1D20.toInt()" in source
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
    assert "private fun loadMoreCandidates()" in source


def test_android_rime_regions_start_with_nanchang() -> None:
    source = JNI.read_text(encoding="utf-8")
    engine = RIME_ENGINE.read_text(encoding="utf-8")

    assert source.index('\\"id\\":\\"lancong\\"') < source.index('\\"id\\":\\"fenni\\"')
    assert 'region_id && *region_id ? region_id : "lancong"' in engine
