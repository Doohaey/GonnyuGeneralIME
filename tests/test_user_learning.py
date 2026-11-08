from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_fcitx5_skips_user_word_when_readings_are_incomplete() -> None:
    source = (ROOT / "platforms/linux/fcitx5/gannyu_fcitx5.cpp").read_text(encoding="utf-8")

    assert "bool accumulatedReadingsIncomplete = false;" in source
    assert "state->accumulatedReadingsIncomplete = true;" in source
    assert "!state->accumulatedReadingsIncomplete" in source
    assert "bool accumulatedMandarinIncomplete = false;" in source
    assert "state->accumulatedMandarinIncomplete" in source


def test_ffi_skips_user_word_when_readings_are_incomplete() -> None:
    source = (ROOT / "crates/ffi/src/lib.rs").read_text(encoding="utf-8")

    assert "accumulated_readings_incomplete: bool" in source
    assert "session.accumulated_readings_incomplete = true;" in source
    assert "!session.accumulated_readings_incomplete" in source
    assert "accumulated_mandarin_incomplete: bool" in source
    assert "session.accumulated_mandarin_incomplete = true;" in source
