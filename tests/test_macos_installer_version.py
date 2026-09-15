"""Keep macOS PKG ordering and overwrite rules aligned with Windows MSI."""

import importlib.util
import plistlib
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "platforms/macos/check_installer_version.sh"
MAPPER = ROOT / "platforms/macos/installer_version.py"


def package_version(value: str) -> str:
    spec = importlib.util.spec_from_file_location("macos_installer_version", MAPPER)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.package_version(value)


def check_version(tmp_path: Path, incoming: str, installed: str | None) -> subprocess.CompletedProcess[str]:
    plist = tmp_path / "Info.plist"
    if installed is not None:
        plist.write_bytes(plistlib.dumps({"CFBundleShortVersionString": installed}))
    return subprocess.run(["bash", str(SCRIPT), incoming, str(plist)], capture_output=True, text=True)


def test_package_version_preserves_windows_upgrade_order() -> None:
    assert package_version("0.2.4-pre.19") == "0.2.4023"
    assert package_version("0.2.4") == "0.2.5004"
    assert package_version("0.2.5-pre.1") == "0.2.5006"
    with pytest.raises(ValueError):
        package_version("0.2.4-pre.1000")


def test_existing_bundle_without_version_fails_closed(tmp_path: Path) -> None:
    bundle = tmp_path / "GonnyuInputMethod.app"
    (bundle / "Contents").mkdir(parents=True)
    result = subprocess.run(
        ["bash", str(SCRIPT), "0.2.4-pre.19", str(bundle / "Contents/Info.plist")],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "no readable Info.plist" in result.stderr


def test_custom_release_version_distinguishes_prereleases(tmp_path: Path) -> None:
    plist = tmp_path / "Info.plist"
    plist.write_bytes(
        plistlib.dumps(
            {"CFBundleShortVersionString": "0.2.4", "GannyuVersion": "0.2.4-pre.18"}
        )
    )
    result = subprocess.run(
        ["bash", str(SCRIPT), "0.2.4-pre.19", str(plist)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize(
    ("incoming", "installed", "allowed"),
    [
        ("0.2.4-pre.19", None, True),
        ("0.2.4-pre.19", "0.2.4-pre.18", True),
        ("0.2.4", "0.2.4-pre.19", True),
        ("0.2.5-pre.1", "0.2.4", True),
        ("0.2.4-pre.18", "0.2.4-pre.19", False),
        ("0.2.4-pre.19", "0.2.4-pre.19", False),
        ("0.2.4-pre.19", "0.2.4", False),
    ],
)
def test_preinstall_rejects_downgrade_and_same_version(
    tmp_path: Path, incoming: str, installed: str | None, allowed: bool
) -> None:
    result = check_version(tmp_path, incoming, installed)
    assert (result.returncode == 0) == allowed, result.stderr
