#!/usr/bin/env python3
"""Build a portable Rime schema from the canonical regional TSV files."""

from __future__ import annotations

import argparse
import csv
import itertools
import re
import shutil
import tomllib
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

try:
    from .fuzzy import compile_algebra, load_rules
except ImportError:
    from fuzzy import compile_algebra, load_rules


ROOT = Path(__file__).resolve().parents[2]
PLATFORM_DIR = Path(__file__).resolve().parent
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

def active_regions() -> tuple[str, ...]:
    regions = []
    for config_path in sorted((ROOT / "resources" / "regions").glob("*/region.toml")):
        with config_path.open("rb") as handle:
            config = tomllib.load(handle)
        if config.get("region", {}).get("status") == "active":
            regions.append(config_path.parent.name)
    if not regions:
        raise ValueError("no active Rime regions")
    return tuple(regions)


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
    return re.sub(r"\d+$", "", value)


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
) -> str:
    if not syllables(entry.gan):
        return ""
    if len(entry.word) == 1:
        match = matching_pair(entry.word, entry.gan, paired_readings, new_old, heteronyms)
        if match is not None:
            first, second, kind = match
            return format_pair_display(first, second, kind, False)
    prefix = {"文": "[文]", "白": "[白]"}.get(entry.category, "")
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
    match = re.search(r"\d+$", value)
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


def build_metadata(entries: list[Entry]) -> tuple[dict[str, str], dict[str, list[str]], dict[str, list[str]]]:
    by_word: dict[str, list[Entry]] = defaultdict(list)
    associates: dict[str, set[str]] = defaultdict(set)
    gan_to_mandarin: dict[str, list[str]] = defaultdict(list)
    mandarin_to_gan: dict[str, list[str]] = defaultdict(list)
    for entry in entries:
        by_word[entry.word].append(entry)
        for synonym in split_values(entry.synonyms):
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
            display = annotated_reading(entry, new_old, heteronyms, paired_readings)
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
                readings = [stored]
            else:
                first, second, _kind = match
                readings = [strip_tone(first)]
                second = strip_tone(second)
                if second not in readings:
                    readings.append(second)
            choices.append([f"G{reading}" for reading in readings])
        total = 1
        for options in choices:
            total *= len(options)
        if total <= MAX_FORMS:
            codes.update(" ".join(parts) for parts in itertools.product(*choices))
    return {code for code in codes if code}


def write_dictionary(path: Path, entries: list[Entry], region: str) -> int:
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
            f'name: gannyu_{region}\nversion: "{VERSION}"\nsort: by_weight\n'
            "use_preset_vocabulary: false\n...\n\n"
        )
        for (word, code), frequency in sorted(records.items()):
            handle.write(f"{word}\t{code}\t{frequency}\n")
    return len(records)


def write_lua_data(
    path: Path,
    annotations: dict[str, str],
    readings: dict[str, str],
    before: dict[str, list[str]],
    after: dict[str, list[str]],
) -> None:
    def table_map(values: dict[str, str]) -> str:
        return "\n".join(f"  [{lua_quote(key)}] = {lua_quote(value)}," for key, value in sorted(values.items()))

    def list_map(values: dict[str, list[str]]) -> str:
        lines = []
        for key, items in sorted(values.items()):
            body = ", ".join(lua_quote(item) for item in items)
            lines.append(f"  [{lua_quote(key)}] = {{{body}}},")
        return "\n".join(lines)

    path.write_text(
        "return {\n"
        f" annotations = {{\n{table_map(annotations)}\n }},\n"
        f" readings = {{\n{table_map(readings)}\n }},\n"
        f" before = {{\n{list_map(before)}\n }},\n"
        f" after = {{\n{list_map(after)}\n }},\n"
        "}\n",
        encoding="utf-8",
    )


def write_default_custom(output: Path, regions: tuple[str, ...]) -> None:
    lines = ["patch:", "  schema_list/+:" ]
    lines.extend(f"    - schema: gannyu_{region}" for region in regions)
    (output / "default.custom.yaml").write_text("\n".join(lines) + "\n", encoding="utf-8")

def build(region: str, output: Path, display_name: str = "short") -> dict[str, int]:
    region_name, entries = load_entries(region)
    region_dir = ROOT / "resources" / "regions" / region
    with (region_dir / "region.toml").open("rb") as handle:
        config = tomllib.load(handle)
    rules = load_rules((region_dir / config["phonology"]["fuzzy_map"]).resolve())
    canonical = {
        strip_tone(reading)
        for entry in entries
        for reading in syllables(entry.gan)
    }
    algebra = compile_algebra(canonical, rules)
    output.mkdir(parents=True, exist_ok=True)
    (output / "lua").mkdir(exist_ok=True)
    dictionary_count = write_dictionary(output / f"gannyu_{region}.dict.yaml", entries, region)
    annotations, before, after = build_metadata(entries)
    readings = build_preferred_readings(entries)
    schema_id = f"gannyu_{region}"
    write_lua_data(output / "lua" / f"{schema_id}_data.lua", annotations, readings, before, after)
    shutil.copy2(PLATFORM_DIR / "gannyu_filter.lua", output / "lua" / "gannyu_filter.lua")
    schema = (PLATFORM_DIR / "gannyu.schema.yaml").read_text(encoding="utf-8")
    label = f"赣语－{region_name}" if display_name == "apple" else region_name[:1]
    schema = (
        schema.replace("@REGION@", region)
        .replace("@REGION_NAME@", region_name)
        .replace("@REGION_LABEL@", label)
        .replace("@VERSION@", VERSION)
        .replace("@FUZZY_ALGEBRA@", "\n".join(algebra))
    )
    (output / f"{schema_id}.schema.yaml").write_text(schema, encoding="utf-8")
    write_default_custom(output, (region,))
    return {
        "entries": len(entries),
        "dictionary_records": dictionary_count,
        "annotations": len(annotations),
        "readings": len(readings),
        "relations": len(before) + len(after),
        "fuzzy_spellings": len(algebra) - 2,
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


if __name__ == "__main__":
    main()
