#!/usr/bin/env python3
"""Generate deterministic checksums and an SPDX SBOM for release assets."""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def write_metadata(version: str, output_dir: Path) -> tuple[Path, Path]:
    assets = sorted(
        path for path in output_dir.iterdir() if path.is_file() and path.name not in {
            "SHA256SUMS",
            f"GonnyuGeneralIME-{version}-SBOM.spdx.json",
        }
    )
    if not assets:
        raise ValueError("release asset directory is empty")
    checksums = output_dir / "SHA256SUMS"
    lines = [f"{digest(path)}  {path.name}" for path in assets]
    checksums.write_text("\n".join(lines) + "\n", encoding="utf-8")

    files = []
    relationships = []
    for index, path in enumerate(assets, start=1):
        identifier = f"SPDXRef-File-{index}"
        files.append(
            {
                "SPDXID": identifier,
                "fileName": path.name,
                "checksums": [{"algorithm": "SHA256", "checksumValue": digest(path)}],
                "fileSize": path.stat().st_size,
                "licenseConcluded": "NOASSERTION",
                "licenseInfoInFiles": ["NOASSERTION"],
            }
        )
        relationships.append(
            {
                "spdxElementId": "SPDXRef-ReleaseAssets",
                "relationshipType": "CONTAINS",
                "relatedSpdxElement": identifier,
            }
        )
    sbom = output_dir / f"GonnyuGeneralIME-{version}-SBOM.spdx.json"
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"GonnyuGeneralIME {version} release assets",
        "documentNamespace": f"https://github.com/Doohaey/GonnyuGeneralIME/releases/{version}",
        "creationInfo": {
            "created": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
            "creators": ["Tool: Gonnyu release metadata generator"],
        },
        "packages": [
            {
                "SPDXID": "SPDXRef-ReleaseAssets",
                "name": f"GonnyuGeneralIME-{version}-release-assets",
                "versionInfo": version,
                "downloadLocation": "NOASSERTION",
                "licenseConcluded": "NOASSERTION",
                "licenseDeclared": "NOASSERTION",
            }
        ],
        "files": files,
        "relationships": relationships,
    }
    sbom.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return checksums, sbom


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    checksums, sbom = write_metadata(args.version, args.output_dir)
    print(checksums)
    print(sbom)


if __name__ == "__main__":
    main()
