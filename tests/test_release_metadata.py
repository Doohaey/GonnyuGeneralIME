import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "platforms/release/generate_release_metadata.py"


def test_release_metadata_contains_all_assets_and_spdx_checksums(tmp_path: Path) -> None:
    (tmp_path / "GonnyuGeneralIME-0.2.4-macos.pkg").write_bytes(b"pkg")
    (tmp_path / "GonnyuGeneralIME-0.2.4-android.apk").write_bytes(b"apk")
    subprocess.run(
        ["python3", str(SCRIPT), "--version", "0.2.4", "--output-dir", str(tmp_path)],
        check=True,
        capture_output=True,
        text=True,
    )
    checksums = (tmp_path / "SHA256SUMS").read_text(encoding="utf-8")
    assert "GonnyuGeneralIME-0.2.4-macos.pkg" in checksums
    assert "GonnyuGeneralIME-0.2.4-android.apk" in checksums
    document = json.loads((tmp_path / "GonnyuGeneralIME-0.2.4-SBOM.spdx.json").read_text())
    assert document["spdxVersion"] == "SPDX-2.3"
    assert len(document["files"]) == 2
    assert all(file["checksums"][0]["algorithm"] == "SHA256" for file in document["files"])


def test_release_workflow_generates_metadata() -> None:
    release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    assert "generate_release_metadata.py" in release
