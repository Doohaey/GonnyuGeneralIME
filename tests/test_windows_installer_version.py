"""Tests for the Windows installer version mapping in build_installer.bat.

build_installer.bat converts a Cargo semver string to two 4-part Windows
installer versions (MSI and Bundle):
  Stable  X.Y.Z       -> MSI=X.Y.Z,    Bundle=X.Y.Z.0
  Pre     X.Y.Z-pre.N -> MSI=X.Y.Z-1,  Bundle=X.Y.Z-1.N   (patch Z > 0)

The MSI version uses only the first three components (Windows Installer
ignores the fourth), so all pre-releases of the same patch share an MSI
version one minor lower than the stable.  Installer.wxs carries
AllowSameVersionUpgrades="yes" so that direct MSI reinstalls between
pre-releases also work.  The Bundle (Burn) version uses all four components
and therefore distinguishes every pre-release number correctly.
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
    pre = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)-pre\.(\d+)", cargo_version)
    if pre:
        major, minor, patch, n = (int(pre[i]) for i in (1, 2, 3, 4))
        if patch == 0:
            raise ValueError(
                f"Pre-release on patch 0 not supported: {cargo_version}. "
                "Use X.Y.1-pre.N instead."
            )
        base = f"{major}.{minor}.{patch - 1}"
        return base, f"{base}.{n}"
    stable = re.fullmatch(r"\d+\.\d+\.\d+", cargo_version)
    if stable:
        return cargo_version, f"{cargo_version}.0"
    raise ValueError(f"Unexpected version format: {cargo_version}")


def _as_ints(version: str) -> tuple[int, ...]:
    return tuple(int(x) for x in version.split("."))


# ---------------------------------------------------------------------------
# Unit tests for the version mapping formula
# ---------------------------------------------------------------------------


def test_pre_release_msi_version() -> None:
    msi, _ = _installer_versions("0.2.4-pre.8")
    assert msi == "0.2.3"


def test_pre_release_bundle_version() -> None:
    _, bundle = _installer_versions("0.2.4-pre.8")
    assert bundle == "0.2.3.8"


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
    assert bundle == "0.2.4.0"


def test_cross_minor_ordering() -> None:
    """pre-releases of the next minor version should be above the current stable."""
    _, b_stable = _installer_versions("0.2.4")
    _, b_pre = _installer_versions("0.2.5-pre.1")
    assert _as_ints(b_stable) < _as_ints(b_pre)


def test_patch_zero_pre_release_raises() -> None:
    with pytest.raises(ValueError, match="patch 0"):
        _installer_versions("0.3.0-pre.1")


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
if ($v -match '^(\\d+)\\.(\\d+)\\.(\\d+)-pre\\.(\\d+)$') {{
  $z = [int]$Matches[3]
  if ($z -eq 0) {{ [Console]::Error.WriteLine('Pre-release on patch 0: ' + $v + '. Use X.Y.1-pre.N.'); exit 1 }}
  $b = '{{0}}.{{1}}.{{2}}' -f $Matches[1], $Matches[2], ($z - 1)
  ('{{0}}|{{1}}.{{2}}' -f $b, $b, $Matches[4])
}} elseif ($v -match '^(\\d+\\.\\d+\\.\\d+)$') {{
  ($v + '|' + $v + '.0')
}} else {{
  [Console]::Error.WriteLine('Unexpected version: ' + $v); exit 1
}}
"""

_PS_CASES: list[tuple[str, tuple[str, str]]] = [
    ("0.2.4-pre.8", ("0.2.3", "0.2.3.8")),
    ("0.2.4-pre.9", ("0.2.3", "0.2.3.9")),
    ("0.2.4", ("0.2.4", "0.2.4.0")),
    ("1.2.3-pre.15", ("1.2.2", "1.2.2.15")),
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
