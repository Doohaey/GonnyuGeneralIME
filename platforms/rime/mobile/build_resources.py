#!/usr/bin/env python3
"""Build the unencrypted, predeployed Rime resource bundle for mobile apps."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
RIME_BUILD = ROOT / "platforms" / "rime" / "build.py"


def run(*command: str) -> None:
    subprocess.run(command, check=True)


def active_regions() -> list[str]:
    result = subprocess.run(
        (sys.executable, str(RIME_BUILD), "--list-regions"),
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def region_metadata(region: str) -> dict[str, str]:
    config_path = ROOT / "resources" / "regions" / region / "region.toml"
    with config_path.open("rb") as handle:
        config = tomllib.load(handle)
    return {
        "id": region,
        "name_zh": str(config["region"]["name_zh"]),
        "schema_id": f"gannyu_{region}",
    }


def write_manifest(output: Path, regions: list[str]) -> None:
    files = []
    for path in sorted(item for item in output.rglob("*") if item.is_file()):
        relative = path.relative_to(output).as_posix()
        if relative == "resource-manifest.json":
            continue
        files.append(
            {
                "path": relative,
                "size": path.stat().st_size,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
    version = next(
        line.split('"')[1]
        for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines()
        if line.startswith("version = ")
    )
    (output / "resource-manifest.json").write_text(
        json.dumps(
            {
                "product_version": version,
                "schema_version": version,
                "regions": [region_metadata(region) for region in regions],
                "files": files,
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def build(output: Path, deployer: Path) -> None:
    if not deployer.is_file():
        raise FileNotFoundError(f"missing pinned rime_deployer: {deployer}")
    output = output.resolve()
    regions = active_regions()
    with tempfile.TemporaryDirectory(prefix="gonnyu-rime-mobile-") as temporary:
        root = Path(temporary)
        shared = root / "shared"
        user = root / "user"
        prebuilt = user / "build"
        run(sys.executable, str(RIME_BUILD), "--region", "all", "--display-name", "apple", "--output", str(shared))
        run(str(deployer), "--build", str(user), str(shared), str(prebuilt))
        if output.exists():
            shutil.rmtree(output)
        (output / "shared").parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(shared, output / "shared")
        shutil.copytree(prebuilt, output / "prebuilt")
        write_manifest(output, regions)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--deployer", type=Path, required=True)
    args = parser.parse_args()
    build(args.output, args.deployer.resolve())


if __name__ == "__main__":
    main()
