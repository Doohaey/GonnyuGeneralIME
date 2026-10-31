#!/usr/bin/env python3
"""Build a portable Rime schema from the canonical regional TSV files."""

from __future__ import annotations

import argparse
import ast
import csv
import hashlib
import itertools
import json
import re
import shutil
import struct
import tomllib
import zlib
from collections import defaultdict
from dataclasses import dataclass, replace
from pathlib import Path

try:
    from . import build_cache
    from .fuzzy import compile_algebra, essential_rules, load_rules
except ImportError:
    import build_cache
    from fuzzy import compile_algebra, essential_rules, load_rules


ROOT = Path(__file__).resolve().parents[2]
PLATFORM_DIR = Path(__file__).resolve().parent
LUA_NAMES = (
    "gannyu_input_options.lua", "gannyu_options_translator.lua",
    "gannyu_annotation_store.lua", "gannyu_data_lifecycle.lua",
    "gannyu_annotation_filter.lua", "gannyu_single_char_filter.lua",
    "gannyu_relation_filter.lua",
)
LUA_SOURCES = {name: (PLATFORM_DIR / name).read_bytes() for name in LUA_NAMES}
SCHEMA_TEMPLATE = (PLATFORM_DIR / "gannyu.schema.yaml").read_text(encoding="utf-8")
HEADERS = (
    "本词",
    "国际音标",
    "方言拼音",
    "汉语拼音",
    "词汇属性",
    "对应官话词",
    "官话拼音",
    "词频",
    "同义词",
    "新旧标记",
)
MAX_FORMS = 64
VERSION_LINE = re.compile(r'(?m)^version: "([^"]+)"$')
# Builder hashes accepted for stamp migration of existing regional resources.
LEGACY_BUILD_HASHES = (
    ("e7df51dc8ac45da642969c98bec9df8a44d9e378c73d402f06d60f009b76a055", True),
    ("573c26d5ac3d1446e28a4a6481e40569be28b2e57285d55aa2ffb5191b3f4455", False),
    ("07cf83af0d45625123dbf4ddffcfc48978f9328ff8f1e1d1dfa9cbe1bd59a560", False),
)
LEGACY_RESOURCE_LOGIC_HASH = "2ed1ac480a043ac10be214841d8009e9aab67d56ff23e2eb971304179a5689e9"
RESOURCE_LOGIC_FUNCTIONS = frozenset((
    "region_name", "split_values", "syllables", "strip_tone",
    "lua_quote", "load_entries", "build_new_old", "build_paired_readings",
    "format_pair_display", "display_pair_priority", "matching_pair",
    "matching_pair_for_lookup", "annotated_reading", "strip_prefix_labels",
    "trailing_tone", "suppress_neutral_displays", "build_metadata",
    "build_preferred_readings", "build_synonym_relations", "entry_codes",
    "write_dictionary", "build_single_character_frequencies", "write_annotation_store",
    "write_lua_data", "selected_entries", "write_option_resources", "build_uncached",
))
RESOURCE_LOGIC_CONSTANTS = frozenset(("HEADERS", "MAX_FORMS", "LUA_NAMES"))

def active_regions() -> tuple[str, ...]:
    """Return active regions with the manifest default first.

    The order is user-visible in Rime's schema list and becomes the first-run
    mobile default, so it must not depend on directory names.
    """
    with (ROOT / "resources" / "manifest.toml").open("rb") as handle:
        manifest = tomllib.load(handle)
    regions = [
        str(region["id"])
        for region in manifest.get("regions", [])
        if region.get("status") == "active"
    ]
    if not regions:
        raise ValueError("no active Rime regions")
    default_region = str(manifest.get("default_region", ""))
    if default_region not in regions:
        raise ValueError("default Rime region must be active and listed in the manifest")
    regions = [default_region, *(region for region in regions if region != default_region)]
    return tuple(regions)


def region_name(region: str) -> str:
    config_path = ROOT / "resources" / "regions" / region / "region.toml"
    with config_path.open("rb") as handle:
        config = tomllib.load(handle)
    return str(config["region"]["name_zh"])


with (ROOT / "Cargo.toml").open("rb") as handle:
    VERSION = tomllib.load(handle)["workspace"]["package"]["version"]


@dataclass(frozen=True)
class Entry:
    word: str
    ipa: str
    gan: str
    mandarin: str
    category: str
    mandarin_word: str
    frequency: int
    synonyms: str
    new_old: str


def split_values(value: str) -> list[str]:
    return list(dict.fromkeys(part.strip() for part in re.split(r"[/;]", value) if part.strip()))


def syllables(value: str) -> list[str]:
    return [part for part in re.split(r"[\s']+", value.strip().lower()) if part]


def strip_tone(value: str) -> str:
    return re.sub(r"\d+\*?$", "", value)


def lua_quote(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def load_entries(region: str) -> tuple[str, list[Entry]]:
    region_dir = ROOT / "resources" / "regions" / region
    with (region_dir / "region.toml").open("rb") as handle:
        config = tomllib.load(handle)
    rows: list[Entry] = []
    for key in ("chars", "gan_chars", "words", "gan_words"):
        path = region_dir / config["language"][key]
        with path.open(encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle, delimiter="\t")
            if tuple(reader.fieldnames or ()) != HEADERS:
                raise ValueError(f"unexpected dictionary header: {path}")
            for row in reader:
                word = row["本词"].strip()
                if not word:
                    continue
                try:
                    frequency = int(row["词频"].strip() or "1")
                except ValueError as error:
                    raise ValueError(f"invalid frequency in {path}: {word}") from error
                rows.append(
                    Entry(
                        word=word,
                        ipa=row["国际音标"].strip(),
                        gan=row["方言拼音"].strip(),
                        mandarin=row["汉语拼音"].strip(),
                        category=row["词汇属性"].strip(),
                        mandarin_word=row["对应官话词"].strip(),
                        frequency=max(1, frequency),
                        synonyms=(row["同义词"] or "").strip(),
                        new_old=(row["新旧标记"] or "").strip(),
                    )
                )
    return config["region"]["name_zh"], rows


def build_new_old(entries: list[Entry]) -> tuple[dict[str, tuple[str, str]], dict[str, tuple[str, str]]]:
    groups: dict[tuple[str, str], list[Entry]] = defaultdict(list)
    for entry in entries:
        if len(entry.word) == 1 and entry.new_old[:1] in {"新", "老", "本", "又"}:
            groups[(entry.word, entry.new_old[1:])].append(entry)
    new_old: dict[str, tuple[str, str]] = {}
    heteronyms: dict[str, tuple[str, str]] = {}
    for (word, _), group in groups.items():
        if len(group) != 2:
            continue
        new = next((item.gan for item in group if item.new_old.startswith("新")), "")
        old = next((item.gan for item in group if item.new_old.startswith("老")), "")
        if new and old:
            new_old[word] = (new, old)
            continue
        base = next((item.gan for item in group if item.new_old.startswith("本")), "")
        variant = next((item.gan for item in group if item.new_old.startswith("又")), "")
        if base and variant:
            heteronyms[word] = (base, variant)
    return new_old, {word: pair for word, pair in heteronyms.items() if word not in new_old}


def build_paired_readings(entries: list[Entry]) -> dict[str, list[tuple[str, str, str]]]:
    groups: dict[str, dict[str, list[Entry]]] = defaultdict(lambda: defaultdict(list))
    order: dict[str, list[str]] = defaultdict(list)
    for entry in entries:
        if len(entry.word) != 1 or entry.new_old[:1] not in {"新", "老", "本", "又"}:
            continue
        suffix = entry.new_old[1:]
        if suffix not in groups[entry.word]:
            order[entry.word].append(suffix)
        groups[entry.word][suffix].append(entry)
    pairs: dict[str, list[tuple[str, str, str]]] = defaultdict(list)
    for word, suffixes in order.items():
        for suffix in suffixes:
            group = groups[word].get(suffix, [])
            if len(group) != 2:
                continue
            new = next((item.gan for item in group if item.new_old.startswith("新")), "")
            old = next((item.gan for item in group if item.new_old.startswith("老")), "")
            if new and old:
                pairs[word].append((new, old, "newold"))
                continue
            base = next((item.gan for item in group if item.new_old.startswith("本")), "")
            variant = next((item.gan for item in group if item.new_old.startswith("又")), "")
            if base and variant:
                pairs[word].append((base, variant, "heteronym"))
    registers: dict[str, tuple[str, str] | None] = {}
    for entry in entries:
        if len(entry.word) != 1:
            continue
        current = registers.get(entry.word)
        wen = current[0] if current else ""
        bai = current[1] if current else ""
        if entry.category == "文" and not wen:
            wen = entry.gan
        if entry.category == "白" and not bai:
            bai = entry.gan
        registers[entry.word] = (wen, bai)
    for word, pair in registers.items():
        if pair and pair[0] and pair[1]:
            pairs[word].append((pair[0], pair[1], "wenbai"))
    return dict(pairs)


def format_pair_display(first: str, second: str, kind: str, wrap: bool) -> str:
    display = {
        "heteronym": f"{first}/[又]{second}",
        "newold": f"[新]{first}/[老]{second}",
    }[kind]
    if wrap:
        return f"({display})"
    return display


def display_pair_priority(kind: str) -> int:
    return {"newold": 0, "heteronym": 1, "wenbai": 2}[kind]


def matching_pair(
    char: str,
    stored: str,
    paired_readings: dict[str, list[tuple[str, str, str]]] | None,
    new_old: dict[str, tuple[str, str]],
    heteronyms: dict[str, tuple[str, str]],
) -> tuple[str, str, str] | None:
    pairs: list[tuple[str, str, str]] = []
    for first, second, kind in (paired_readings or {}).get(char, []):
        pairs.append((first, second, kind))
    if not pairs:
        if pair := new_old.get(char):
            pairs.append((pair[0], pair[1], "newold"))
        elif pair := heteronyms.get(char):
            pairs.append((pair[0], pair[1], "heteronym"))
    matches = [
        (first, second, kind)
        for first, second, kind in pairs
        if kind != "wenbai"
        and (
            stored == first
            or stored == second
            or (
                trailing_tone(stored) == "0"
                and strip_tone(stored) in {strip_tone(first), strip_tone(second)}
            )
        )
    ]
    return min(matches, key=lambda item: display_pair_priority(item[2])) if matches else None


def matching_pair_for_lookup(
    char: str,
    stored: str,
    paired_readings: dict[str, list[tuple[str, str, str]]],
) -> tuple[str, str, str] | None:
    matches = [
        (first, second, kind)
        for first, second, kind in paired_readings.get(char, [])
        if stored in {strip_tone(first), strip_tone(second)}
    ]
    return min(matches, key=lambda item: display_pair_priority(item[2])) if matches else None


def annotated_reading(
    entry: Entry,
    new_old: dict[str, tuple[str, str]],
    heteronyms: dict[str, tuple[str, str]],
    paired_readings: dict[str, list[tuple[str, str, str]]] | None = None,
    reading_mode: str = "both",
) -> str:
    if not syllables(entry.gan):
        return ""
    if len(entry.word) == 1:
        match = matching_pair(entry.word, entry.gan, paired_readings, new_old, heteronyms)
        if match is not None:
            first, second, kind = match
            return format_pair_display(first, second, kind, False)
    prefix = {"文": "[文]", "白": "[白]"}.get(entry.category, "")
    if reading_mode != "both" and len(entry.word) == 1 and entry.new_old[:1] in {"新", "老"}:
        prefix = f"[{entry.new_old[0]}]"
    annotated = prefix + entry.gan
    if len(entry.word) <= 1:
        return annotated
    parts = annotated.split()
    stored = syllables(entry.gan)
    if len(parts) != len(entry.word) or len(stored) != len(entry.word):
        return annotated
    for index, char in enumerate(entry.word):
        match = matching_pair(char, stored[index], paired_readings, new_old, heteronyms)
        if match is None:
            continue
        first, second, kind = match
        if trailing_tone(stored[index]) == "0":
            first = strip_tone(first) + "0"
            second = strip_tone(second) + "0"
        parts[index] = format_pair_display(first, second, kind, True)
    return " ".join(parts)


def strip_prefix_labels(display: str) -> str:
    rest = display
    while rest.startswith("["):
        end = rest.find("]")
        if end < 0:
            break
        rest = rest[end + 1 :]
    return rest


def trailing_tone(value: str) -> str:
    match = re.search(r"\d+\*?$", value)
    return match.group(0) if match else ""


def suppress_neutral_displays(displays: dict[str, int]) -> dict[str, int]:
    toned_bases = set()
    for display in displays:
        reading = strip_prefix_labels(display)
        tone = trailing_tone(reading)
        if tone and tone != "0":
            toned_bases.add(strip_tone(reading))
    return {
        display: frequency
        for display, frequency in displays.items()
        if (trailing_tone(strip_prefix_labels(display)) not in ("", "0"))
        or strip_tone(strip_prefix_labels(display)) not in toned_bases
    }


def build_metadata(entries: list[Entry], reading_mode: str = "both", include_synonyms: bool = True) -> tuple[dict[str, str], dict[str, list[str]], dict[str, list[str]]]:
    by_word: dict[str, list[Entry]] = defaultdict(list)
    associates: dict[str, set[str]] = defaultdict(set)
    gan_to_mandarin: dict[str, list[str]] = defaultdict(list)
    mandarin_to_gan: dict[str, list[str]] = defaultdict(list)
    for entry in entries:
        by_word[entry.word].append(entry)
        for synonym in split_values(entry.synonyms) if include_synonyms else ():
            if synonym != entry.word:
                associates[entry.word].add(synonym)
                associates[synonym].add(entry.word)
        for mandarin in split_values(entry.mandarin_word):
            if mandarin != entry.word:
                gan_to_mandarin[entry.word].append(mandarin)
                mandarin_to_gan[mandarin].append(entry.word)

    new_old, heteronyms = build_new_old(entries)
    paired_readings = build_paired_readings(entries)

    def aggregate(word: str, include_mandarin: bool = False) -> str:
        displays: dict[str, int] = {}
        for entry in by_word.get(word, []):
            if entry.category == "官" and not include_mandarin:
                continue
            display = annotated_reading(entry, new_old, heteronyms, paired_readings, reading_mode)
            if display:
                displays[display] = max(displays.get(display, 0), entry.frequency)
        if len(word) == 1:
            displays = suppress_neutral_displays(displays)

        def priority(item: tuple[str, int]) -> tuple[int, int, str]:
            display, frequency = item
            label = 0 if display.startswith("[文]") else 1 if display.startswith("[白]") else 2
            return label, -frequency, display

        ordered = [display for display, _ in sorted(displays.items(), key=priority)]
        joiner = " " if ordered and all(item.startswith("[") for item in ordered) else " / "
        return joiner.join(ordered)

    annotations: dict[str, str] = {}
    all_words = set(by_word) | set(mandarin_to_gan)
    for word in all_words:
        word_entries = by_word.get(word, [])
        is_mandarin = any(entry.category == "官" for entry in word_entries) or word in mandarin_to_gan
        own = aggregate(word)
        if is_mandarin and not own:
            own = aggregate(word, include_mandarin=True)
        parts: list[str] = []
        if is_mandarin:
            parts.append("[不习用]")
            if own:
                parts.append(own)
            reverse = []
            for gan in dict.fromkeys(mandarin_to_gan.get(word, [])):
                reading = aggregate(gan)
                reverse.append(f"{gan}（{reading}）" if reading else gan)
            if reverse:
                parts.append("[习用]" + "/".join(reverse))
        else:
            if own:
                parts.append(own)
            mandarins = list(dict.fromkeys(gan_to_mandarin.get(word, [])))
            if mandarins:
                parts.append("[义]" + "/".join(mandarins))
        linked = sorted(associates.get(word, set()))
        if linked:
            parts.append("[联]" + "/".join(linked))
        if parts:
            annotations[word] = " ".join(parts)

    known = set(by_word)
    before: dict[str, list[str]] = {}
    after: dict[str, list[str]] = {}
    for word in known:
        prior = [gan for gan in dict.fromkeys(mandarin_to_gan.get(word, [])) if gan in known]
        if prior:
            before[word] = prior[:1]
        following = [
            item
            for item in dict.fromkeys(gan_to_mandarin.get(word, []) + sorted(associates.get(word, set())))
            if item in known
        ]
        if following:
            after[word] = following
    return annotations, before, after


def build_preferred_readings(entries: list[Entry]) -> dict[str, str]:
    preferred: dict[str, tuple[int, str]] = {}
    for entry in entries:
        parts = syllables(entry.gan)
        if len(entry.word) != 1 or len(parts) != 1 or entry.category == "官":
            continue
        current = preferred.get(entry.word)
        if current is None or entry.frequency > current[0]:
            preferred[entry.word] = (entry.frequency, parts[0])
    return {word: reading for word, (_, reading) in preferred.items()}


def build_synonym_relations(entries: list[Entry], known_only: bool = False) -> dict[str, list[str]]:
    relations: dict[str, set[str]] = defaultdict(set)
    for entry in entries:
        for synonym in split_values(entry.synonyms):
            if synonym != entry.word:
                relations[entry.word].add(synonym)
                relations[synonym].add(entry.word)
    known = {entry.word for entry in entries}
    return {word: sorted(item for item in words if not known_only or item in known)
            for word, words in relations.items()}


def entry_codes(entry: Entry, paired_readings: dict[str, list[tuple[str, str, str]]]) -> set[str]:
    gan = [strip_tone(item) for item in syllables(entry.gan)]
    mandarin = [strip_tone(item) for item in syllables(entry.mandarin)]
    codes: set[str] = set()
    if gan:
        codes.add(" ".join(f"G{item}" for item in gan))
    if mandarin and mandarin != gan:
        codes.add(" ".join(mandarin))
    if gan and len(gan) == len(mandarin):
        choices = [
            [f"G{g}"] if g == m else [f"G{g}", m]
            for g, m in zip(gan, mandarin)
        ]
        total = 1
        for options in choices:
            total *= len(options)
        if total <= MAX_FORMS:
            codes.update(" ".join(parts) for parts in itertools.product(*choices))
    if len(entry.word) > 1 and len(gan) == len(entry.word):
        choices = []
        for index, char in enumerate(entry.word):
            stored = strip_tone(gan[index])
            match = matching_pair_for_lookup(char, stored, paired_readings)
            if match is None:
                readings = [f"G{stored}"]
            else:
                first, second, kind = match
                readings = list(dict.fromkeys(
                    ("W" if kind == "wenbai" and strip_tone(reading) != stored else "G") + strip_tone(reading)
                    for reading in (first, second)
                ))
            choices.append(readings)
        total = 1
        for options in choices:
            total *= len(options)
        if total <= MAX_FORMS:
            codes.update(" ".join(parts) for parts in itertools.product(*choices))
    return {code for code in codes if code}


def write_dictionary(path: Path, entries: list[Entry], region: str, name: str | None = None) -> int:
    name = name or f"gannyu_{region}"
    paired_readings = build_paired_readings(entries)
    records: dict[tuple[str, str], int] = {}
    for entry in entries:
        weight = min(entry.frequency, 200000)
        for code in entry_codes(entry, paired_readings):
            key = (entry.word, code)
            records[key] = max(records.get(key, 0), weight)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write("# Rime dictionary\n# encoding: utf-8\n\n---\n")
        handle.write(
            f'name: {name}\nversion: "{VERSION}"\nsort: by_weight\n'
            "use_preset_vocabulary: false\n...\n\n"
        )
        for (word, code), frequency in sorted(records.items()):
            handle.write(f"{word}\t{code}\t{frequency}\n")
    return len(records)


def build_single_character_frequencies(entries: list[Entry]) -> dict[str, int]:
    frequencies: dict[str, int] = {}
    for entry in entries:
        if len(entry.word) == 1:
            frequencies[entry.word] = max(frequencies.get(entry.word, 0), entry.frequency)
    return frequencies


def write_annotation_store(path: Path, name: str, annotations: dict[str, str]) -> None:
    index = bytearray()
    blob = bytearray()
    for key, value in sorted(annotations.items(), key=lambda item: item[0].encode("utf-8")):
        key_bytes, value_bytes = key.encode("utf-8"), value.encode("utf-8")
        offset = len(blob)
        index.extend(struct.pack("<IIII", offset, len(key_bytes), offset + len(key_bytes), len(value_bytes)))
        blob.extend(key_bytes)
        blob.extend(value_bytes)
    name_bytes = name.encode("ascii")
    payload = name_bytes + index + blob
    header = b"GNYANN01" + struct.pack("<IIII", len(annotations), len(blob), len(name_bytes), zlib.adler32(payload))
    path.write_bytes(header + payload)


def write_lua_data(
    path: Path,
    annotations: dict[str, str],
    readings: dict[str, str],
    before: dict[str, list[str]],
    after: dict[str, list[str]],
    single_character_frequencies: dict[str, int],
    forbidden_words: list[str] | None = None,
    synonyms: dict[str, list[str]] | None = None,
    synonym_candidates: dict[str, list[str]] | None = None,
) -> None:
    name = path.stem.removesuffix("_data")
    write_annotation_store(path.with_name(f"{name}_annotations.bin"), name, annotations)
    def table_map(values: dict[str, str]) -> str:
        return "\n".join(f"  [{lua_quote(key)}] = {lua_quote(value)}," for key, value in sorted(values.items()))

    def list_map(values: dict[str, list[str]]) -> str:
        lines = []
        for key, items in sorted(values.items()):
            body = ", ".join(lua_quote(item) for item in items)
            lines.append(f"  [{lua_quote(key)}] = {{{body}}},")
        return "\n".join(lines)

    def number_map(values: dict[str, int]) -> str:
        return "\n".join(f"  [{lua_quote(key)}] = {value}," for key, value in sorted(values.items()))

    forbidden = ""
    if forbidden_words:
        forbidden = " forbidden_words = {\n" + "\n".join(f"  [{lua_quote(word)}] = true," for word in sorted(forbidden_words)) + "\n },\n"
    synonym_table = "" if synonyms is None else f" synonyms = {{\n{list_map(synonyms)}\n }},\n"
    if synonym_candidates is not None:
        synonym_table += f" synonym_candidates = {{\n{list_map(synonym_candidates)}\n }},\n"
    path.write_text(
        "return {\n"
        + forbidden
        + synonym_table
        + f" annotations = require(\"gannyu_annotation_store\").open({lua_quote(name)}),\n"
        f" readings = {{\n{table_map(readings)}\n }},\n"
        f" before = {{\n{list_map(before)}\n }},\n"
        f" after = {{\n{list_map(after)}\n }},\n"
        f" single_character_frequencies = {{\n{number_map(single_character_frequencies)}\n }},\n"
        "}\n",
        encoding="utf-8",
    )


OPTION_NAMES = (
    "gannyu_no_annotations", "gannyu_no_relations", "gannyu_no_synonyms", "gannyu_no_fuzzy",
    "gannyu_no_mandarin", "gannyu_no_wenbai", "gannyu_no_fuzzy_rules", "gannyu_both", "gannyu_new", "gannyu_old",
)


def selected_entries(entries: list[Entry], mode: str, relations: bool) -> list[Entry]:
    pairs = build_paired_readings(entries)
    mandarin_words = {word for entry in entries for word in split_values(entry.mandarin_word) if word != entry.word}
    selected = []
    for entry in entries:
        if not relations and len(entry.word) > 1 and (entry.category == "官" or entry.word in mandarin_words):
            continue
        if (mode == "new" and entry.new_old.startswith("老")) or (mode == "old" and entry.new_old.startswith("新")):
            continue
        parts = syllables(entry.gan)
        if mode != "both" and len(entry.word) > 1 and len(parts) == len(entry.word):
            for index, char in enumerate(entry.word):
                pair = next((pair for pair in pairs.get(char, []) if pair[2] == "newold"
                             and strip_tone(parts[index]) in {strip_tone(pair[0]), strip_tone(pair[1])}), None)
                if pair:
                    reading = pair[0 if mode == "new" else 1]
                    parts[index] = strip_tone(reading) + "0" if trailing_tone(parts[index]) == "0" else reading
            entry = replace(entry, gan=" ".join(parts))
        if not relations:
            entry = replace(entry, mandarin_word="", category="赣" if entry.category == "官" else entry.category)
        selected.append(entry)
    return selected


def write_option_resources(region: str, entries: list[Entry], output: Path,
                           full_algebra: list[str], special_algebra: list[str]) -> list[str]:
    dependencies = []
    root = f"gannyu_{region}"
    forbidden_words = sorted({entry.word for entry in entries if entry.category == "官" and len(entry.word) > 1}
                             | {word for entry in entries for word in split_values(entry.mandarin_word) if word != entry.word and len(word) > 1})
    for mode, relations in itertools.product(("both", "new", "old"), (True, False)):
        suffix = "" if mode == "both" and relations else f"_{mode}_" + ("rel" if relations else "norel")
        name = root + suffix
        selected = entries if not suffix else selected_entries(entries, mode, relations)
        if suffix:
            write_dictionary(output / f"{name}.dict.yaml", selected, region, name)
            annotations, before, after = build_metadata(selected, mode, include_synonyms=False)
            write_lua_data(output / "lua" / f"{name}_data.lua", annotations,
                           build_preferred_readings(selected), before, after,
                           build_single_character_frequencies(selected), None if relations else forbidden_words,
                           synonyms=build_synonym_relations(selected),
                           synonym_candidates=build_synonym_relations(selected, known_only=True))
        for full, mandarin, wenbai in itertools.product((True, False), repeat=3):
            prism_suffix = "" if full and mandarin else ("_full" if full else "_special") + ("_mix" if mandarin else "_gan")
            if not wenbai:
                prism_suffix += "_nowenbai"
            schema_id = name + prism_suffix
            if schema_id == root:
                continue
            algebra = list(full_algebra if full else special_algebra)
            algebra.insert(0, "    - xform/^W/G/" if wenbai else "    - erase/^W.*$/")
            if not mandarin:
                algebra.insert(-3, "    - erase/^[^GF].*$/")
            body = (
                f"schema:\n  schema_id: {schema_id}\n  name: {region}\n  version: \"{VERSION}\"\n"
                f"translator:\n  dictionary: {name}\n  prism: {schema_id}\n"
                f"  user_dict: {name}\n  enable_user_dict: true\n  enable_sentence: true\n"
                "  enable_completion: true\n  spelling_hints: 8\n"
                "speller:\n  alphabet: zyxwvutsrqponmlkjihgfedcba\n  delimiter: \" '\"\n  algebra:\n"
                + "\n".join(algebra) + "\n"
            )
            (output / f"{schema_id}.schema.yaml").write_text(body, encoding="utf-8")
            dependencies.append(schema_id)
    return dependencies


def write_default_custom(output: Path, regions: tuple[str, ...]) -> None:
    lines = ["patch:", "  switcher/save_options/+:" ]
    lines.extend(f"    - {option}" for option in OPTION_NAMES)
    lines.append("  schema_list/+:")
    lines.extend(f"    - schema: gannyu_{region}" for region in regions)
    (output / "default.custom.yaml").write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_mobile_default(output: Path, regions: tuple[str, ...]) -> None:
    """Write the self-contained default config used by mobile predeployment.

    Desktop Rime clients bring a distribution ``default.yaml`` with unrelated
    schemas such as luna_pinyin. Mobile packages ship only Gonnyu schemas, so
    their base config must not name an external schema during deployment.
    """
    schema_list = "\n".join(f"  - schema: gannyu_{region}" for region in regions)
    (output / "default.yaml").write_text(
        "# Generated mobile Rime defaults; do not edit.\n"
        f"config_version: \"{VERSION}\"\n\n"
        f"schema_list:\n{schema_list}\n\n"
        "menu:\n"
        "  page_size: 9\n\n"
        "punctuator:\n"
        "  half_shape:\n"
        "    ',' : { commit: ， }\n"
        "    '.' : { commit: 。 }\n"
        "    '?' : { commit: ？ }\n"
        "    '!' : { commit: ！ }\n"
        "  full_shape:\n"
        "    ',' : { commit: ， }\n"
        "    '.' : { commit: 。 }\n"
        "    '?' : { commit: ？ }\n"
        "    '!' : { commit: ！ }\n\n"
        "key_binder:\n"
        "  bindings:\n"
        "    - { when: paging, accept: minus, send: Page_Up }\n"
        "    - { when: has_menu, accept: equal, send: Page_Down }\n\n"
        "recognizer:\n"
        "  patterns:\n"
        "    uppercase: '[A-Z][-_+.''0-9A-Za-z]*$'\n",
        encoding="utf-8",
    )


def write_resource_manifest(output: Path, regions: tuple[str, ...], package_files: tuple[str, ...] | None = None) -> None:
    files = []
    paths = (output / name for name in package_files) if package_files is not None else output.rglob("*")
    for path in sorted(paths):
        relative = path.relative_to(output).as_posix()
        if relative == "resource-manifest.json" or path.name.startswith(".gannyu-cache-"):
            continue
        if not path.is_file():
            if package_files is not None:
                raise FileNotFoundError(path)
            continue
        files.append(
            {
                "path": relative,
                "size": path.stat().st_size,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
    payload = {
        "product_version": VERSION,
        "schema_version": VERSION,
        "regions": [
            {
                "id": region,
                "name_zh": region_name(region),
                "schema_id": f"gannyu_{region}",
            }
            for region in regions
        ],
        "files": files,
    }
    (output / "resource-manifest.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def legacy_fingerprint(paths: list[Path], options: str, old_builder_hash: str) -> str:
    records = [
        (
            path.relative_to(ROOT).as_posix(),
            old_builder_hash if path == PLATFORM_DIR / "build.py" else build_cache.digest(path),
        )
        for path in sorted(set(paths))
    ]
    return hashlib.sha256(json.dumps([options, records], sort_keys=True).encode()).hexdigest()


def resource_logic_fingerprint() -> str:
    def dump(node: ast.AST) -> str:
        try:
            return ast.dump(node, include_attributes=False, show_empty=True)
        except TypeError:
            return ast.dump(node, include_attributes=False)

    tree = ast.parse(Path(__file__).read_text(encoding="utf-8"))
    selected = []
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in RESOURCE_LOGIC_FUNCTIONS:
            selected.append(dump(node))
        elif isinstance(node, ast.ClassDef) and node.name == "Entry":
            selected.append(dump(node))
        elif isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id in RESOURCE_LOGIC_CONSTANTS
            for target in node.targets
        ):
            selected.append(dump(node))
    return hashlib.sha256("\n".join(selected).encode()).hexdigest()


def build(region: str, output: Path, display_name: str = "short") -> dict[str, int]:
    region_dir = ROOT / "resources" / "regions" / region
    config_path = region_dir / "region.toml"
    with config_path.open("rb") as handle:
        config = tomllib.load(handle)
    paths = [config_path, ROOT / "LICENSE", PLATFORM_DIR / "build.py",
             PLATFORM_DIR / "fuzzy.py", PLATFORM_DIR / "build_cache.py",
             PLATFORM_DIR / "gannyu.schema.yaml",
             *(PLATFORM_DIR / name for name in LUA_NAMES)]
    paths.extend(region_dir / config["language"][name]
                 for name in ("chars", "words", "gan_chars", "gan_words")
                 if config["language"].get(name))
    paths.append((region_dir / config["phonology"]["fuzzy_map"]).resolve())
    key = build_cache.fingerprint(ROOT, paths, display_name)
    stamp = output / f".gannyu-cache-{region}.json"
    cached = build_cache.load(stamp, output, key)
    if (
        cached is None
        and stamp.is_file()
        and resource_logic_fingerprint() == LEGACY_RESOURCE_LOGIC_HASH
    ):
        try:
            stamp_data = json.loads(stamp.read_text(encoding="utf-8"))
            cached_files = stamp_data.get("files", {})
            cached_version = next(
                match.group(1)
                for name in cached_files
                if name.endswith((".schema.yaml", ".dict.yaml"))
                and (match := VERSION_LINE.search((output / name).read_text(encoding="utf-8")))
            )
        except (OSError, ValueError, KeyError, TypeError, StopIteration):
            cached_version = ""
        for legacy_build_hash, versioned_options in LEGACY_BUILD_HASHES:
            if not cached_version:
                break
            legacy_options = f"{cached_version}:{display_name}" if versioned_options else display_name
            legacy_key = legacy_fingerprint(paths, legacy_options, legacy_build_hash)
            cached = build_cache.load(stamp, output, legacy_key)
            if cached is not None:
                print(f"resource-cache [{region}] legacy hit", flush=True)
                break
    if cached is not None:
        cached_files = [output / name for name in cached["files"]]
        if cached["key"] != key:
            build_cache.save(stamp, output, key, cached_files, cached["counts"])
        print(f"resource-cache [{region}] hit", flush=True)
        return cached["counts"]
    print(f"resource-cache [{region}] rebuild", flush=True)
    for path in output.glob(f"gannyu_{region}.*"):
        if path.is_file():
            path.unlink()
    for path in output.glob(f"gannyu_{region}_*"):
        if path.is_file():
            path.unlink()
    for path in (output / "lua").glob(f"gannyu_{region}*"):
        if path.is_file():
            path.unlink()
    counts = build_uncached(region, output, display_name)
    files = [path for path in output.rglob("*") if path.is_file()
             and (path.name.startswith(f"gannyu_{region}.")
                  or path.name.startswith(f"gannyu_{region}_")
                  or path.name in LUA_NAMES or path.name == "LICENSE")]
    if build_cache.fingerprint(ROOT, paths, display_name) != key:
        raise RuntimeError(f"[{region}] source changed during generation")
    build_cache.save(stamp, output, key, files, counts)
    return counts


def build_uncached(region: str, output: Path, display_name: str = "short") -> dict[str, int]:
    region_name, entries = load_entries(region)
    region_dir = ROOT / "resources" / "regions" / region
    with (region_dir / "region.toml").open("rb") as handle:
        config = tomllib.load(handle)
    rules = load_rules(
        (region_dir / config["phonology"]["fuzzy_map"]).resolve(),
        region,
    )
    canonical = {
        strip_tone(reading)
        for entry in entries
        for reading in syllables(entry.gan)
    }
    algebra = compile_algebra(canonical, rules)
    output.mkdir(parents=True, exist_ok=True)
    (output / "lua").mkdir(exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", output / "LICENSE")
    dictionary_count = write_dictionary(output / f"gannyu_{region}.dict.yaml", entries, region)
    annotations, before, after = build_metadata(entries, include_synonyms=False)
    readings = build_preferred_readings(entries)
    single_character_frequencies = build_single_character_frequencies(entries)
    schema_id = f"gannyu_{region}"
    write_lua_data(
        output / "lua" / f"{schema_id}_data.lua",
        annotations,
        readings,
        before,
        after,
        single_character_frequencies,
        synonyms=build_synonym_relations(entries),
        synonym_candidates=build_synonym_relations(entries, known_only=True),
    )
    for name, content in LUA_SOURCES.items():
        (output / "lua" / name).write_bytes(content)
    schema = SCHEMA_TEMPLATE
    label = f"赣语－{region_name}" if display_name == "apple" else region_name[:1]
    schema = (
        schema.replace("@REGION@", region)
        .replace("@REGION_NAME@", region_name)
        .replace("@REGION_LABEL@", label)
        .replace("@VERSION@", VERSION)
        .replace("@FUZZY_ALGEBRA@", "\n".join(["    - xform/^W/G/", *algebra]))
    )
    (output / f"{schema_id}.schema.yaml").write_text(schema, encoding="utf-8")
    dependencies = write_option_resources(region, entries, output, algebra, compile_algebra(canonical, essential_rules(rules)))
    schema_path = output / f"{schema_id}.schema.yaml"
    schema_path.write_text(schema_path.read_text(encoding="utf-8").replace("@OPTIONS_DEPENDENCIES@", "  dependencies:\n" + "\n".join(f"    - {name}" for name in dependencies)), encoding="utf-8")
    write_default_custom(output, (region,))
    write_resource_manifest(output, (region,))
    return {
        "entries": len(entries),
        "dictionary_records": dictionary_count,
        "annotations": len(annotations),
        "readings": len(readings),
        "relations": len(before) + len(after),
        "fuzzy_spellings": len(algebra) - 3,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", default="all")
    parser.add_argument("--list-regions", action="store_true")
    parser.add_argument("--display-name", choices=("short", "apple"), default="short")
    parser.add_argument("--output", type=Path, default=ROOT / "build" / "rime")
    args = parser.parse_args()
    available = active_regions()
    if args.list_regions:
        print("\n".join(available))
        return
    if args.region == "all":
        regions = available
    elif args.region in available:
        regions = (args.region,)
    else:
        parser.error(f"inactive or unknown region: {args.region}")
    for region in regions:
        counts = build(region, args.output.resolve(), args.display_name)
        print(
            f"built Rime schema for {region}: "
            + ", ".join(f"{key}={value}" for key, value in counts.items())
        )
    write_default_custom(args.output.resolve(), regions)
    write_mobile_default(args.output.resolve(), regions)
    write_resource_manifest(args.output.resolve(), regions)


if __name__ == "__main__":
    main()
