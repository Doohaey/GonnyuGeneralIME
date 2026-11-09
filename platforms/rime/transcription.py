"""Build-time IPA-to-second-scheme projection for Rime resources.

The dictionary remains the source of truth.  This module derives the
scheme2 spelling map from the stored IPA every time a region is rebuilt; no
static per-region map is checked in.
"""

from __future__ import annotations

import csv
import re
import unicodedata
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE = ROOT / "resources" / "transcription" / "ipa_scheme.tsv"


@lru_cache(maxsize=4)
def _table_signature(path: Path):
    stat = path.stat()
    return str(path), stat.st_mtime_ns, stat.st_size


@lru_cache(maxsize=4)
def _load_table(signature):
    groups = {"initial": {}, "vowel": {}, "coda": {}, "syllabic": {}}
    with Path(signature[0]).open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            kind = (row.get("kind") or "").strip()
            ipa = (row.get("ipa") or "").strip()
            spelling = (row.get("spelling") or "").strip()
            if kind in groups and ipa and spelling:
                groups[kind][ipa] = spelling
    return groups


def table():
    return _load_table(_table_signature(TABLE))


def normalize_ipa(value: str) -> str:
    return (unicodedata.normalize("NFC", value)
            .replace("Ǿ", "").replace("ʨ", "tɕ").replace("α", "ɑ")
            .replace("I", "i").replace("ʊ", "u").replace("ᴀ", "a")
            .replace("ᴇ", "e").replace("ː", "")
            .replace("ts'", "tsʰ").replace("tɕ'", "tɕʰ")
            .replace("p'", "pʰ").replace("t'", "tʰ").replace("k'", "kʰ"))


def _syllabic_spelling(body: str) -> str | None:
    normalized = unicodedata.normalize("NFD", body)
    if normalized.startswith("ɭ") and normalized[1:] in {"\u0329", ""}:
        return "l"
    match = re.fullmatch(r"([mnŋ])(?:\u0329|\u030d)?", normalized)
    if not match:
        return None
    return {"m": "m", "n": "n", "ŋ": "ng"}[match.group(1)]


def ipa_syllable_to_scheme2(syllable: str, *, ee_e_opposition: bool = False,
                            oo_o_opposition: bool = False) -> str:
    groups = table()
    body = normalize_ipa(syllable).lstrip("~").strip()
    if not body:
        return ""
    syllabic = _syllabic_spelling(body)
    if syllabic is not None:
        return syllabic
    if body.startswith("ʔ"):
        body = body[1:]
        if not body:
            return ""
        syllabic = _syllabic_spelling(body)
        if syllabic is not None:
            return syllabic

    initials = groups["initial"]
    initial = ""
    initial_spelling = ""
    for candidate in sorted(initials, key=len, reverse=True):
        if body.startswith(candidate):
            initial = candidate
            initial_spelling = initials[candidate]
            body = body[len(candidate):]
            break

    final = body
    syllabic = _syllabic_spelling(final)
    if syllabic is not None:
        return initial_spelling + syllabic

    decomposed = unicodedata.normalize("NFD", final)
    nasalized = "\u0303" in decomposed
    final = unicodedata.normalize("NFC", decomposed.replace("\u0303", ""))

    codas = groups["coda"]
    coda = ""
    if final and final[-1] in codas:
        coda = codas[final[-1]]
        final = final[:-1]

    if initial_spelling.endswith("j") and len(final) > 1 and final.startswith("i"):
        final = final[1:]

    vowels = groups["vowel"]
    nucleus = []
    for char in final:
        spelling = vowels.get(char)
        if spelling is None:
            return ""
        if char == "ɛ" and not ee_e_opposition:
            spelling = "e"
        elif char == "ɔ" and not oo_o_opposition:
            spelling = "o"
        nucleus.append(spelling)

    nasal = "nn" if nasalized and coda not in {"n", "ng", "m"} else ""
    return initial_spelling + "".join(nucleus) + nasal + coda


def strip_tone(token: str) -> str:
    return re.sub(r"[0-9]\*?$", "", token.strip())


def scheme2_config(config: dict) -> tuple[bool, bool, bool]:
    section = config.get("transcription", {}) or {}
    schemes = section.get("schemes", ["gon-pin"])
    if isinstance(schemes, str):
        schemes = [schemes]
    ipa = section.get("ipa", {}) or {}
    return ("ipa" in schemes,
            bool(ipa.get("ee_e_opposition", False)),
            bool(ipa.get("oo_o_opposition", False)))


def build_gon_map(entries, *, ee_e_opposition: bool = False,
                  oo_o_opposition: bool = False) -> dict[str, tuple[str, ...]]:
    """Return gan syllable -> possible scheme2 spellings, tone-stripped."""
    result: dict[str, set[str]] = {}
    for entry in entries:
        ipa_tokens = [part for part in re.split(r"\s+", entry.ipa.strip()) if part]
        gan_tokens = [part for part in re.split(r"\s+", entry.gan.strip()) if part]
        if len(ipa_tokens) != len(gan_tokens):
            continue
        for ipa_token, gan_token in zip(ipa_tokens, gan_tokens):
            scheme2 = ipa_syllable_to_scheme2(
                strip_tone(ipa_token),
                ee_e_opposition=ee_e_opposition,
                oo_o_opposition=oo_o_opposition,
            )
            gan = strip_tone(gan_token)
            if scheme2 and gan:
                result.setdefault(gan, set()).add(scheme2)
    return {gan: tuple(sorted(values)) for gan, values in result.items()}


def _derive_tone_base(name: str) -> int:
    name = name.replace("陰", "阴").replace("陽", "阳").strip()
    name = re.sub(r"[甲乙丙丁]$", "", name)
    if "平" in name:
        category = "ping"
    elif "上" in name:
        category = "shang"
    elif "去" in name:
        category = "qu"
    elif "入" in name:
        category = "ru"
    else:
        return 9
    if "阴" in name:
        polarity = "yin"
    elif "阳" in name:
        polarity = "yang"
    else:
        polarity = "neutral"
    if polarity == "yin":
        return {"ping": 1, "shang": 3, "qu": 5, "ru": 7}[category]
    if polarity == "yang":
        return {"ping": 2, "shang": 4, "qu": 6, "ru": 8}[category]
    return {"ping": 1, "shang": 3, "qu": 5, "ru": 7}[category]


def tone_to_scheme2(marker: str, tone_classes: dict, overrides: dict | None = None) -> str:
    marker = str(marker or "").strip()
    match = re.fullmatch(r"([0-9])(\*?)", marker)
    if not match:
        return marker
    number, star = match.groups()
    if number == "0":
        return "0" + star
    overrides = overrides or {}
    override = overrides.get(marker)
    if override is None:
        override = overrides.get(number)
        if override is not None and star:
            override = str(override) + star
    if override is not None:
        return str(override)
    entry = tone_classes.get(marker) or tone_classes.get(number) or {}
    name = str(entry.get("name", "")) if isinstance(entry, dict) else ""
    if not name:
        return marker
    return f"{_derive_tone_base(name)}{star}"


def build_tone_map(config: dict) -> dict[str, str]:
    section = config.get("transcription", {}) or {}
    overrides = (section.get("ipa", {}) or {}).get("tone_map", {}) or {}
    classes = config.get("tone_classes", {}) or {}
    result = {"0": "0"}
    for marker in classes:
        marker = str(marker)
        result[marker] = tone_to_scheme2(marker, classes, overrides)
    return result


def scheme2_algebra(gon_map: dict[str, tuple[str, ...]], *,
                    mandarin: bool = True, wenbai: bool = True) -> list[str]:
    rules: list[str] = ["    - xform/^W/G/" if wenbai else "    - erase/^W.*$/"]
    for gan in sorted(gon_map):
        for scheme2 in gon_map[gan]:
            rules.append(f"    - derive/^G{re.escape(gan)}$/F{re.escape(scheme2)}/")
    rules.append("    - erase/^G.*$/")
    if not mandarin:
        rules.append("    - erase/^[^F].*$/")
    rules.extend((
        "    - xform/^F//",
        "    - abbrev/^([a-z]).+$/$1/",
    ))
    return rules
