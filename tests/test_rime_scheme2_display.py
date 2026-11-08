import re
import struct
from pathlib import Path

from platforms.rime.build import active_regions, build


TOKEN = re.compile(r"[A-Za-zêïô]+[0-9]*\*?")
SPECIAL = re.compile(r"[ïêô]")


def _annotations(path: Path) -> list[str]:
    data = path.read_bytes()
    if data[:8] != b"GNYANN01":
        return []
    count, _blob_len, name_len, _checksum = struct.unpack("<IIII", data[8:24])
    index = 24 + name_len
    blob = index + count * 16
    values = []
    for row in range(count):
        offset, key_len, _value_offset, value_len = struct.unpack(
            "<IIII", data[index + row * 16 : index + (row + 1) * 16]
        )
        start = blob + offset + key_len
        values.append(data[start : start + value_len].decode("utf-8"))
    return values


def _lua_table(path: Path, name: str) -> dict[str, str]:
    text = path.read_text(encoding="utf-8")
    body = text.split(f"  {name} = {{", 1)[1].split("  },", 1)[0]
    return dict(re.findall(r'^\s+\["(.*)"\] = "(.*)",$', body, flags=re.MULTILINE))


def _convert(text: str, spellings: dict[str, str], tones: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        token = match.group(0)
        body = re.sub(r"[0-9]\*?$", "", token)
        scheme2 = spellings.get(body)
        if not scheme2:
            return token
        tone = token[len(body):]
        return scheme2 + tones.get(tone, tone)

    return TOKEN.sub(replace, text)


def test_all_second_scheme_regions_convert_to_scheme2_display(tmp_path: Path) -> None:
    for region in active_regions():
        build(region, tmp_path)
        display_path = tmp_path / "lua" / f"gannyu_{region}_scheme2.lua"
        if not display_path.is_file():
            assert region == "fenni"
            continue
        spellings = _lua_table(display_path, "spellings")
        tones = _lua_table(display_path, "tones")
        assert spellings and tones

        residual = []
        lua_dir = tmp_path / "lua"
        for path in sorted(lua_dir.glob(f"gannyu_{region}_annotations.bin")):
            for value in _annotations(path):
                converted = _convert(value, spellings, tones)
                if SPECIAL.search(converted):
                    residual.append((path.name, value, converted))
        for path in sorted(lua_dir.glob(f"gannyu_{region}_data.lua")):
            for value in re.findall(r'^  \[".*"\] = "(.*)"$', path.read_text(encoding="utf-8"), flags=re.MULTILINE):
                if not SPECIAL.search(value):
                    continue
                converted = _convert(value, spellings, tones)
                if SPECIAL.search(converted):
                    residual.append((path.name, value, converted))
        assert not residual, (region, residual[:3])
