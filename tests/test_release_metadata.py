from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
def test_release_metadata_generation_is_removed() -> None:
    """Release downloads contain platform artifacts only."""
    assert not (ROOT / "platforms/release/generate_release_metadata.py").exists()


def test_release_workflow_does_not_publish_metadata_files() -> None:
    release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    assert "generate_release_metadata.py" not in release
    assert "SHA256SUMS" not in release
    assert "SBOM" not in release
