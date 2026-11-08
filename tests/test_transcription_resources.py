import subprocess
import sys
from pathlib import Path

from platforms.rime.build_transcription_resources import ROOT, build_region

RESOURCES = ROOT / "resources"
SCRIPT = ROOT / "platforms" / "rime" / "build_transcription_resources.py"


def test_build_region_writes_core_lookup_tables(tmp_path: Path) -> None:
    assert build_region("lancong", RESOURCES, tmp_path)
    target = tmp_path / "lancong" / "transcription"
    gon = (target / "gon_to_scheme2.tsv").read_text(encoding="utf-8").splitlines()
    tone = (target / "tone_map.tsv").read_text(encoding="utf-8").splitlines()
    assert gon[0] == "scheme1\tscheme2"
    assert "ba\tpa" in gon
    assert tone[0] == "scheme1\tscheme2"
    assert "4\t5" in tone
    assert (target / "scheme2_map.tsv").read_text(encoding="utf-8").splitlines()[0] == "scheme2\tscheme1"


def test_build_region_skips_first_scheme_only_regions(tmp_path: Path) -> None:
    assert not build_region("fenni", RESOURCES, tmp_path)
    assert not (tmp_path / "fenni").exists()


def test_cli_enumerates_regions_under_resources_root(tmp_path: Path) -> None:
    root = tmp_path / "resources"
    (root / "regions").mkdir(parents=True)
    (root / "regions" / "lancong").symlink_to(
        RESOURCES / "regions" / "lancong", target_is_directory=True
    )
    output = tmp_path / "output"
    subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(root), "--output", str(output)],
        check=True,
    )
    assert (output / "lancong" / "transcription" / "gon_to_scheme2.tsv").is_file()
