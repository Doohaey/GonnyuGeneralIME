from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SERVICE = ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt"
METHOD = ROOT / "platforms/android/app/src/main/res/xml/method.xml"


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


def test_android_ime_switch_key_is_the_leftmost_key_on_both_pages() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    pinyin_page = source.split("private fun renderPinyinPage", 1)[1].split(
        "private fun renderSymbolPage", 1
    )[0]
    symbol_page = source.split("private fun renderSymbolPage", 1)[1].split(
        "private fun keyRow", 1
    )[0]

    switch_add = "r4.addView(keyBtn(KeySpec(IME_SWITCH_KEY, 1.1f), gap))"
    assert pinyin_page.index(switch_add) < pinyin_page.index(
        'r4.addView(keyBtn(KeySpec(if (englishMode)'
    )
    assert symbol_page.index(switch_add) < symbol_page.index(
        'r4.addView(keyBtn(KeySpec("\\u62FC", 1.2f), gap))'
    )
    assert "IME_SWITCH_KEY)" in source.split("private val ACTION_KEYS", 1)[1].splitlines()[0]


def test_android_keyboard_uses_the_blue_palette_on_both_pages() -> None:
    source = SERVICE.read_text(encoding="utf-8")
    action = (ROOT / "platforms/android/app/src/main/res/drawable/key_action.xml").read_text(encoding="utf-8")
    normal = (ROOT / "platforms/android/app/src/main/res/drawable/key_normal.xml").read_text(encoding="utf-8")
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")

    for color in ("#FFD2E9F8", "#FF9FC6E2", "#FFE4F3FF", "#FFBFDFF5"):
        assert color in action
    for color in ("#FFEAF4FB", "#FFD1E2F0", "#FFF8FCFF"):
        assert color in normal
    assert 'android:background="#F3F9FE"' in layout
    assert 'android:background="#B3C7E4F7"' in layout
    assert 'android:textColor="#FF365C78"' in layout
    assert "private const val KEY_TEXT        = 0xFF334B5F.toInt()" in source
    assert "private const val ACTION_KEY_TEXT = 0xFF274B64.toInt()" in source
    assert "symbolPage || key.label in ACTION_KEYS" in source
