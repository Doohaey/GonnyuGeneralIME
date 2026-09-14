#!/usr/bin/env python3

import json
import os
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
LOCK_PATH = Path(__file__).resolve().with_name("engine-lock.json")
OUTPUT = ROOT / "build" / "rime-mobile" / "sources"


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
    for name in ("librime", "librime_lua"):
        component = lock.get(name, {})
        if not isinstance(component.get("repository"), str) or not isinstance(component.get("commit"), str):
            raise RuntimeError(f"invalid engine lock component: {name}")
        if len(component["commit"]) != 40 or any(char not in "0123456789abcdef" for char in component["commit"]):
            raise RuntimeError(f"engine lock commit must be a full SHA-1: {name}")
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


def fetch_sources(destination: Path = OUTPUT) -> Path:
    lock = read_lock()
    destination.mkdir(parents=True, exist_ok=True)
    librime = lock["librime"]
    checkout(librime["repository"], librime["commit"], destination / "librime", recursive=True)
    lua = lock["librime_lua"]
    checkout(lua["repository"], lua["commit"], destination / "librime" / "plugins" / "librime-lua")
    return destination


if __name__ == "__main__":
    print(fetch_sources())
