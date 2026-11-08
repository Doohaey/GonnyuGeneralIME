#!/usr/bin/env python3
"""Build one-scheme and second-scheme Rime archives for one region."""
from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

PLATFORM_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(PLATFORM_DIR))

from build import OPTION_NAMES, VERSION, write_resource_manifest  # noqa: E402


def _files(output: Path) -> list[str]:
    return sorted(
        path.relative_to(output).as_posix()
        for path in output.rglob("*")
        if path.is_file() and path.name != "resource-manifest.json"
    )


def _default_custom(schema_id: str) -> str:
    lines = ["patch:", "  switcher/save_options/+:"]
    lines.extend(f"    - {name}" for name in OPTION_NAMES)
    lines.extend(["  schema_list/+:", f"    - schema: {schema_id}"])
    return "\n".join(lines) + "\n"


def _archive_name(version: str, platform: str, region: str, scheme2: bool) -> str:
    stem = f"GonnyuGeneralIME-{version}-rime-{platform}-{region}" if platform != "universal" else f"GonnyuGeneralIME-{version}-rime-{region}"
    return f"{stem}{'-II' if scheme2 else ''}.zip"


def _package(output: Path, archive_root: Path, region: str, platform: str, *,
             scheme2: bool) -> Path:
    archive_root.mkdir(parents=True, exist_ok=True)
    root = f"gannyu_{region}"
    all_files = _files(output)
    if scheme2:
        selected = [
            name for name in all_files
            if name in {"LICENSE"}
            or name.startswith(f"{root}_ipa") and name.endswith(".schema.yaml")
            or name.startswith(f"{root}") and name.endswith(".dict.yaml")
            or name.startswith(f"lua/{root}") and (
                name.endswith("_data.lua") or name.endswith("_annotations.bin")
            )
            or name == f"lua/{root}_scheme2.lua"
            or name in {
                "lua/gannyu_input_options.lua", "lua/gannyu_options_translator.lua",
                "lua/gannyu_annotation_store.lua", "lua/gannyu_data_lifecycle.lua",
                "lua/gannyu_annotation_filter.lua", "lua/gannyu_single_char_filter.lua",
                "lua/gannyu_relation_filter.lua",
            }
        ]
        schema_id = f"{root}_ipa"
    else:
        selected = [
            name for name in all_files
            if name in {"LICENSE"}
            or name.startswith(f"{root}") and not name.startswith(f"{root}_ipa")
            and (name.endswith(".schema.yaml") or name.endswith(".dict.yaml"))
            or name.startswith(f"lua/{root}") and not name.endswith("_scheme2.lua") and (
                name.endswith("_data.lua") or name.endswith("_annotations.bin")
            )
            or name in {
                "lua/gannyu_input_options.lua", "lua/gannyu_options_translator.lua",
                "lua/gannyu_annotation_store.lua", "lua/gannyu_data_lifecycle.lua",
                "lua/gannyu_annotation_filter.lua", "lua/gannyu_single_char_filter.lua",
                "lua/gannyu_relation_filter.lua",
            }
        ]
        schema_id = root
    if scheme2 and f"{root}_ipa.schema.yaml" not in selected:
        return Path()
    if not selected:
        return Path()
    with tempfile.TemporaryDirectory(prefix=f"gonnyu-rime-package-{region}-") as temporary:
        staging = Path(temporary)
        for name in selected:
            target = staging / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(output / name, target)
        (staging / "default.custom.yaml").write_text(
            _default_custom(schema_id), encoding="utf-8"
        )
        package_files = tuple(sorted([*selected, "default.custom.yaml"]))
        write_resource_manifest(staging, (region,), package_files)
        if scheme2:
            manifest_path = staging / "resource-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["regions"][0]["schema_id"] = schema_id
            manifest_path.write_text(
                json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
        archive = archive_root / _archive_name(VERSION, platform, region, scheme2)
        with ZipFile(archive, "w", ZIP_DEFLATED) as package:
            for name in sorted([*selected, "default.custom.yaml", "resource-manifest.json"]):
                package.write(staging / name, name)
        with ZipFile(archive) as package:
            assert package.testzip() is None
        return archive


def main() -> None:
    output, archive_root, region, platform = sys.argv[1:]
    output = Path(output)
    archive_root = Path(archive_root)
    archive_root.mkdir(parents=True, exist_ok=True)
    _package(output, archive_root, region, platform, scheme2=False)
    _package(output, archive_root, region, platform, scheme2=True)


if __name__ == "__main__":
    main()
