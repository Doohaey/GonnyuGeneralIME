"""Tests for the Windows installer version mapping in build_installer.bat.

build_installer.bat converts a Cargo semver string to two Windows installer
versions (MSI and Bundle):

  Stable  X.Y.Z       -> MSI=X.Y.Z,   Bundle=X.Y.Z.65535
  Pre     X.Y.Z-pre.N -> MSI=X.Y.Z,   Bundle=X.Y.Z.N   (N < 65535)

All pre-releases of the same base version share the same MSI version.
Installer.wxs carries AllowSameVersionUpgrades="yes" so direct MSI
installs within the same version series work too.

Bundle ordering:  pre.N < pre.M (N<M) < stable (65535)

The 65535 sentinel for stable is not user-visible but ensures a stable
release always beats every possible pre-release in upgrade comparisons.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


# ---------------------------------------------------------------------------
# Shared helper – mirrors the PowerShell block in build_installer.bat
# ---------------------------------------------------------------------------


def _installer_versions(cargo_version: str) -> tuple[str, str]:
    """Return (msi_version, bundle_version) for the given Cargo version string."""
    pre = re.fullmatch(r"(\d+\.\d+\.\d+)-pre\.(\d+)", cargo_version)
    if pre:
        base, n = pre[1], int(pre[2])
        if n >= 65535:
            raise ValueError(
                f"Pre-release number must be < 65535: {cargo_version}"
            )
        return base, f"{base}.{n}"
    stable = re.fullmatch(r"\d+\.\d+\.\d+", cargo_version)
    if stable:
        return cargo_version, f"{cargo_version}.65535"
    raise ValueError(f"Unexpected version format: {cargo_version}")


def _as_ints(version: str) -> tuple[int, ...]:
    return tuple(int(x) for x in version.split("."))


# ---------------------------------------------------------------------------
# Unit tests for the version mapping formula
# ---------------------------------------------------------------------------


def test_pre_release_msi_version() -> None:
    msi, _ = _installer_versions("0.2.4-pre.8")
    assert msi == "0.2.4"


def test_pre_release_bundle_version() -> None:
    _, bundle = _installer_versions("0.2.4-pre.8")
    assert bundle == "0.2.4.8"


def test_upgrade_ordering_between_pre_releases() -> None:
    _, b7 = _installer_versions("0.2.4-pre.7")
    _, b8 = _installer_versions("0.2.4-pre.8")
    _, b9 = _installer_versions("0.2.4-pre.9")
    assert _as_ints(b7) < _as_ints(b8) < _as_ints(b9)


def test_upgrade_ordering_pre_to_stable() -> None:
    _, bundle_pre = _installer_versions("0.2.4-pre.9")
    _, bundle_stable = _installer_versions("0.2.4")
    assert _as_ints(bundle_pre) < _as_ints(bundle_stable)


def test_stable_msi_and_bundle_versions() -> None:
    msi, bundle = _installer_versions("0.2.4")
    assert msi == "0.2.4"
    assert bundle == "0.2.4.65535"


def test_cross_minor_ordering() -> None:
    """Stable X.Y should be below pre-releases of X.Y+1."""
    _, b_stable = _installer_versions("0.2.4")
    _, b_pre = _installer_versions("0.2.5-pre.1")
    assert _as_ints(b_stable) < _as_ints(b_pre)


def test_patch_zero_pre_release_works() -> None:
    """pre-releases on patch 0 are valid (0.3.0-pre.N)."""
    msi, bundle = _installer_versions("0.3.0-pre.1")
    assert msi == "0.3.0"
    assert bundle == "0.3.0.1"


def test_migration_from_broken_old_install() -> None:
    """First fixed release (pre.10) must appear as an upgrade over old broken installs.

    Old installers produced Bundle ≈ X.Y.Z.0 (WiX stripped the -pre.N suffix).
    The new formula gives Bundle X.Y.Z.N (N > 0), so it is a proper upgrade.
    """
    _, bundle_old = "0.2.4", "0.2.4.0"  # What old WiX produced
    _, bundle_new = _installer_versions("0.2.4-pre.10")
    assert _as_ints(bundle_new) > _as_ints(bundle_old)


# ---------------------------------------------------------------------------
# Structural test: Installer.wxs must carry AllowSameVersionUpgrades
# ---------------------------------------------------------------------------


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
  Write-Output ($Matches[1]+'|'+$Matches[1]+'.'+$n)
}} elseif($v -match '^\\d+\\.\\d+\\.\\d+$'){{
  Write-Output ($v+'|'+$v+'.65535')
}} else{{exit 1}}
"""

_PS_CASES: list[tuple[str, tuple[str, str]]] = [
    ("0.2.4-pre.8", ("0.2.4", "0.2.4.8")),
    ("0.2.4-pre.10", ("0.2.4", "0.2.4.10")),
    ("0.2.4", ("0.2.4", "0.2.4.65535")),
    ("1.2.3-pre.15", ("1.2.3", "1.2.3.15")),
    ("0.3.0-pre.1", ("0.3.0", "0.3.0.1")),
]


@pytest.mark.skipif(
    __import__("shutil").which("powershell") is None,
    reason="powershell not available on this platform",
)
@pytest.mark.parametrize("cargo_ver,expected", _PS_CASES)
def test_powershell_snippet_matches_formula(
    tmp_path: Path, cargo_ver: str, expected: tuple[str, str]
) -> None:
    assert _installer_versions(cargo_ver) == expected, "Python formula mismatch"

    script = tmp_path / "ver.ps1"
    script.write_text(_PS_SNIPPET.format(cargo_ver=cargo_ver), encoding="utf-8")
    result = subprocess.run(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(script)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"PS script failed: {result.stderr}"
    msi, bundle = result.stdout.strip().split("|")
    assert (msi, bundle) == expected

