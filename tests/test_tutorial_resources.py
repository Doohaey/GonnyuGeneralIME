from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


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


def test_android_backspace_uses_grapheme_safe_deletion() -> None:
    source = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt").read_text(encoding="utf-8")
    layout = (ROOT / "platforms/android/app/src/main/res/layout/input_view.xml").read_text(encoding="utf-8")

    assert 'key.label == "分词"                            -> appendInput(\'\\\'\')' in source
    assert "onUpdateSelection(" in source
    assert "setComposingText" not in source
    assert 'android:id="@+id/cacheTag"' in layout
    assert 'android:id="@+id/preeditView"' not in layout
    assert 'android:translationY="-12dp"' not in layout
    assert "getTextBeforeCursor(32, 0)" in source
    assert "BreakIterator.getCharacterInstance(Locale.ROOT)" in source
    assert "deleteSurroundingTextInCodePoints(codePointCount, 0)" in source
