"""Tests for the Windows installer version mapping in build_installer.bat.

build_installer.bat converts a Cargo semver string to a 4-part version used
for both MSI ProductVersion and Bundle Version:

  Pre     X.Y.Z-pre.N -> X.Y.Z.N      (N < 65535)
  Stable  X.Y.Z       -> X.Y.Z.65535

Old pre-release builds stored 'X.Y.Z-pre.N' as-is in the MSI ProductVersion.
Windows Installer extracts the numeric prefix of each dot-separated token, so
the stored version was effectively X.Y.Z.N.  Using the same formula here means:
  pre.M > pre.N when M>N  (e.g. 0.2.4.10 > 0.2.4.5)
  stable > any pre-release  (65535 is never a valid pre-release number)
AllowSameVersionUpgrades="yes" in Installer.wxs covers reinstalling the same
pre-release version (X.Y.Z.N == X.Y.Z.N).
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


# ---------------------------------------------------------------------------
# Shared helper - mirrors the PowerShell block in build_installer.bat
# ---------------------------------------------------------------------------


def _installer_version(cargo_version: str) -> str:
    """Return the 4-part installer version for the given Cargo version string.

    The same value is used for both MSI ProductVersion and Bundle Version.
    """
    pre = re.fullmatch(r"(\d+\.\d+\.\d+)-pre\.(\d+)", cargo_version)
    if pre:
        base, n = pre[1], int(pre[2])
        if n >= 65535:
            raise ValueError(f"Pre-release number must be < 65535: {cargo_version}")
        return f"{base}.{n}"
    stable = re.fullmatch(r"\d+\.\d+\.\d+", cargo_version)
    if stable:
        return f"{cargo_version}.65535"
    raise ValueError(f"Unexpected version format: {cargo_version}")


def _as_ints(version: str) -> tuple[int, ...]:
    return tuple(int(x) for x in version.split("."))


# ---------------------------------------------------------------------------
# Unit tests
# ---------------------------------------------------------------------------


def test_pre_release_version() -> None:
    assert _installer_version("0.2.4-pre.8") == "0.2.4.8"
    assert _installer_version("0.2.4-pre.10") == "0.2.4.10"


def test_stable_version() -> None:
    assert _installer_version("0.2.4") == "0.2.4.65535"


def test_upgrade_ordering_between_pre_releases() -> None:
    b7 = _installer_version("0.2.4-pre.7")
    b8 = _installer_version("0.2.4-pre.8")
    b9 = _installer_version("0.2.4-pre.9")
    assert _as_ints(b7) < _as_ints(b8) < _as_ints(b9)


def test_upgrade_ordering_pre_to_stable() -> None:
    assert _as_ints(_installer_version("0.2.4-pre.9")) < _as_ints(_installer_version("0.2.4"))


def test_cross_minor_ordering() -> None:
    assert _as_ints(_installer_version("0.2.4")) < _as_ints(_installer_version("0.2.5-pre.1"))


def test_patch_zero_pre_release_works() -> None:
    assert _installer_version("0.3.0-pre.1") == "0.3.0.1"


def test_migration_from_broken_old_install() -> None:
    """pre.10 must beat old installs where WiX stored '0.2.4-pre.5'.

    Windows Installer parses '0.2.4-pre.5' as (0,2,4,5) so its effective
    version is 0.2.4.5.  The new installer version 0.2.4.10 > 0.2.4.5.
    """
    old_effective = (0, 2, 4, 5)
    assert _as_ints(_installer_version("0.2.4-pre.10")) > old_effective


def test_installer_wxs_allows_same_version_upgrades() -> None:
    wxs = (ROOT / "platforms/windows/Installer.wxs").read_text(encoding="utf-8")
    assert 'AllowSameVersionUpgrades="yes"' in wxs


# ---------------------------------------------------------------------------
# Integration test: run the actual PowerShell snippet (Windows only)
# ---------------------------------------------------------------------------

_PS_SNIPPET = """\
$v = '{cargo_ver}'
if($v -match '^(\\d+\\.\\d+\\.\\d+)-pre\\.(\\d+)$'){{
  $n=[int]$Matches[2]; if($n -ge 65535){{exit 1}}
  Write-Output ($Matches[1]+'.'+$n+'|'+$Matches[1]+'.'+$n)
}} elseif($v -match '^\\d+\\.\\d+\\.\\d+$'){{
  Write-Output ($v+'.65535|'+$v+'.65535')
}} else{{exit 1}}
"""

_PS_CASES = [
    ("0.2.4-pre.8", ("0.2.4.8", "0.2.4.8")),
    ("0.2.4-pre.10", ("0.2.4.10", "0.2.4.10")),
    ("0.2.4", ("0.2.4.65535", "0.2.4.65535")),
    ("1.2.3-pre.15", ("1.2.3.15", "1.2.3.15")),
    ("0.3.0-pre.1", ("0.3.0.1", "0.3.0.1")),
]


@pytest.mark.skipif(
    __import__("shutil").which("powershell") is None,
    reason="powershell not available on this platform",
)
@pytest.mark.parametrize("cargo_ver,expected", _PS_CASES)
def test_powershell_snippet_matches_formula(
    tmp_path: Path, cargo_ver: str, expected: tuple[str, str]
) -> None:
    msi_exp, bundle_exp = expected
    assert _installer_version(cargo_ver) == msi_exp

    script = tmp_path / "ver.ps1"
    script.write_text(_PS_SNIPPET.format(cargo_ver=cargo_ver), encoding="utf-8")
    result = subprocess.run(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(script)],
        capture_output=True, text=True,
    )
    assert result.returncode == 0, f"PS failed: {result.stderr}"
    msi, bundle = result.stdout.strip().split("|")
    assert msi == msi_exp
    assert bundle == bundle_exp
