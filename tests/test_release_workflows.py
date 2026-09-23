from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = (
    "android.yml",
    "fcitx5.yml",
    "rime.yml",
    "windows.yml",
    "macos.yml",
)


def test_platform_workflows_use_the_workspace_version() -> None:
    for name in WORKFLOWS:
        content = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
        assert "awk -F" in content
        assert "scripts/" not in content
        assert "0.2.1" not in content


def test_quality_workflow_rejects_a_stale_cargo_lockfile() -> None:
    content = (ROOT / ".github/workflows/quality.yml").read_text(encoding="utf-8")
    assert "cargo clippy --workspace --all-targets --locked -- -D warnings" in content
    assert "cargo test --workspace --locked" in content


def test_release_workflow_uses_tagged_workspace_version() -> None:
    content = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    assert "workflow_dispatch:" not in content
    assert "awk -F" in content
    assert "scripts/" not in content
    assert "actions/setup-python@v5" in content
    assert "platforms/rime/build.py --list-regions" in content
    assert '"$((4 + ${#regions[@]}))"' in content
    assert "macos.pkg" in content
    assert "uses: ./.github/workflows/macos.yml" in content
    assert "GonnyuGeneralIME-${{ steps.product.outputs.version }}-*" in content
    assert "rime-${region}.zip" in content
    assert 'gh release view "$GITHUB_REF_NAME" > /dev/null 2>&1' in content
    assert 'gh release upload "$GITHUB_REF_NAME" release-assets/* --clobber' in content
    assert 'gh release edit "$GITHUB_REF_NAME"' in content
    assert 'gh release create "$GITHUB_REF_NAME" release-assets/*' in content
    assert "--prerelease" not in content

def test_fcitx5_workflow_runs_an_isolated_installer_smoke_test() -> None:
    content = (ROOT / ".github/workflows/fcitx5.yml").read_text(encoding="utf-8")
    assert 'DESTDIR="$smoke_root/root" bash "$smoke_root/install.sh"' in content
    assert 'tar -xzf "$artifact" -C "$smoke_root"' in content


def test_fcitx5_buffer_supports_middle_editing() -> None:
    source = (ROOT / "platforms/linux/fcitx5/gannyu_fcitx5.cpp").read_text(encoding="utf-8")

    assert "size_t cursor = 0" in source
    assert "state->buffer.insert(state->cursor" in source
    assert "state->buffer.erase(state->cursor - 1, 1)" in source
    assert "state->buffer.erase(state->cursor, 1)" in source
    assert "sym == FcitxKey_Left" in source
    assert "sym == FcitxKey_Right" in source
    assert "sym == FcitxKey_Delete" in source
    assert "FcitxKey_minus" in source
    assert "FcitxKey_equal" in source


def test_macos_workflow_rebuilds_and_inspects_pkg() -> None:
    content = (ROOT / ".github/workflows/macos.yml").read_text(encoding="utf-8")
    assert "runs-on: macos-15-intel" in content
    assert "bash platforms/macos/build.sh" in content
    assert "GannyuMacOSSmoke --region lancong --input gau" in content
    assert "bash platforms/macos/package.sh" in content
    assert "pkgutil --expand" in content
    assert "GANNYU_MACOS_UNSIGNED_TEST" in content
    assert "MACOS_DEVELOPER_ID_APPLICATION_P12_BASE64" in content
    assert "MACOS_DEVELOPER_ID_INSTALLER_P12_BASE64" in content
    assert "APPLE_NOTARY_KEY_P8_BASE64" in content
    assert "GANNYU_MACOS_SIGNING_KEYCHAIN" in content
    assert "timeout-minutes: 45" in content
    assert 'CMAKE_BUILD_PARALLEL_LEVEL: "4"' in content
    assert "timeout-minutes: 15" in content
    assert "release_signing:" in content
    assert "startsWith(github.ref, 'refs/tags/v') || inputs.release_signing" in content
    assert "matrix:" in content
    assert "macos-15" in content and "macos-26" in content


def test_macos_installer_signing_is_non_interactive_and_observable() -> None:
    workflow = (ROOT / ".github/workflows/macos.yml").read_text(encoding="utf-8")
    package = (ROOT / "platforms/macos/package.sh").read_text(encoding="utf-8")

    assert 'security import "$signing_dir/installer.p12" -k "$keychain" -f pkcs12' in workflow
    assert "-T /usr/bin/security" in workflow
    assert workflow.index('security import "$signing_dir/installer.p12"') < workflow.index(
        "security set-key-partition-list"
    )
    assert "Preflight installer private-key signing" in workflow
    assert "Preflight installer trusted timestamp" in workflow
    assert "--timestamp=none" in workflow
    assert "timeout-minutes: 2" in workflow
    assert "timeout-minutes: 3" in workflow
    assert 'pkgutil --check-signature "$preflight/local-signed.pkg"' in workflow
    assert 'pkgutil --check-signature "$preflight/timestamped.pkg"' in workflow
    assert 'productsign_args=(--sign "$installer_identity" --timestamp)' in package
    assert 'productsign_args+=(--keychain "$signing_keychain")' in package
    assert '"$unsigned_package" "$final_package"' in package
    assert "installer package signing completed" in package


def test_release_notes_do_not_link_to_generated_metadata() -> None:
    content = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    assert "SHA256SUMS" not in content
    assert "SBOM" not in content
    assert "generate_release_metadata.py" not in content

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
    assert "local.properties" in build


def test_platform_builds_accept_prerelease_versions() -> None:
    android = (ROOT / "platforms/android/app/build.gradle.kts").read_text(encoding="utf-8")
    assert 'productVersion.substringBefore("-")' in android

    cmake = (ROOT / "platforms/linux/fcitx5/CMakeLists.txt").read_text(encoding="utf-8")
    assert "(-[0-9A-Za-z.-]+)?" in cmake


def test_local_android_sdk_file_stays_ignored_and_ci_safe() -> None:
    share_ignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    repository_ignore = Path(__file__).resolve().parents[2].joinpath(".gitignore")
    root_ignore = repository_ignore.read_text(encoding="utf-8") if repository_ignore.is_file() else share_ignore
    build_script = (ROOT / "platforms/android/build.sh").read_text(encoding="utf-8")

    assert "platforms/android/local.properties" in root_ignore
    assert "platforms/android/local.properties" in share_ignore
    assert 'write_local_properties "$SDK_ROOT"' in build_script
