"""Compile the canonical fuzzy table into source-scoped Rime spelling algebra."""

from __future__ import annotations

import csv
import re
from dataclasses import dataclass
from pathlib import Path


MAX_FORMS = 64
TIER_RANK = {"primary": 0, "secondary": 1, "fallback": 2}


@dataclass(frozen=True)
class FuzzyRule:
    region: str
    source: str
    target: str
    applies: str
    bidirectional: bool
    chainable: bool
    tier: str
    starts_with: tuple[str, ...]
    condition: str = ""


def strip_tone(value: str) -> str:
    return re.sub(r"\d+\*?$", "", value)


def load_rules(path: Path, region: str | None = None) -> list[FuzzyRule]:
    with path.open(encoding="utf-8", newline="") as handle:
        lines = (line for line in handle if line.strip() and not line.lstrip().startswith("#"))
        reader = csv.DictReader(lines, delimiter="\t")
        rules = []
        for row in reader:
            rule_region = row.get("region", "").strip() or "common"
            if rule_region != "common" and rule_region != region:
                continue
            tier = row["priority_tier"].strip() or "primary"
            if tier not in TIER_RANK:
                raise ValueError(f"invalid fuzzy priority tier: {tier}")
            condition = row.get("conversion_condition", "").strip()
            if condition not in {"", "none", "not-derived-from-ir"}:
                raise ValueError(f"invalid fuzzy conversion condition: {condition}")
            if condition == "not-derived-from-ir" and (
                row["gon_fuzzy"].strip(), row["gon_pin"].strip()
            ) not in {
                ("e", "ê"), ("ee", "ê")
            }:
                raise ValueError("not-derived-from-ir is only valid for e/ee→ê")
            condition = "" if condition in {"", "none"} else condition
            rules.append(
                FuzzyRule(
                    region=rule_region,
                    source=row["gon_fuzzy"].strip(),
                    target=row["gon_pin"].strip(),
                    applies=row["applies"].strip() or "anywhere",
                    bidirectional=row["bidirectional"].strip() != "false",
                    chainable=row["chainable"].strip() != "false",
                    tier=tier,
                    starts_with=tuple(
                        item.strip() for item in row["starts_with"].split(",") if item.strip()
                    ),
                    condition=condition,
                )
            )
    return rules


def substitute_matches(
    text: str, source: str, target: str, applies: str, initials: tuple[str, ...] = ()
) -> list[tuple[str, int, int]]:
    if applies == "syllable-initial":
        if len(target) > len(source) and text.startswith(target):
            return []
        return [(target + text[len(source) :], 0, len(source))] if text.startswith(source) else []
    if applies == "syllable-final":
        if source == "o" and target == "eo" and text != source:
            return []
        if source == "eu" and target == "ieu" and text.endswith("ieu"):
            return []
        if not source:
            if not target or text.endswith(("p", "t", "k")):
                return []
            return [(text + target, len(text), 0)]
        if source == "u" and target == "yu":
            stem = text[: -len(source)]
            if stem.endswith(("y", "w")):
                return []
        return [(text[: -len(source)] + target, len(text) - len(source), len(source))] if text.endswith(source) else []
    if applies != "anywhere":
        raise ValueError(f"invalid fuzzy applies: {applies}")
    if not source:
        return []
    outputs = []
    start = 0
    while (position := text.find(source, start)) >= 0:
        if source == "u" and target == "yu" and text[:position] not in initials:
            start = position + len(source)
            continue
        outputs.append((text[:position] + target + text[position + len(source) :],
                        position, len(source)))
        start = position + len(source)
    return outputs


def substitute(
    text: str, source: str, target: str, applies: str, initials: tuple[str, ...] = ()
) -> list[str]:
    return [candidate for candidate, _, _ in substitute_matches(text, source, target, applies, initials)]


PLAIN_E_ALIASES = {("ir", "e"), ("er", "e")}
PLAIN_E_ALIAS_SOURCES = {source for source, _target in PLAIN_E_ALIASES}


def _carry_plain_e_origins(origins: frozenset[int], start: int, source_length: int,
                           target_length: int, source: str, target: str) -> frozenset[int]:
    end = start + source_length
    shift = target_length - source_length
    carried = {position if position < start else position + shift
               for position in origins if position >= end}
    carried.update(position for position in origins if position < start)
    if any(start <= position < end for position in origins):
        carried.update(range(start, start + target_length))
    if (source, target) in PLAIN_E_ALIASES:
        carried.add(start)
    return frozenset(carried)


def normalize(value: str, rules: list[FuzzyRule], reverse: bool = False) -> dict[str, int]:
    outputs: list[tuple[str, int, bool, frozenset[int]]] = [(strip_tone(value), 0, True, frozenset())]
    produced = {(outputs[0][0], outputs[0][3])}
    produced_texts = {outputs[0][0]}
    cursor = 0
    while cursor < len(outputs) and len(produced_texts) < MAX_FORMS:
        base, base_tier, expandable, origins = outputs[cursor]
        cursor += 1
        if not expandable:
            continue
        for rule in rules:
            if rule.starts_with and not base.startswith(rule.starts_with):
                continue
            if reverse:
                if not rule.bidirectional:
                    continue
                directions = [(rule.target, rule.source)]
            else:
                directions = [(rule.source, rule.target)]
                if rule.bidirectional:
                    directions.append((rule.target, rule.source))
            for source, target in directions:
                for candidate, start, source_length in substitute_matches(
                    base, source, target, rule.applies, rule.starts_with
                ):
                    if rule.condition == "not-derived-from-ir" and any(
                        start <= position < start + source_length for position in origins
                    ):
                        continue
                    candidate_origins = _carry_plain_e_origins(
                        origins, start, source_length, len(target), source, target
                    )
                    state = (candidate, candidate_origins)
                    if state in produced:
                        continue
                    if candidate not in produced_texts and len(produced_texts) >= MAX_FORMS:
                        continue
                    produced.add(state)
                    produced_texts.add(candidate)
                    outputs.append(
                        (candidate, max(base_tier, TIER_RANK[rule.tier]), rule.chainable,
                         candidate_origins)
                    )
    forms = {}
    for text, tier, _, _ in outputs:
        forms[text] = min(tier, forms.get(text, tier))
    return forms


def inverse_candidates(value: str, rules: list[FuzzyRule]) -> set[str]:
    outputs = {value}
    frontier = [value]
    while frontier and len(outputs) < MAX_FORMS:
        base = frontier.pop(0)
        for rule in rules:
            if (rule.source, rule.target) in PLAIN_E_ALIASES and any(
                alias in base for alias in PLAIN_E_ALIAS_SOURCES
            ):
                continue
            directions = [(rule.target, rule.source)]
            if rule.bidirectional:
                directions.append((rule.source, rule.target))
            for source, target in directions:
                for candidate in substitute(base, source, target, rule.applies, rule.starts_with):
                    if candidate in outputs:
                        continue
                    outputs.add(candidate)
                    frontier.append(candidate)
                    if len(outputs) >= MAX_FORMS:
                        break
                if len(outputs) >= MAX_FORMS:
                    break
            if len(outputs) >= MAX_FORMS:
                break
    return outputs


def compile_algebra(canonical: set[str], rules: list[FuzzyRule]) -> list[str]:
    aliases: dict[tuple[str, str], int] = {}
    for target in sorted(canonical):
        for source in inverse_candidates(target, rules):
            if source == target:
                continue
            forward = normalize(source, rules)
            backward = normalize(source, rules, reverse=True)
            tiers = [forms[target] for forms in (forward, backward) if target in forms]
            if tiers:
                aliases[(target, source)] = min(tiers)

    algebra = []
    for (target, source), tier in sorted(aliases.items()):
        operator = "derive" if tier == 0 else "fuzz"
        algebra.append(f"    - {operator}/^G{re.escape(target)}$/F{source}/")
    algebra.extend(
        (
            "    - xform/^G//",
            "    - xform/^F//",
            "    - abbrev/^([a-z]).+$/$1/",
        )
    )
    return algebra


def essential_rules(rules: list[FuzzyRule]) -> list[FuzzyRule]:
    return [rule for rule in rules if (rule.source, rule.target) in {
        ("i", "ï"), ("ii", "ï"), ("e", "ê"), ("ee", "ê"), ("ir", "e"), ("er", "e"),
        ("o", "ô"), ("oo", "ô"),
    }]
