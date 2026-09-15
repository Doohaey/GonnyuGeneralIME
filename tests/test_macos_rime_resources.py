from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_macos_resource_staging_uses_canonical_mobile_resources() -> None:
    script = (ROOT / "platforms/macos/prepare_rime_resources.sh").read_text(encoding="utf-8")

    assert 'build/rime-mobile/mobile-resources' in script
    assert 'platforms/rime/mobile/prepare_resources.sh' in script
    assert 'resource-manifest.json' in script
    assert '"$source_root/shared"' in script
    assert '"$source_root/prebuilt"' in script
    assert 'ditto "$source_root" "$output_root"' in script
