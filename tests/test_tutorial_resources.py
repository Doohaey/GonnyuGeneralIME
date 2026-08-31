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
        "A词语后面接“[官]B词语”时，A为赣语对应地区常用表达，B为普通话/官话区常用表达。",
        "A词语后面接“[官话词][赣]B词语”时，说明对于赣语对应地区，A词语不常用，B词语是较为地道表达。",
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
