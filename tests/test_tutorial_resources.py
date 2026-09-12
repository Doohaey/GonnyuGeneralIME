from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_tutorial_resource_contains_the_requested_content() -> None:
    tutorial = (ROOT / "resources/tutorial/tutorial.html").read_text(encoding="utf-8")

    for text in (
        "可使用汉语拼音普通话发音或者赣语拼音输入。显示发音为赣语拼音。",
        "ng为舌根鼻音，例字：五ng3 我ngo3",
        "t/k分别为两种入声。入声是赣语韵尾塞音。t是舌尖处塞音。k是声门塞音。为方便，输入法兼容不输入入声或者输入错误入声的情况。",
        "韵母yu统一采用yu拼写。",
        "南昌词典中，数字1-7为南昌话七个声调，具体调值见下",
        "拼音说明",
        "词语标记说明",
        "A词语后面接“[义]B词语”时，B为A在普通话中的对应义。",
        "A词语后面接“[不习用] [习用]B词语”时，说明对于赣语对应地区，A词语不习用，B词语是较为地道的习用表达。",
        "A词语后面接“[联]B词语”时，是用赣语可用表达对A词语进行解释。",
        "读音前[新][老]分别表示新派与老派读音。新派读音是指近几十年受普通话与城市化影响逐渐普及的发音。",
        "读音前[文][白]分别表示文读音/白读音。文读音属于比较正式词语贴近各时期通用语读音。白读音是本地存在于日常词语的一些读音。同一个词语不同词语可能视词语来源采用不同读音。如“共(qiung5)太公”的“共”就是白读音。",
        "“A读音/[又]B读音”配对表示（词语中）某字可能有两个读音。",
        "更多内容请查看本项目github，欢迎对本项目star支持！",
        "https://github.com/Doohaey/GonnyuGeneralIME",
    ):
        assert text in tutorial


def test_android_and_windows_package_the_same_tutorial_resource() -> None:
    gradle = (ROOT / "platforms/android/app/build.gradle.kts").read_text(encoding="utf-8")
    activity = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/TutorialActivity.kt").read_text(encoding="utf-8")
    setup = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/SetupActivity.kt").read_text(encoding="utf-8")
    windows = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text(encoding="utf-8")
    installer = (ROOT / "platforms/windows/Installer.wxs").read_text(encoding="utf-8")

    assert 'assets.srcDirs("../../../resources/tutorial")' in gradle
    assert 'assets.open("tutorial.html")' in activity
    assert "loadDataWithBaseURL" in activity
    assert "allowFileAccess = false" in activity
    assert "javaScriptEnabled = false" in activity
    assert "R.id.openTutorial" in setup
    assert "OpenTutorial();" in windows


def test_android_candidate_bar_keeps_cache_out_of_the_editor() -> None:
    source = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt").read_text(encoding="utf-8")
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")

    assert 'key.label == "分词"                            -> appendInput(\'\\\'\')' in source
    assert "deleteSurroundingTextInCodePoints(1, 0)" in source
    assert "onUpdateSelection(" in source
    assert "setComposingText" not in source
    assert 'android:id="@+id/cacheTag"' in layout
    assert 'android:id="@+id/preeditView"' not in layout
    assert 'android:translationY="-12dp"' not in layout
    assert "getTextBeforeCursor" not in source


def test_android_symbol_page_clears_composition_and_writes_symbols_literally() -> None:
    source = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt").read_text(encoding="utf-8")

    symbol_page_branch = source.split('key.label == "123"', 1)[1].splitlines()[0]
    assert "resetState(clearAccumulated = true)" in symbol_page_branch
    assert "englishShift = false" in symbol_page_branch
    assert "symbolPage = true" in symbol_page_branch
    assert "renderKeyboard()" in symbol_page_branch
    assert "key.label == \"\\u201C\"" not in source
    assert "key.label == \"\\u201D\"" not in source
    assert "else                                         -> currentInputConnection?.commitText(key.label, 1)" in source
    assert 'KeySpec("\\u232B", 1.5f)' in source
    assert "BACKSPACE_INITIAL_DELAY_MS" in source
    assert "BACKSPACE_REPEAT_INTERVAL_MS" in source
    assert "stopBackspaceRepeat()" in source
    assert 'r4.addView(keyBtn(KeySpec("\\u62FC", 1.2f), gap)); r4.addView(keyBtn(KeySpec("\\uFF08", 1f), gap))' in source
    assert "private var englishMode = false" in source
    assert 'key.label == "\\u82F1" || key.label == "\\u4E2D"' in source
