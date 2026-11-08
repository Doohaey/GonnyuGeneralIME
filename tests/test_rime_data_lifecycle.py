import os
from pathlib import Path
import shutil
import subprocess

import pytest


def test_rime_data_lifecycle() -> None:
    lua = os.environ.get("GANNYU_LUA_BIN") or next(
        (binary for name in ("lua5.4", "lua54", "lua") if (binary := shutil.which(name))),
        None,
    )
    if lua is None:
        pytest.skip("Lua interpreter is unavailable")
    root = Path(__file__).resolve().parents[1]
    subprocess.run(
        [lua, str(root / "tests/lua/rime_data_lifecycle.lua")],
        env={**os.environ, "GANNYU_RIME_LUA_DIR": str(root / "platforms/rime")},
        check=True,
    )


def test_rime_input_options_without_extra_marker() -> None:
    lua = os.environ.get("GANNYU_LUA_BIN") or shutil.which("lua")
    if lua is None:
        pytest.skip("Lua interpreter is unavailable")
    root = Path(__file__).resolve().parents[1]
    subprocess.run(
        [lua, str(root / "tests/lua/rime_input_options.lua")],
        env={**os.environ, "GANNYU_RIME_LUA_DIR": str(root / "platforms/rime")},
        check=True,
    )


def test_rime_annotation_scheme2() -> None:
    lua = os.environ.get("GANNYU_LUA_BIN") or shutil.which("lua")
    if lua is None:
        pytest.skip("Lua interpreter is unavailable")
    root = Path(__file__).resolve().parents[1]
    subprocess.run(
        [lua, str(root / "tests/lua/rime_annotation_scheme2.lua")],
        env={**os.environ, "GANNYU_RIME_LUA_DIR": str(root / "platforms/rime")},
        check=True,
    )
