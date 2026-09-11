from pathlib import Path

from platforms.rime.build import (
    Entry,
    annotated_reading,
    build,
    build_default_codes,
    build_metadata,
    build_new_old,
    build_paired_readings,
    entry_codes,
    build_preferred_readings,
    load_default_words,
    load_entries,
)
from platforms.rime.fuzzy import compile_algebra, load_rules, normalize


RULES_PATH = Path(__file__).resolve().parents[1] / "resources" / "fuzzy_scheme.tsv"


def test_builds_rime_dictionary_annotations_and_relations(tmp_path: Path) -> None:
    counts = build("lancong", tmp_path)

    dictionary = (tmp_path / "gannyu_lancong.dict.yaml").read_text(encoding="utf-8")
    data = (tmp_path / "lua" / "gannyu_lancong_data.lua").read_text(encoding="utf-8")
    schema = (tmp_path / "gannyu_lancong.schema.yaml").read_text(encoding="utf-8")

    assert counts["dictionary_records"] > counts["entries"]
    assert counts["default_words"] == 8
    assert counts["fuzzy_spellings"] > 0
    assert "䁐牛\tGyang Gniu\t156320" in dictionary
    assert "䁐牛\tying niu\t156320" in dictionary
    assert '["䁐牛"] = "yang4 niu4 [义]放牛"' in data
    assert '  ["我"] = "ngo3",' in data
    assert '  ["们"] = "men4",' in data
    assert '  ["嗰"] = "go0",' in data
    assert not any(
        character.isdigit()
        for line in dictionary.splitlines()
        if "\t" in line
        for character in line.split("\t")[1]
    )
    defaults = data.split(" defaults = {", 1)[1]
    assert '["南昌"]' not in defaults
    assert '["䁐牛"] = {"放牛"}' in data
    assert '["我"] = {"ngo", "wo"}' in data
    assert "\t`\t" not in dictionary
    assert "lua_processor@*gannyu_default_processor" in schema
    assert "lua_translator@*gannyu_default_translator" in schema
    assert (tmp_path / "lua" / "gannyu_default_processor.lua").is_file()
    assert (tmp_path / "lua" / "gannyu_default_translator.lua").is_file()
    assert (tmp_path / "lua" / "gannyu_default_translator.lua").read_text(
        encoding="utf-8"
    ).rstrip().endswith("return M")
    assert "dictionary: gannyu_lancong" in schema
    assert "schema_id: gannyu_lancong" in schema
    assert "name: 南" in schema
    assert "menu:\n  page_size: 10" in schema
    assert "0123456789" not in schema
    assert "fuzz/^G" in schema
    assert "- xform/^G//" in schema
    assert "- xform/^F//" in schema
    assert "@FUZZY_ALGEBRA@" not in schema
    assert (tmp_path / "default.custom.yaml").is_file()


def test_rime_mandarin_only_annotation_includes_dialect_reading() -> None:
    annotations, _, _ = build_metadata([
        Entry("混混", "", "fen5 fen5", "hun4 hun4", "官", "", 100, "", ""),
    ])

    assert annotations["混混"] == "[不习用] fen5 fen5"


def test_rime_heteronym_word_annotation_marks_only_second_reading() -> None:
    entries = [
        Entry("手", "", "shou1", "", "赣", "", 100, "", "本1"),
        Entry("手", "", "sou1", "", "赣", "", 100, "", "又1"),
        Entry("手心", "", "shou1 xin1", "", "赣", "", 100, "", ""),
        Entry("语", "", "yu3", "", "赣", "", 100, "", "新1"),
        Entry("语", "", "nyu3", "", "赣", "", 100, "", "老1"),
        Entry("语言", "", "nyu3 nien4", "", "赣", "", 100, "", ""),
    ]
    new_old, heteronyms = build_new_old(entries)

    assert annotated_reading(entries[2], new_old, heteronyms) == "(shou1/[又]sou1) xin1"
    assert annotated_reading(entries[5], new_old, heteronyms) == "([新]yu3/[老]nyu3) nien4"


def test_rime_single_char_heteronym_annotation_shows_full_pair() -> None:
    entries = [
        Entry("手", "", "shou1", "", "赣", "", 100, "", "本1"),
        Entry("手", "", "sou1", "", "赣", "", 100, "", "又1"),
    ]
    new_old, heteronyms = build_new_old(entries)
    paired_readings = build_paired_readings(entries)

    assert annotated_reading(entries[0], new_old, heteronyms, paired_readings) == "shou1/[又]sou1"
    assert annotated_reading(entries[1], new_old, heteronyms, paired_readings) == "shou1/[又]sou1"


def test_word_reading_outside_pair_is_not_substituted() -> None:
    entries = [
        Entry("手", "", "shou1", "", "赣", "", 100, "", "本1"),
        Entry("手", "", "sou1", "", "赣", "", 100, "", "又1"),
        Entry("语", "", "yu3", "", "赣", "", 100, "", "新1"),
        Entry("语", "", "nyu3", "", "赣", "", 100, "", "老1"),
        Entry("谜语", "", "mi5 xi5", "", "赣", "", 100, "", ""),
        Entry("歌手", "", "ge1 xiu1", "", "赣", "", 100, "", ""),
    ]
    new_old, heteronyms = build_new_old(entries)

    assert annotated_reading(entries[4], new_old, heteronyms) == "mi5 xi5"
    assert annotated_reading(entries[5], new_old, heteronyms) == "ge1 xiu1"


def test_neutral_word_annotation_uses_first_matching_pair_only() -> None:
    entries = [
        Entry("辑", "", "qit6", "", "赣", "", 100, "", "本1"),
        Entry("辑", "", "jit6", "", "赣", "", 100, "", "又1"),
        Entry("辑", "", "lap6", "", "赣", "", 100, "", "本2"),
        Entry("辑", "", "nap6", "", "赣", "", 100, "", "又2"),
        Entry("逻辑", "", "lo5 qit0", "", "赣", "", 100, "", ""),
    ]
    new_old, heteronyms = build_new_old(entries)
    paired_readings = build_paired_readings(entries)

    assert annotated_reading(entries[4], new_old, heteronyms, paired_readings) == "lo5 (qit0/[又]jit0)"


def test_neutral_word_annotation_skips_non_matching_pair() -> None:
    entries = [
        Entry("辑", "", "lap6", "", "赣", "", 100, "", "本1"),
        Entry("辑", "", "nap6", "", "赣", "", 100, "", "又1"),
        Entry("辑", "", "qit6", "", "赣", "", 100, "", "本2"),
        Entry("辑", "", "jit6", "", "赣", "", 100, "", "又2"),
        Entry("逻辑", "", "lo5 qit0", "", "赣", "", 100, "", ""),
    ]
    new_old, heteronyms = build_new_old(entries)
    paired_readings = build_paired_readings(entries)

    assert annotated_reading(entries[4], new_old, heteronyms, paired_readings) == "lo5 (qit0/[又]jit0)"


def test_rime_entry_codes_stay_within_matching_pair_only() -> None:
    entries = [
        Entry("还", "", "hat6", "", "赣", "", 100, "", "本1"),
        Entry("还", "", "hai6", "", "赣", "", 100, "", "又1"),
        Entry("还", "", "wan6", "", "赣", "", 100, "", "本2"),
        Entry("还", "", "fan6", "", "赣", "", 100, "", "又2"),
        Entry("还有", "", "hat6 yiu3", "", "赣", "", 100, "", ""),
    ]
    paired_readings = build_paired_readings(entries)
    codes = entry_codes(entries[4], paired_readings)

    assert "Ghat Gyiu" in codes
    assert "Ghai Gyiu" in codes
    assert "Gwan Gyiu" not in codes
    assert "Gfan Gyiu" not in codes


def test_rime_subtitle_prefers_newold_over_heteronym_and_keeps_wenbai_nonpaired() -> None:
    entries = [
        Entry("横", "", "vang2", "", "赣", "", 100, "", "新1"),
        Entry("横", "", "wang2", "", "赣", "", 100, "", "老1"),
        Entry("横", "", "vang2", "", "赣", "", 100, "", "本2"),
        Entry("横", "", "fang2", "", "赣", "", 100, "", "又2"),
        Entry("横额", "", "vang2 ngak8", "", "赣", "", 100, "", ""),
        Entry("明", "", "ming5", "", "文", "", 100, "", ""),
        Entry("明", "", "miang5", "", "白", "", 100, "", ""),
        Entry("明年", "", "ming5 nien4", "", "文", "", 100, "", ""),
    ]
    new_old, heteronyms = build_new_old(entries)
    paired_readings = build_paired_readings(entries)

    assert annotated_reading(entries[4], new_old, heteronyms, paired_readings) == "([新]vang2/[老]wang2) ngak8"
    assert annotated_reading(entries[7], new_old, heteronyms, paired_readings) == "[文]ming5 nien4"


def test_builds_separate_fenni_schema(tmp_path: Path) -> None:
    counts = build("fenni", tmp_path, "apple")

    schema = (tmp_path / "gannyu_fenni.schema.yaml").read_text(encoding="utf-8")
    assert counts["fuzzy_spellings"] > 0
    assert "schema_id: gannyu_fenni" in schema
    assert "name: 赣语－分宜" in schema
    assert (tmp_path / "lua" / "gannyu_fenni_data.lua").is_file()


def test_default_words_resolve_to_normal_dictionary_codes() -> None:
    import tomllib

    region_dir = Path(__file__).resolve().parents[1] / "resources" / "regions" / "fenni"
    with (region_dir / "region.toml").open("rb") as handle:
        config = tomllib.load(handle)
    _, entries = load_entries("fenni")
    words = load_default_words(region_dir, config, entries)
    codes = build_default_codes(entries, words)

    assert words == ("我", "你", "渠", "许", "个", "啊", "嘎")
    assert set(codes) == set(words)
    assert "ngo" in codes["我"]
    assert all("`" not in code for values in codes.values() for code in values)


def test_sentence_readings_use_highest_frequency_toned_character_entries() -> None:
    _, entries = load_entries("lancong")
    readings = build_preferred_readings(entries)

    assert " ".join(readings[character] for character in "我们嗰") == "ngo3 men4 go0"


def test_lua_filter_rebuilds_sentence_readings_and_cleans_internal_marker() -> None:
    source = (
        Path(__file__).resolve().parents[1] / "platforms" / "rime" / "gannyu_filter.lua"
    ).read_text(encoding="utf-8")

    assert "sentence_reading(candidate.text, data)" in source
    assert ':gsub("^G", ""):gsub(" G", " ")' in source


def test_default_candidates_are_guarded_to_mobile_rime_frontends() -> None:
    source = (
        Path(__file__).resolve().parents[1] / "platforms" / "rime" / "gannyu_default_processor.lua"
    ).read_text(encoding="utf-8")

    assert 'distribution == "trime"' in source
    assert 'distribution == "hamster"' in source
    assert 'distribution == "irime"' in source
    assert 'user_data:find("/data/user/", 1, true)' in source
    assert 'user_data:find("/var/mobile/", 1, true)' in source
    assert "if not is_mobile_rime() then" in source


def test_mobile_default_candidates_are_dismissed_before_any_key_reaches_selector() -> None:
    source = (
        Path(__file__).resolve().parents[1] / "platforms" / "rime" / "gannyu_default_processor.lua"
    ).read_text(encoding="utf-8")

    handler = source.split("function M.func(key, env)", 1)[1].split("function M.init", 1)[0]
    assert "key:release()" in handler
    assert "clear_marker(env, context)" in handler
    assert 'repr == "Up"' not in handler
    assert 'repr:match("^[0-9]$")' not in handler


def test_fuzzy_rules_keep_core_directions_and_non_chainable_boundary() -> None:
    rules = load_rules(RULES_PATH)

    bare = normalize("ni", rules)
    assert "nit" in bare
    assert "nik" in bare
    assert "ni" not in normalize("nit", rules, reverse=True)
    assert "nik" in normalize("nit", rules)
    assert "nip" not in bare
    assert "yon" in normalize("ion", rules)
    assert "yuon" not in normalize("ion", rules)


def test_yuo_family_accepts_all_supported_mandarin_style_spellings() -> None:
    rules = load_rules(RULES_PATH)
    variants = ("yue", "ue", "ve", "ye")

    for onset in ("", "j", "n", "q", "x"):
        expected = f"{onset}yuon"
        for variant in variants:
            input_syllable = f"{onset}{variant}n"
            assert expected in normalize(input_syllable, rules)

    for onset in ("", "j", "l", "n", "q", "x"):
        expected = f"{onset}yuot"
        invalid = f"{onset}yuok"
        for variant in variants:
            for coda in ("", "t", "k"):
                input_syllable = f"{onset}{variant}{coda}"
                outputs = normalize(input_syllable, rules)
                assert expected in outputs
                assert invalid not in outputs


def test_added_theoretical_spellings_normalize_to_stored_forms() -> None:
    rules = load_rules(RULES_PATH)

    for input_syllable, expected in {
        "hieu": "heu",
        "fi": "fei",
        "zuon": "zon",
        "cuon": "con",
        "ciu": "ceu",
    }.items():
        assert expected in normalize(input_syllable, rules)


def test_mandarin_ao_and_ou_inputs_normalize_to_au_and_eu() -> None:
    rules = load_rules(RULES_PATH)

    for onset in ("b", "c", "d", "g", "h", "k", "l", "m", "ng", "p", "s", "t", "z"):
        assert f"{onset}au" in normalize(f"{onset}ao", rules)
    assert "niau" in normalize("niao", rules)

    for onset in ("c", "d", "f", "g", "h", "j", "k", "l", "m", "ng", "p", "s", "t", "y", "z"):
        assert f"{onset}eu" in normalize(f"{onset}ou", rules)
    assert "cheu" not in normalize("chou", rules)


def test_algebra_is_explicit_and_scoped_to_gan_syllables() -> None:
    rules = load_rules(RULES_PATH)
    algebra = compile_algebra({"nit", "nik", "yuon"}, rules)

    assert "    - fuzz/^Gnit$/Fni/" in algebra
    assert "    - fuzz/^Gnik$/Fni/" in algebra
    assert "    - fuzz/^Gnit$/Fnik/" in algebra
    assert "    - derive/^Gyuon$/Fyon/" in algebra
    assert not any(rule.startswith("    - ") and "Fion/" in rule for rule in algebra)
    assert all("^M" not in rule for rule in algebra)

def test_rime_installers_discover_regions_from_build_output() -> None:
    for name in ("install.sh", "install_macos.sh"):
        content = (Path(__file__).resolve().parents[1] / "platforms/rime" / name).read_text(encoding="utf-8")
        assert "gannyu_*.schema.yaml" in content
        assert "gannyu_*_data.lua" in content
        assert "gannyu_default_*.lua" in content
        assert 'build.py" --list-regions' in content
        assert "gannyu_lancong" not in content
        assert "gannyu_fenni" not in content
