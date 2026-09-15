from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_macos_rime_build_uses_pinned_mobile_engine_sources() -> None:
    script = (ROOT / "platforms/macos/build_rime_engine.sh").read_text(encoding="utf-8")

    assert 'mobile_dir="$repo_root/platforms/rime/mobile"' in script
    assert 'fetch_sources.py' in script
    assert 'GANNYU_MACOS_ARCHITECTURES:-arm64;x86_64' in script
    assert 'CMAKE_OSX_DEPLOYMENT_TARGET="$deployment_target"' in script
    assert 'RIME_PLUGINS="librime-lua"' in script
    assert 'libgannyu_rime_engine.a' in script
    assert 'lipo -verify_arch arm64' in script
    assert 'lipo -verify_arch x86_64' in script
