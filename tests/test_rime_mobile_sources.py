import json
from pathlib import Path
from subprocess import run

from platforms.rime.mobile.fetch_sources import LOCK_PATH, checkout, read_lock


def test_mobile_engine_lock_uses_full_pinned_commits() -> None:
    lock = read_lock()

    assert lock["librime"]["tag"] == "1.17.0"
    assert lock["librime"]["commit"] == "33e78140250125871856cdc5b42ddc6a5fcd3cd4"
    assert lock["librime_lua"]["commit"] == "ad1e4a6c98abf634dd34242a747f9b1d5d069fbe"
    assert lock["boost"]["version"] == "1.88.0"
    assert lock["boost"]["sha256"] == "46d9d2c06637b219270877c9e16155cbd015b6dc84349af064c088e9b5b12f7b"


def test_mobile_engine_lock_is_valid_json() -> None:
    raw = json.loads(Path(LOCK_PATH).read_text(encoding="utf-8"))

    assert set(raw) == {"librime", "librime_lua", "librime_lua_thirdparty", "boost"}


def test_checkout_uses_the_locked_revision(tmp_path: Path) -> None:
    source = tmp_path / "source"
    source.mkdir()
    run(("git", "init", source), check=True)
    run(("git", "-C", source, "config", "user.name", "test"), check=True)
    run(("git", "-C", source, "config", "user.email", "test@example.invalid"), check=True)
    (source / "marker").write_text("pinned", encoding="utf-8")
    run(("git", "-C", source, "add", "marker"), check=True)
    run(("git", "-C", source, "-c", "commit.gpgsign=false", "commit", "-m", "test"), check=True)
    revision = run(("git", "-C", source, "rev-parse", "HEAD"), check=True, text=True, capture_output=True).stdout.strip()

    destination = tmp_path / "destination" / "engine"
    checkout(source.as_uri(), revision, destination)

    assert (destination / "marker").read_text(encoding="utf-8") == "pinned"


def test_mobile_host_build_script_uses_pinned_librime_lua_and_manifest() -> None:
    content = (
        Path(__file__).resolve().parents[1]
        / "platforms"
        / "rime"
        / "mobile"
        / "build_host.sh"
    ).read_text(encoding="utf-8")

    assert 'RIME_PLUGINS="librime-lua"' in content
    assert 'platforms/rime/build.py" --region all' in content
    assert "resource-manifest.json" in content


def test_mobile_builds_compile_pinned_boost_regex_without_mutating_librime() -> None:
    root = Path(__file__).resolve().parents[1]
    scripts = [
        root / "platforms" / "rime" / "mobile" / "build_host.sh",
        root / "platforms" / "rime" / "mobile" / "build_android_engine.sh",
    ]
    for script in scripts:
        content = script.read_text(encoding="utf-8")
        assert "boost_regex" in content
        assert "Boost_LIBRARIES" in content
        assert "perl -0pi" not in content
        assert "copy_directory \"$librime_root\"" not in content
        assert "std::regex" not in content


def test_mobile_boost_regex_target_builds_the_required_pinned_sources() -> None:
    content = (
        Path(__file__).resolve().parents[1]
        / "platforms"
        / "rime"
        / "mobile"
        / "boost_regex"
        / "CMakeLists.txt"
    ).read_text(encoding="utf-8")

    for source in (
        "posix_api.cpp",
        "regex.cpp",
        "regex_debug.cpp",
        "static_mutex.cpp",
        "wide_posix_api.cpp",
    ):
        assert source in content
    assert "POSITION_INDEPENDENT_CODE ON" in content


def test_android_link_step_includes_pinned_boost_regex() -> None:
    root = Path(__file__).resolve().parents[1]
    android_cmake = (
        root / "platforms" / "android" / "app" / "src" / "main" / "cpp" / "CMakeLists.txt"
    ).read_text(encoding="utf-8")

    assert "libboost_regex.a" in android_cmake
