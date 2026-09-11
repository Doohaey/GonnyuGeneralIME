"""Tests for the MSI and Burn version mapping in build_installer.bat."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _installer_versions(cargo_version: str) -> tuple[str, str]:
    pre = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)-pre\.(\d+)", cargo_version)
    if pre:
        major, minor, patch, number = (int(part) for part in pre.groups())
        build = patch * 1001 + number
        if number >= 1000 or build > 65535:
            raise ValueError(f"Unsupported pre-release version: {cargo_version}")
        return f"{major}.{minor}.{build}", f"{major}.{minor}.{patch}.{number}"
    stable = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", cargo_version)
    if stable:
        major, minor, patch = (int(part) for part in stable.groups())
        build = patch * 1001 + 1000
        if build > 65535:
            raise ValueError(f"Unsupported stable version: {cargo_version}")
        return f"{major}.{minor}.{build}", f"{major}.{minor}.{patch}.65535"
    raise ValueError(f"Unexpected version format: {cargo_version}")


def _as_ints(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def test_pre_release_versions_keep_msi_order_in_three_fields() -> None:
    assert _installer_versions("0.2.4-pre.8") == ("0.2.4012", "0.2.4.8")
    assert _installer_versions("0.2.4-pre.10") == ("0.2.4014", "0.2.4.10")


def test_stable_version_sorts_after_its_pre_releases() -> None:
    assert _installer_versions("0.2.4") == ("0.2.5004", "0.2.4.65535")
    assert _as_ints(_installer_versions("0.2.4-pre.999")[0]) < _as_ints(
        _installer_versions("0.2.4")[0]
    )


def test_legacy_pre_release_and_future_patch_ordering() -> None:
    assert (0, 2, 4) < _as_ints(_installer_versions("0.2.4-pre.11")[0])
    assert _as_ints(_installer_versions("0.2.4")[0]) < _as_ints(
        _installer_versions("0.2.5-pre.1")[0]
    )


def test_installer_uses_strict_msi_upgrade_ordering() -> None:
    wxs = (ROOT / "platforms/windows/Installer.wxs").read_text(encoding="utf-8")
    assert 'AllowSameVersionUpgrades="yes"' not in wxs


_PS_SNIPPET = """\
$v = '{cargo_ver}'
if($v -match '^(\\d+)\\.(\\d+)\\.(\\d+)-pre\\.(\\d+)$'){{
  $major=[int]$Matches[1];$minor=[int]$Matches[2];$patch=[int]$Matches[3];$n=[int]$Matches[4]
  if($n -ge 1000 -or ($patch*1001+$n) -gt 65535){{exit 1}}
  Write-Output ($major+'.'+$minor+'.'+($patch*1001+$n)+'|'+$major+'.'+$minor+'.'+$patch+'.'+$n)
}} elseif($v -match '^(\\d+)\\.(\\d+)\\.(\\d+)$'){{
  $major=[int]$Matches[1];$minor=[int]$Matches[2];$patch=[int]$Matches[3]
  if(($patch*1001+1000) -gt 65535){{exit 1}}
  Write-Output ($major+'.'+$minor+'.'+($patch*1001+1000)+'|'+$major+'.'+$minor+'.'+$patch+'.65535')
}} else{{exit 1}}
"""


@pytest.mark.skipif(
    __import__("shutil").which("powershell") is None,
    reason="powershell not available on this platform",
)
@pytest.mark.parametrize(
    ("cargo_version", "expected"),
    [
        ("0.2.4-pre.10", ("0.2.4014", "0.2.4.10")),
        ("0.2.4-pre.11", ("0.2.4015", "0.2.4.11")),
        ("0.2.4", ("0.2.5004", "0.2.4.65535")),
    ],
)
def test_powershell_mapping_matches_formula(
    tmp_path: Path, cargo_version: str, expected: tuple[str, str]
) -> None:
    assert _installer_versions(cargo_version) == expected
    script = tmp_path / "version.ps1"
    script.write_text(_PS_SNIPPET.format(cargo_ver=cargo_version), encoding="utf-8")
    result = subprocess.run(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(script)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert tuple(result.stdout.strip().split("|")) == expected
