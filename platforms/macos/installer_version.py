#!/usr/bin/env python3
"""Map the workspace version to an ordered numeric PKG version (as on Windows)."""

import re
import sys


def package_version(value: str) -> str:
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:-pre\.(\d+))?", value)
    if not match:
        raise ValueError(f"invalid workspace version: {value}")
    major, minor, patch = (int(part) for part in match.groups()[:3])
    sequence = match.group(4)
    if sequence is not None and int(sequence) >= 1000:
        raise ValueError("pre-release sequence must be below 1000")
    build = patch * 1001 + (int(sequence) if sequence is not None else 1000)
    if build > 65535:
        raise ValueError("installer build version exceeds 65535")
    return f"{major}.{minor}.{build}"


if __name__ == "__main__":
    try:
        print(package_version(sys.argv[1]))
    except (IndexError, ValueError) as error:
        raise SystemExit(str(error)) from error
