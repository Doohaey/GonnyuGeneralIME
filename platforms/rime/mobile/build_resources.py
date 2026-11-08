#!/usr/bin/env python3
"""Build the unencrypted, predeployed Rime resource bundle for mobile apps."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build_cache


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


def region_metadata(region: str) -> dict[str, object]:
    config_path = ROOT / "resources" / "regions" / region / "region.toml"
    with config_path.open("rb") as handle:
        config = tomllib.load(handle)
    schemes = (config.get("transcription") or {}).get("schemes", ["gon-pin"])
    if isinstance(schemes, str):
        schemes = [schemes]
    return {
        "id": region,
        "name_zh": str(config["region"]["name_zh"]),
        "schema_id": f"gannyu_{region}",
        "transcription_schemes": [str(scheme) for scheme in schemes],
    }


def write_manifest(output: Path, regions: list[str]) -> None:
    files = []
    for path in sorted(item for item in output.rglob("*") if item.is_file()):
        relative = path.relative_to(output).as_posix()
        if relative == "resource-manifest.json" or path.name.startswith(".gannyu-cache-"):
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


def cache_files(shared: Path, include_manifest: bool = False) -> list[Path]:
    return [
        path for path in shared.rglob("*")
        if path.is_file()
        and not path.name.startswith(".gannyu-cache-")
        and (include_manifest or path.name != "resource-manifest.json")
    ]


def deployment_fingerprint(shared: Path, deployer: Path) -> str:
    records = []
    for path in sorted(cache_files(shared)):
        relative = path.relative_to(shared).as_posix()
        content = path.read_bytes()
        if relative == "default.yaml":
            content = re.sub(
                rb'(?m)^config_version: "[^"]+"$',
                b'config_version: "<resource-config>"',
                content,
                count=1,
            )
        records.append((relative, hashlib.sha256(content).hexdigest()))
    options = build_cache.digest(deployer)
    return hashlib.sha256(json.dumps([options, records], sort_keys=True).encode()).hexdigest()


def deployment_inputs_match(previous: Path, current: Path) -> bool:
    def records(root: Path) -> dict[str, bytes]:
        result = {}
        for path in cache_files(root):
            relative = path.relative_to(root).as_posix()
            content = path.read_bytes()
            if relative == "default.yaml":
                content = re.sub(
                    rb'(?m)^config_version: "[^"]+"$',
                    b'config_version: "<resource-config>"',
                    content,
                    count=1,
                )
            result[relative] = content
        return result

    return records(previous) == records(current)


def publish_shared(output: Path, shared: Path, regions: list[str]) -> None:
    current_shared = output / "shared"
    if current_shared.exists():
        shutil.rmtree(current_shared)
    shutil.move(str(shared), current_shared)
    write_manifest(output, regions)


def build(output: Path, deployer: Path) -> None:
    if not deployer.is_file():
        raise FileNotFoundError(f"missing pinned rime_deployer: {deployer}")
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    regions = active_regions()
    previous_shared = output / "shared"
    previous_source_key = None
    if previous_shared.is_dir():
        previous_files = cache_files(previous_shared, include_manifest=True)
        previous_source_key = build_cache.fingerprint(
            previous_shared, previous_files, build_cache.digest(deployer)
        )
    with tempfile.TemporaryDirectory(prefix="gonnyu-rime-mobile-", dir=output.parent) as temporary:
        root = Path(temporary)
        shared = root / "shared"
        user = root / "user"
        prebuilt = user / "build"
        if (output / "shared").is_dir():
            shutil.copytree(output / "shared", shared, copy_function=shutil.copy2)
        removed_regions = []
        for stamp in shared.glob(".gannyu-cache-*.json"):
            region = stamp.name[len(".gannyu-cache-"):-len(".json")]
            if region not in regions:
                removed_regions.append(region)
                for prefix in (f"gannyu_{region}", f"konny_{region}"):
                    for path in shared.rglob(f"{prefix}*"):
                        if path.is_file():
                            path.unlink()
                stamp.unlink()
        run(sys.executable, str(RIME_BUILD), "--region", "all", "--display-name", "apple", "--output", str(shared))
        source_key = deployment_fingerprint(shared, deployer)
        deployment_stamp = output / ".gannyu-cache-deployment.json"
        deployment_cache = build_cache.load(deployment_stamp, output, source_key)
        if deployment_cache is not None:
            print("resource-cache predeployment: hit", flush=True)
            publish_shared(output, shared, regions)
            build_cache.save(
                deployment_stamp,
                output,
                source_key,
                [path for path in (output / "prebuilt").rglob("*") if path.is_file()],
                deployment_cache["counts"],
            )
            return
        if (
            previous_source_key
            and build_cache.load(deployment_stamp, output, previous_source_key) is not None
            and deployment_inputs_match(previous_shared, shared)
        ):
            print("resource-cache predeployment: migrated hit", flush=True)
            publish_shared(output, shared, regions)
            build_cache.save(
                deployment_stamp,
                output,
                source_key,
                [path for path in (output / "prebuilt").rglob("*") if path.is_file()],
                {"engine": build_cache.digest(deployer)},
            )
            return
        previous = {}
        try:
            previous = json.loads(deployment_stamp.read_text())
        except (OSError, ValueError):
            pass
        if previous.get("counts", {}).get("engine") == build_cache.digest(deployer) and (output / "prebuilt").is_dir():
            shutil.copytree(output / "prebuilt", prebuilt, copy_function=shutil.copy2)
            for name, checksum in previous.get("files", {}).items():
                old = output / name
                staged = prebuilt / Path(name).relative_to("prebuilt")
                if not old.is_file() or build_cache.digest(old) != checksum:
                    staged.unlink(missing_ok=True)
        for region in removed_regions:
            for prefix in (f"gannyu_{region}", f"konny_{region}"):
                for path in prebuilt.glob(f"{prefix}*"):
                    if path.is_file():
                        path.unlink()
        if prebuilt.is_dir():
            for region in regions:
                for path in prebuilt.glob(f"gannyu_{region}_II*"):
                    if path.is_file():
                        path.unlink()
        for region in regions:
            before = output / "shared" / f".gannyu-cache-{region}.json"
            after = shared / f".gannyu-cache-{region}.json"
            if before.is_file() and after.is_file():
                try:
                    same = json.loads(before.read_text()).get("key") == json.loads(after.read_text()).get("key")
                except (OSError, ValueError, AttributeError):
                    same = False
                print(f"resource-cache predeployment [{region}] {'reuse eligible' if same else 'changed'}", flush=True)
        print("resource-cache predeployment: validate changed inputs", flush=True)
        run(str(deployer), "--build", str(user), str(shared), str(prebuilt))
        if output.exists():
            shutil.rmtree(output)
        output.mkdir(parents=True)
        shutil.move(str(shared), output / "shared")
        shutil.move(str(prebuilt), output / "prebuilt")
        write_manifest(output, regions)
        build_cache.save(output / ".gannyu-cache-deployment.json", output, source_key,
                         [path for path in (output / "prebuilt").rglob("*") if path.is_file()],
                         {"engine": build_cache.digest(deployer)})


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--deployer", type=Path, required=True)
    args = parser.parse_args()
    build(args.output, args.deployer.resolve())


if __name__ == "__main__":
    main()
