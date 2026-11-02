import os
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize(
    ("statuses", "expected_status", "expected_attempts"),
    [([0], 0, 1), ([68, 68, 0], 0, 3), ([65], 65, 1), ([68, 68, 68], 68, 3)],
)
def test_stapling_retries_only_service_failures(
    tmp_path: Path, statuses: list[int], expected_status: int, expected_attempts: int
) -> None:
    state = tmp_path / "statuses"
    state.write_text("\n".join(map(str, statuses)) + "\n")
    calls = tmp_path / "calls"
    xcrun = tmp_path / "xcrun"
    xcrun.write_text(
        '#!/bin/bash\n'
        'printf "%s\\n" "$*" >> "$STAPLE_CALLS"\n'
        'status=$(head -n 1 "$STAPLE_STATUSES")\n'
        'sed \'1d\' "$STAPLE_STATUSES" > "$STAPLE_STATUSES.next"\n'
        'mv "$STAPLE_STATUSES.next" "$STAPLE_STATUSES"\n'
        'exit "$status"\n'
    )
    xcrun.chmod(0o755)
    sleep = tmp_path / "sleep"
    sleep.write_text('#!/bin/bash\nexit 0\n')
    sleep.chmod(0o755)
    package = tmp_path / "Gonnyu Input.pkg"
    result = subprocess.run(
        ["bash", str(ROOT / "platforms/macos/staple_package.sh"), str(package)],
        env={
            **os.environ,
            "PATH": f"{tmp_path}{os.pathsep}{os.environ['PATH']}",
            "STAPLE_STATUSES": str(state),
            "STAPLE_CALLS": str(calls),
        },
        capture_output=True,
        text=True,
    )
    assert result.returncode == expected_status
    assert calls.read_text().splitlines() == [f"stapler staple {package}"] * expected_attempts
