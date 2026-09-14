#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import hashlib
import os
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
LOCK_PATH = Path(__file__).resolve().with_name("engine-lock.json")
OUTPUT = ROOT / "build" / "rime-mobile" / "sources"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(*args: str, cwd: Path | None = None, capture: bool = False) -> str:
    result = subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        env={**os.environ, "GIT_CONFIG_GLOBAL": os.devnull},
    )
    return result.stdout.strip() if capture else ""


def read_lock() -> dict[str, dict[str, str]]:
    lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    for name in ("librime", "librime_lua", "librime_lua_thirdparty"):
        component = lock.get(name, {})
        if not isinstance(component.get("repository"), str) or not isinstance(component.get("commit"), str):
            raise RuntimeError(f"invalid engine lock component: {name}")
        if len(component["commit"]) != 40 or any(char not in "0123456789abcdef" for char in component["commit"]):
            raise RuntimeError(f"engine lock commit must be a full SHA-1: {name}")
    boost = lock.get("boost", {})
    if not all(isinstance(boost.get(field), str) for field in ("version", "archive", "sha256")):
        raise RuntimeError("invalid engine lock component: boost")
    if len(boost["sha256"]) != 64 or any(char not in "0123456789abcdef" for char in boost["sha256"]):
        raise RuntimeError("Boost archive checksum must be a full SHA-256")
    return lock


def checkout(repository: str, commit: str, destination: Path, recursive: bool = False) -> None:
    if destination.exists():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    run("git", "init", destination)
    run("git", "remote", "add", "origin", repository, cwd=destination)
    run("git", "fetch", "--depth", "1", "origin", commit, cwd=destination)
    run("git", "checkout", "--detach", "FETCH_HEAD", cwd=destination)
    if recursive:
        run("git", "submodule", "sync", "--recursive", cwd=destination)
        run("git", "submodule", "update", "--init", "--recursive", cwd=destination)
    actual = run("git", "rev-parse", "HEAD", cwd=destination, capture=True)
    if actual != commit:
        raise RuntimeError(f"engine source revision mismatch: expected {commit}, found {actual}")


def fetch_boost(component: dict[str, str], destination: Path) -> None:
    version = component["version"]
    major, minor, patch = (int(part) for part in version.split("."))
    version_number = major * 100000 + minor * 100 + patch
    version_header = destination / "boost" / "version.hpp"
    if version_header.is_file() and f"#define BOOST_VERSION {version_number}" in version_header.read_text(encoding="utf-8"):
        return

    archive = destination.parent / Path(component["archive"]).name
    if not archive.is_file() or sha256(archive) != component["sha256"]:
        archive.unlink(missing_ok=True)
        partial = archive.with_suffix(archive.suffix + ".partial")
        partial.unlink(missing_ok=True)
        with urllib.request.urlopen(component["archive"]) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        actual = sha256(partial)
        if actual != component["sha256"]:
            partial.unlink(missing_ok=True)
            raise RuntimeError(f"Boost archive checksum mismatch: expected {component['sha256']}, found {actual}")
        partial.replace(archive)

    with tempfile.TemporaryDirectory(prefix="gonnyu-boost-", dir=destination.parent) as temporary:
        temporary_root = Path(temporary)
        with tarfile.open(archive, "r:bz2") as source:
            for member in source.getmembers():
                member_path = Path(member.name)
                if member_path.is_absolute() or ".." in member_path.parts or member.issym() or member.islnk():
                    raise RuntimeError(f"unsafe Boost archive member: {member.name}")
            source.extractall(temporary_root)
        roots = [path for path in temporary_root.iterdir() if path.is_dir()]
        if len(roots) != 1 or not (roots[0] / "boost" / "version.hpp").is_file():
            raise RuntimeError("Boost archive does not contain one complete source tree")
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(roots[0], destination)


def fetch_sources(destination: Path = OUTPUT) -> Path:
    lock = read_lock()
    destination.mkdir(parents=True, exist_ok=True)
    librime = lock["librime"]
    checkout(librime["repository"], librime["commit"], destination / "librime", recursive=True)
    lua = lock["librime_lua"]
    lua_root = destination / "librime" / "plugins" / "librime-lua"
    checkout(lua["repository"], lua["commit"], lua_root)
    thirdparty = lock["librime_lua_thirdparty"]
    checkout(thirdparty["repository"], thirdparty["commit"], lua_root / "thirdparty")
    fetch_boost(lock["boost"], destination / "boost")
    return destination


def fetch_boost_source(destination: Path = OUTPUT) -> Path:
    lock = read_lock()
    destination.mkdir(parents=True, exist_ok=True)
    fetch_boost(lock["boost"], destination / "boost")
    return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--boost-only", action="store_true")
    args = parser.parse_args()
    print(fetch_boost_source() if args.boost_only else fetch_sources())
