#!/usr/bin/env python3
"""Generate core second-scheme lookup tables from the stored dictionary IPA.

The Core engine and the native Fcitx5 addons consume plain TSV lookup tables
under ``regions/<region>/transcription/``.  This script derives those tables
from the canonical regional dictionaries so the public repository can build a
complete bundle without any private tooling.
"""

from __future__ import annotations

import argparse
import csv
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

PLATFORM_DIR = Path(__file__).resolve().parent
ROOT = PLATFORM_DIR.parents[1]
if str(PLATFORM_DIR) not in sys.path:
    sys.path.insert(0, str(PLATFORM_DIR))

import transcription  # noqa: E402

DICTIONARY_KEYS = ("chars", "gan_chars", "words", "gan_words")


@dataclass(frozen=True)
class Reading:
    ipa: str
    gan: str


def load_config(resources_root: Path, region: str) -> tuple[Path, dict]:
    region_dir = resources_root / "regions" / region
    with (region_dir / "region.toml").open("rb") as handle:
        config = tomllib.load(handle)
    return region_dir, config


def load_readings(region_dir: Path, config: dict) -> list[Reading]:
    language = config.get("language", {}) or {}
    readings: list[Reading] = []
    for key in DICTIONARY_KEYS:
        relative = language.get(key)
        if not relative:
            continue
        path = region_dir / relative
        if not path.is_file():
            continue
        with path.open(encoding="utf-8", newline="") as handle:
            for row in csv.DictReader(handle, delimiter="\t"):
                readings.append(
                    Reading(
                        ipa=(row.get("国际音标") or "").strip(),
                        gan=(row.get("方言拼音") or "").strip(),
                    )
                )
    return readings


def write_pairs(path: Path, header: tuple[str, str], pairs) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write("\t".join(header) + "\n")
        for first, second in sorted(set(pairs)):
            handle.write(f"{first}\t{second}\n")


def build_region(region: str, resources_root: Path, output_root: Path) -> bool:
    region_dir, config = load_config(resources_root, region)
    enabled, ee_e_opposition, oo_o_opposition = transcription.scheme2_config(config)
    if not enabled:
        return False
    readings = load_readings(region_dir, config)
    gon_map = transcription.build_gon_map(
        readings,
        ee_e_opposition=ee_e_opposition,
        oo_o_opposition=oo_o_opposition,
    )
    if not gon_map:
        return False
    target = output_root / region / "transcription"
    gon_pairs = [(gon, scheme2) for gon, values in gon_map.items() for scheme2 in values]
    write_pairs(target / "gon_to_scheme2.tsv", ("scheme1", "scheme2"), gon_pairs)
    write_pairs(
        target / "tone_map.tsv",
        ("scheme1", "scheme2"),
        ((marker, value) for marker, value in transcription.build_tone_map(config).items()
         if marker != "0"),
    )
    reverse_pairs = [(scheme2, gon) for gon, scheme2 in gon_pairs]
    write_pairs(target / "scheme2_map.tsv", ("scheme2", "scheme1"), reverse_pairs)
    return True


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", action="append", help="Region ID; default: all")
    parser.add_argument(
        "--root",
        type=Path,
        default=ROOT / "resources",
        help="Region resource root (default: <repo>/resources)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "build" / "transcription",
        help="Output root for generated transcription tables",
    )
    args = parser.parse_args()
    resources_root = args.root.resolve()
    output_root = args.output.resolve()
    selected = args.region or [
        path.parent.name
        for path in sorted((resources_root / "regions").glob("*/region.toml"))
    ]
    generated = 0
    for region in selected:
        if build_region(region, resources_root, output_root):
            generated += 1
            print(region)
    if generated == 0:
        raise SystemExit("no second-scheme regions generated")


if __name__ == "__main__":
    main()
