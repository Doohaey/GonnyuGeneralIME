from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FCITX = ROOT / "platforms" / "linux" / "fcitx5"


def test_fcitx5_builds_first_and_second_scheme_addons() -> None:
    cmake = (FCITX / "CMakeLists.txt").read_text(encoding="utf-8")
    assert 'gonnyu_addon("gonnyu" 0' in cmake
    assert 'gonnyu_addon("gonnyu-ipa" 1' in cmake
    assert 'GANNYU_TRANSCRIPTION_SCHEME=${scheme}' in cmake
    assert 'OUTPUT_NAME "fcitx5-${addon_id}"' in cmake


def test_fcitx5_installs_both_addon_configs() -> None:
    install = (FCITX / "install.sh").read_text(encoding="utf-8")
    assert "ADDONS=(gonnyu gonnyu-ipa)" in install
    assert 'inputmethod/$addon.conf' in install
    assert 'addon/$addon.conf' in install
    package = (FCITX / "package.sh").read_text(encoding="utf-8")
    assert 'stage_addon gonnyu ' in package
    assert 'stage_addon gonnyu-ipa ' in package
    assert "fcitx5-gonnyu.tar.gz" in package
    assert "fcitx5-gonnyu-ipa.tar.gz" in package
    assert 'ADDONS+=("$addon")' in install
    build = (FCITX / "build.sh").read_text(encoding="utf-8")
    assert "for addon in gonnyu gonnyu-ipa" in build
