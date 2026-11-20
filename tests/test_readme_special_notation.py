from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPECIAL = "êïô"


def _final_table_keys():
    for name in ("README.md", "README.en.md"):
        in_table = False
        for line in (ROOT / name).read_text(encoding="utf-8").splitlines():
            if line.startswith("| gon-pin |"):
                in_table = True
                continue
            if not in_table:
                continue
            if not line.startswith("|"):
                in_table = False
                continue
            key = line.strip("|").split("|")[0].strip()
            if key and set(key) != {"-"}:
                yield name, key


def test_no_separate_special_notation_rows():
    offenders = [(name, key) for name, key in _final_table_keys() if any(ch in key for ch in SPECIAL)]
    assert not offenders
