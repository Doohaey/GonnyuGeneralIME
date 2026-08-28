from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = (
    "android.yml",
    "fcitx5.yml",
    "ibus.yml",
    "rime-ios.yml",
    "rime-macos.yml",
    "windows.yml",
)


def test_platform_workflows_use_the_workspace_version() -> None:
    for name in WORKFLOWS:
        content = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
        assert "awk -F" in content
        assert "scripts/" not in content
        assert "0.2.1" not in content


def test_release_workflow_uses_tagged_workspace_version() -> None:
    content = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    assert "workflow_dispatch:" not in content
    assert "awk -F" in content
    assert "scripts/" not in content
    assert "actions/setup-python@v5" in content
    assert "platforms/rime/build.py --list-regions" in content
    assert '"$((4 + 2 * ${#regions[@]}))"' in content
    assert "GonnyuGeneralIME-${{ steps.product.outputs.version }}-*" in content
    assert 'gh release create "$GITHUB_REF_NAME" release-assets/*' in content

def test_linux_workflows_run_isolated_installer_smoke_tests() -> None:
    for name in ("fcitx5.yml", "ibus.yml"):
        content = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
        assert 'DESTDIR="$smoke_root/root" bash "$smoke_root/install.sh"' in content
        assert 'tar -xzf "$artifact" -C "$smoke_root"' in content

def test_android_workflow_runs_installation_smoke_test() -> None:
    content = (ROOT / ".github/workflows" / "android.yml").read_text(encoding="utf-8")
    assert "reactivecircus/android-emulator-runner@v2" in content
    assert "adb install build/android/GonnyuGeneralIME-" in content
    assert "--name-match=kvm" in content
    assert "sudo rm -rf /usr/share/dotnet /opt/ghc /usr/local/share/boost" in content
    assert "adb uninstall io.gannyu.input" in content
    assert "ime set" not in content


def test_android_build_supports_a_local_keystore_without_base64() -> None:
    root = ROOT / "platforms" / "android"
    build = (root / "build.sh").read_text(encoding="utf-8")
    gradle = (root / "app" / "build.gradle.kts").read_text(encoding="utf-8")

    assert "ANDROID_KEYSTORE_PATH" in build
