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
    source: str
    target: str
    applies: str
    bidirectional: bool
    chainable: bool
    tier: str
    starts_with: tuple[str, ...]


def strip_tone(value: str) -> str:
    return re.sub(r"\d+$", "", value)


def load_rules(path: Path) -> list[FuzzyRule]:
    with path.open(encoding="utf-8", newline="") as handle:
        lines = (line for line in handle if line.strip() and not line.lstrip().startswith("#"))
        reader = csv.DictReader(lines, delimiter="\t")
        rules = []
        for row in reader:
            tier = row["priority_tier"].strip() or "primary"
            if tier not in TIER_RANK:
                raise ValueError(f"invalid fuzzy priority tier: {tier}")
            rules.append(
                FuzzyRule(
                    source=row["gon_han"].strip(),
                    target=row["gon_pin"].strip(),
                    applies=row["applies"].strip() or "anywhere",
                    bidirectional=row["bidirectional"].strip() != "false",
                    chainable=row["chainable"].strip() != "false",
                    tier=tier,
                    starts_with=tuple(
                        item.strip() for item in row["starts_with"].split(",") if item.strip()
                    ),
                )
            )
    return rules


def substitute(text: str, source: str, target: str, applies: str) -> list[str]:
    if applies == "syllable-initial":
        return [target + text[len(source) :]] if text.startswith(source) else []
    if applies == "syllable-final":
        if not source:
            if not target or text.endswith(("t", "k")):
                return []
            return [text + target]
        if source == "u" and target == "yu":
            stem = text[: -len(source)]
            if stem.endswith(("y", "w")):
                return []
        return [text[: -len(source)] + target] if text.endswith(source) else []
    if applies != "anywhere":
        raise ValueError(f"invalid fuzzy applies: {applies}")
    if not source:
        return []
    outputs = []
    start = 0
    while (position := text.find(source, start)) >= 0:
        if source == "y" and target == "yu" and text[position + 1 :].startswith("u"):
            start = position + len(source)
            continue
        outputs.append(text[:position] + target + text[position + len(source) :])
        start = position + len(source)
    return outputs


def normalize(value: str, rules: list[FuzzyRule], reverse: bool = False) -> dict[str, int]:
    outputs: list[tuple[str, int, bool]] = [(strip_tone(value), 0, True)]
    produced = {outputs[0][0]}
    cursor = 0
    while cursor < len(outputs) and len(outputs) < MAX_FORMS:
        base, base_tier, expandable = outputs[cursor]
        cursor += 1
        if not expandable:
            continue
        for rule in rules:
            if rule.starts_with and not base.startswith(rule.starts_with):
                continue
            if reverse:
                if not rule.bidirectional:
                    continue
                source, target = rule.target, rule.source
            else:
                source, target = rule.source, rule.target
            for candidate in substitute(base, source, target, rule.applies):
                if candidate in produced:
                    continue
                produced.add(candidate)
                outputs.append(
                    (candidate, max(base_tier, TIER_RANK[rule.tier]), rule.chainable)
                )
                if len(outputs) >= MAX_FORMS:
                    break
            if len(outputs) >= MAX_FORMS:
                break
    return {text: tier for text, tier, _ in outputs}


def inverse_candidates(value: str, rules: list[FuzzyRule]) -> set[str]:
    outputs = {value}
    frontier = [value]
    while frontier and len(outputs) < MAX_FORMS:
        base = frontier.pop(0)
        for rule in rules:
            directions = [(rule.target, rule.source)]
            if rule.bidirectional:
                directions.append((rule.source, rule.target))
            for source, target in directions:
                for candidate in substitute(base, source, target, rule.applies):
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
    algebra.extend(("    - xform/^G//", "    - xform/^F//"))
    return algebra
