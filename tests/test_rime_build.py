import hashlib
import json
from pathlib import Path

from platforms.rime.build import (
    Entry,
    annotated_reading,
    active_regions,
    build,
    build_metadata,
    build_single_character_frequencies,
    build_new_old,
    build_paired_readings,
    entry_codes,
    build_preferred_readings,
    load_entries,
)
from platforms.rime.fuzzy import compile_algebra, load_rules, normalize


RULES_PATH = Path(__file__).resolve().parents[1] / "resources" / "fuzzy_scheme.tsv"


def test_active_regions_use_canonical_default_order() -> None:
    regions = active_regions()
    assert regions[0] == "lancong"
    assert len(regions) == len(set(regions))


def test_fuzzy_rules_are_scoped_to_the_selected_region(tmp_path: Path) -> None:
    path = tmp_path / "fuzzy.tsv"
    path.write_text(
        "region\tcategory\tgon_fuzzy\tgon_pin\tapplies\tbidirectional\tchainable\tpriority_tier\tstarts_with\n"
        "common\tonset\tzz\tz\tsyllable-initial\tfalse\tfalse\tprimary\t\n"
        "lancong\tonset\tll\tl\tsyllable-initial\tfalse\tfalse\tprimary\t\n"
        "fenni\tonset\tff\tf\tsyllable-initial\tfalse\tfalse\tprimary\t\n",
        encoding="utf-8",
    )

    common = load_rules(path)
    assert [rule.region for rule in common] == ["common"]
    assert "za" in normalize("zza", common)
    assert "la" not in normalize("lla", common)

    lancong = load_rules(path, "lancong")
    assert [rule.region for rule in lancong] == ["common", "lancong"]
    assert "la" in normalize("lla", lancong)
    assert "fa" not in normalize("ffa", lancong)


def test_entering_tone_rules_are_scoped_to_lancong_and_fungcen() -> None:
    lancong = load_rules(RULES_PATH, "lancong")
    assert "baet" in normalize("bae", lancong)
    assert "baek" in normalize("bae", lancong)
    assert "baep" not in normalize("bae", lancong)

    fungcen = load_rules(RULES_PATH, "fungcen")
    assert {"baet", "baep", "baek"}.issubset(normalize("bae", fungcen))
    assert "baet" in normalize("baep", fungcen, reverse=True)

    fenni = load_rules(RULES_PATH, "fenni")
    assert not {"baet", "baep", "baek"}.intersection(normalize("bae", fenni))


def test_all_regional_entering_codas_preserve_omission_and_interchange() -> None:
    for region, codas in (("lancong", "tk"), ("fungcen", "tpk"), ("tiqien", "pk"),
                          ("sinyi", "tk"), ("songau", "tk"), ("seusong", ""), ("fenni", "")):
        rules = load_rules(RULES_PATH, region)
        expected = {"bae" + coda for coda in codas}
        assert {"baet", "baep", "baek"}.intersection(normalize("bae", rules)) == expected
        algebra = compile_algebra(expected, rules)
        for coda in codas:
            spelling = "bae" + coda
            forms = set(normalize(spelling, rules)) | set(normalize(spelling, rules, reverse=True))
            assert expected.issubset(forms)
            assert not {spelling + tail for tail in "ptk"}.intersection(forms)
            assert any(line.endswith(f"/^G{spelling}$/Fbae/") for line in algebra)


def test_zero_initial_ui_compatibility_preserves_onset_ui() -> None:
    rules = load_rules(RULES_PATH)
    for spelling in ("ui", "wui"):
        assert "wi" in normalize(spelling, rules)
    assert "wui" not in normalize("wi", rules)
    assert "gui" in normalize("guei", rules)
    assert "guêi" in normalize("guei", rules)
    for spelling in ("uei", "wei", "uêi", "wêi"):
        assert "wêi" in normalize(spelling, rules)
        assert "wi" not in normalize(spelling, rules)
    assert "gwi" not in normalize("gui", rules)
    algebra = compile_algebra({"wi", "gui", "wêi"}, rules)
    for spelling in ("ui", "wui"):
        assert f"    - derive/^Gwi$/F{spelling}/" in algebra
    assert "    - derive/^Ggui$/Fguei/" in algebra
    assert "    - derive/^Ggui$/Fguêi/" in algebra


def test_ueik_rules_follow_uei_with_checked_coda() -> None:
    rules = load_rules(RULES_PATH)
    for source, target in (("uei", "wei"), ("uei", "wêi"), ("uêi", "wêi"),
                           ("uei", "ui"), ("uêi", "ui")):
        plain = [rule for rule in rules if rule.source == source and rule.target == target]
        checked = [rule for rule in rules if rule.source == source + "k" and rule.target == target + "k"]
        assert len(plain) == len(checked) == 1
        plain, checked = plain[0], checked[0]
        assert checked.applies == plain.applies
        assert checked.bidirectional == plain.bidirectional
        assert checked.chainable == plain.chainable
        assert checked.tier == plain.tier
        expected_starts = tuple(value + "k" for value in plain.starts_with) if plain.starts_with == (source,) else plain.starts_with
        assert checked.starts_with == expected_starts


def test_ueik_zero_initial_and_shortened_forms_compile_to_stored_codes() -> None:
    rules = load_rules(RULES_PATH)
    assert {"weik", "wêik"}.issubset(normalize("ueik", rules))
    assert "wêik" in normalize("uêik", rules)
    assert "weik" not in normalize("wêik", rules)
    for spelling in ("ueik", "uêik", "weik", "wêik"):
        assert "uik" not in normalize(spelling, rules)
    for initial in ("g", "n", "ng", "zh"):
        for final in ("ueik", "uêik"):
            assert initial + "uik" in normalize(initial + final, rules)
    algebra = compile_algebra({"weik", "wêik", "guik"}, rules)
    assert "    - derive/^Gweik$/Fueik/" in algebra
    assert "    - derive/^Gwêik$/Fueik/" in algebra
    assert "    - derive/^Gwêik$/Fuêik/" in algebra
    assert "    - derive/^Gguik$/Fgueik/" in algebra
    assert "    - derive/^Gguik$/Fguêik/" in algebra


def test_zero_initial_weik_never_becomes_uik() -> None:
    for region in active_regions():
        rules = load_rules(RULES_PATH, region)
        for spelling in ("weik", "wêik"):
            for reverse in (False, True):
                assert "uik" not in normalize(spelling, rules, reverse=reverse), (region, spelling, reverse)
        algebra = compile_algebra({"uik", "guik"}, rules)
        for spelling in ("weik", "wêik"):
            assert not any(
                "/^Guik$/" in rule and rule.endswith(f"/F{spelling}/")
                for rule in algebra
            ), (region, spelling)


def test_eo_accepts_only_standalone_o_one_way() -> None:
    for region in ("lancong", "fenni", "fungcen", "tiqien", "sinyi", "songau", "seusong", "jingon", "yikyan-henfeng"):
        rules = load_rules(RULES_PATH, region)
        assert "eo" in normalize("o6", rules)
        for source, target in (("e", "eo"), ("go", "geo"), ("oo", "oeo"), ("ot", "eot"), ("et", "eot"), ("got", "geot"), ("get", "geot"), ("gop", "geop"), ("gep", "geop"), ("gok", "geok"), ("gek", "geok"), ("yue", "yueo"), ("yuek", "yueok")):
            assert target not in normalize(source, rules), (region, source, target)
        for reverse in (False, True):
            forms = normalize("eo", rules, reverse=reverse)
            assert "o" not in forms and "e" not in forms
        for source in ("yuo", "yuon", "yuong", "yuot", "yuop", "yuok", "gyuo", "jyuot"):
            for reverse in (False, True):
                assert normalize(source, rules, reverse=reverse) == {source: 0}
        algebra = compile_algebra({"eo", "geot", "yueok"}, rules)
        assert "    - derive/^Gyueok$/Fyuok/" not in algebra
        assert "    - derive/^Geo$/Fo/" in algebra
        for target, source in (("eo", "e"), ("geot", "got"), ("geot", "get"),
                               ("yueok", "yuek")):
            assert f"    - derive/^G{target}$/F{source}/" not in algebra


def test_open_e_input_matches_circumflex_one_way() -> None:
    rules = load_rules(RULES_PATH)
    for plain, circumflex in (("e", "ê"), ("en", "ên"), ("gek", "gêk")):
        assert plain in normalize(plain, rules)
        assert circumflex in normalize(plain, rules)
        assert plain not in normalize(circumflex, rules)
        assert plain not in normalize(circumflex, rules, reverse=True)
    algebra = compile_algebra({"e", "ê", "gêk"}, rules)
    assert "    - derive/^Gê$/Fe/" in algebra
    assert "    - derive/^Ggêk$/Fgek/" in algebra
    assert "    - derive/^Ge$/Fê/" not in algebra


def test_zero_initial_iu_input_keeps_yiu_distinct_from_yu() -> None:
    rules = load_rules(RULES_PATH)
    for spelling in ("iu", "you", "yiu"):
        forms = normalize(spelling, rules)
        assert "yiu" in forms
        assert "yu" not in forms
    assert "yiu" not in normalize("yu", rules)
    algebra = compile_algebra({"yiu", "yu"}, rules)
    assert "    - derive/^Gyiu$/Fiu/" in algebra
    assert "    - derive/^Gyiu$/Fyou/" in algebra
    assert not any("^Gyu$/Fyiu/" in line or "^Gyiu$/Fyu/" in line for line in algebra)


def test_builds_rime_dictionary_annotations_and_relations(tmp_path: Path) -> None:
    counts = build("lancong", tmp_path)
    assert (tmp_path / "lua/gannyu_data_lifecycle.lua").is_file()

    dictionary = (tmp_path / "gannyu_lancong.dict.yaml").read_text(encoding="utf-8")
    data = (tmp_path / "lua" / "gannyu_lancong_data.lua").read_text(encoding="utf-8")
    schema = (tmp_path / "gannyu_lancong.schema.yaml").read_text(encoding="utf-8")

    assert counts["dictionary_records"] > counts["entries"]
    assert counts["fuzzy_spellings"] > 0
    assert "䁐牛\tGyang Gniu\t156320" in dictionary
    assert "䁐牛\tying niu\t156320" in dictionary
    annotations = (tmp_path / "lua/gannyu_lancong_annotations.bin").read_bytes()
    assert "䁐牛yang4 niu4 [义]放牛".encode() in annotations
    assert 'require("gannyu_annotation_store").open("gannyu_lancong")' in data
    assert '  ["我"] = "ngo3",' in data
    assert '  ["们"] = "men4",' in data
    assert '  ["嗰"] = "go0",' in data
    assert 'single_character_frequencies' in data
    assert not any(
        character.isdigit()
        for line in dictionary.splitlines()
        if "\t" in line
        for character in line.split("\t")[1]
    )
    assert '["䁐牛"] = {"放牛"}' in data
    # Associations contain related headwords, not reading spellings.  Keep this
    # regression check tied to the canonical 南昌 relation in the source data.
    assert '["我"] = {"咱"}' in data
    assert "\t`\t" not in dictionary
    assert " defaults = {" not in data
    assert "gannyu_default_processor" not in schema
    assert "gannyu_default_translator" not in schema
    assert not (tmp_path / "lua" / "gannyu_filter.lua").exists()
    assert (tmp_path / "lua" / "gannyu_annotation_filter.lua").is_file()
    assert (tmp_path / "lua" / "gannyu_single_char_filter.lua").is_file()
    assert (tmp_path / "lua" / "gannyu_relation_filter.lua").is_file()
    assert "dictionary: gannyu_lancong" in schema
    assert "schema_id: gannyu_lancong" in schema
    assert "name: 南" in schema
    assert "menu:\n  page_size: 9" in schema
    assert "0123456789" not in schema
    assert "fuzz/^G" in schema
    assert "- xform/^G//" in schema
    assert "- xform/^F//" in schema
    assert "- abbrev/^([a-z]).+$/$1/" in schema
    assert schema.index("- xform/^F//") < schema.index("- abbrev/^([a-z]).+$/$1/")
    assert "银行卡\tGnin Ghong Gka\t" in dictionary
    assert "@FUZZY_ALGEBRA@" not in schema
    assert (tmp_path / "default.custom.yaml").is_file()


def test_rime_installers_install_the_shared_single_character_filter() -> None:
    root = Path(__file__).resolve().parents[1]

    for name in ("install.sh", "install_macos.sh"):
        source = (root / "platforms" / "rime" / name).read_text(encoding="utf-8")
        assert "gannyu_single_char_filter.lua" in source


def test_rime_build_writes_resource_manifest(tmp_path: Path) -> None:
    build("lancong", tmp_path)
    manifest_path = tmp_path / "resource-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    assert manifest["product_version"]
    assert manifest["schema_version"] == manifest["product_version"]
    assert manifest["regions"] == [
        {"id": "lancong", "name_zh": "南昌", "schema_id": "gannyu_lancong"}
    ]
    assert any(file["path"] == "gannyu_lancong.schema.yaml" for file in manifest["files"])
    assert any(file["path"] == "lua/gannyu_lancong_data.lua" for file in manifest["files"])
    assert any(file["path"] == "lua/gannyu_lancong_annotations.bin" for file in manifest["files"])
    assert any(file["path"] == "lua/gannyu_annotation_store.lua" for file in manifest["files"])
    reader = (tmp_path / "lua/gannyu_annotation_store.lua").read_bytes()
    source = Path(__file__).resolve().parents[1] / "platforms/rime/gannyu_annotation_store.lua"
    assert reader == source.read_bytes()
    record = next(file for file in manifest["files"] if file["path"] == "lua/gannyu_annotation_store.lua")
    assert record["size"] == len(reader)
    assert record["sha256"] == hashlib.sha256(reader).hexdigest()


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


def test_builds_fungcen_dictionary(tmp_path: Path) -> None:
    counts = build("fungcen", tmp_path, "apple")

    dictionary = (tmp_path / "gannyu_fungcen.dict.yaml").read_text(encoding="utf-8")
    schema = (tmp_path / "gannyu_fungcen.schema.yaml").read_text(encoding="utf-8")

    assert counts["entries"] > 2800
    assert "八\tGbaet\t120873" in dictionary
    assert "插\tGcaek\t121214" in dictionary
    assert "煠\tGsaep\t120692" in dictionary
    assert "schema_id: gannyu_fungcen" in schema


def test_builds_sinyi_dictionary_with_starred_tone_markers(tmp_path: Path) -> None:
    counts = build("sinyi", tmp_path, "apple")

    dictionary = (tmp_path / "gannyu_sinyi.dict.yaml").read_text(encoding="utf-8")
    assert counts["entries"] > 0
    assert "切\tGqiêt\t" in dictionary
    assert "Gqiêt5" not in dictionary
    assert "Gqiêt5*" not in dictionary
    schema = (tmp_path / "gannyu_sinyi.schema.yaml").read_text(encoding="utf-8")
    assert "derive/^Gqiêt$/Fqiet/" in schema
    assert "丝\tGsï\t" in dictionary
    assert "derive/^Gsï$/Fsi/" in schema


def test_sentence_readings_use_highest_frequency_toned_character_entries() -> None:
    _, entries = load_entries("lancong")
    readings = build_preferred_readings(entries)

    assert " ".join(readings[character] for character in "我们嗰") == "ngo3 men4 go0"


def test_annotation_filter_rebuilds_sentence_readings_and_cleans_internal_marker() -> None:
    source = (Path(__file__).resolve().parents[1] / "platforms" / "rime" / "gannyu_annotation_filter.lua").read_text(encoding="utf-8")

    assert "sentence_reading(candidate.text, data)" in source
    assert ':gsub("^G", ""):gsub(" G", " ")' in source


def test_relation_filter_preserves_relation_ordering() -> None:
    source = (Path(__file__).resolve().parents[1] / "platforms" / "rime" / "gannyu_relation_filter.lua").read_text(encoding="utf-8")

    assert 'data.before[candidate.text]' in source
    assert 'emit_relations(candidate, data.before[candidate.text], seen, 0.01, data)' in source
    assert 'source.quality + offset' in source
    assert 'data.after[candidate.text]' in source
    assert 'emit_relations(candidate, data.after[candidate.text], seen, -0.02, data)' in source


def test_single_character_filter_keeps_anchor_and_runs_before_relations() -> None:
    root = Path(__file__).resolve().parents[1]
    schema = (root / "platforms" / "rime" / "gannyu.schema.yaml").read_text(encoding="utf-8")
    source = (root / "platforms" / "rime" / "gannyu_single_char_filter.lua").read_text(encoding="utf-8")

    assert schema.index("lua_filter@*gannyu_single_char_filter") < schema.index("lua_filter@*gannyu_relation_filter")
    assert "yield(anchor)" in source
    assert "candidate._end < anchor._end" in source
    assert "MIN_FREQUENCY = 250000" in source
    assert "data.single_character_frequencies[candidate.text]" in source
    assert 'candidate.type == "phrase" or candidate.type == "user_phrase"' in source


def test_single_character_frequency_uses_the_highest_canonical_value() -> None:
    frequencies = build_single_character_frequencies([
        Entry("上", "", "", "", "", "", 100000, "", ""),
        Entry("上", "", "", "", "", "", 508101, "", ""),
        Entry("尝试", "", "", "", "", "", 999999, "", ""),
    ])

    assert frequencies == {"上": 508101}


def test_fuzzy_rules_keep_core_directions_and_non_chainable_boundary() -> None:
    rules = load_rules(RULES_PATH, "lancong")

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
    variants = ("yue", "ue", "ve")

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
    rules = load_rules(RULES_PATH, "lancong")
    algebra = compile_algebra({"nit", "nik", "yuon"}, rules)

    assert "    - fuzz/^Gnit$/Fni/" in algebra
    assert "    - fuzz/^Gnik$/Fni/" in algebra
    assert "    - fuzz/^Gnit$/Fnik/" in algebra
    assert "    - derive/^Gyuon$/Fyon/" not in algebra
    assert not any(rule.startswith("    - ") and "Fion/" in rule for rule in algebra)
    assert all("^M" not in rule for rule in algebra)
    assert algebra[-3:] == [
        "    - xform/^G//",
        "    - xform/^F//",
        "    - abbrev/^([a-z]).+$/$1/",
    ]

def test_rime_installers_discover_regions_from_build_output() -> None:
    for name in ("install.sh", "install_macos.sh"):
        content = (Path(__file__).resolve().parents[1] / "platforms/rime" / name).read_text(encoding="utf-8")
        assert "gannyu_*.schema.yaml" in content
        assert "gannyu_*_data.lua" in content
        assert "gannyu_annotation_filter.lua" in content
        assert "gannyu_relation_filter.lua" in content
        assert '"$user_dir/lua/gannyu_filter.lua"' in content
        assert "gannyu_default_*.lua" not in content
        assert "gannyu_default_processor.lua" in content
        assert "gannyu_default_translator.lua" in content
        assert 'build.py" --list-regions' in content
        assert "gannyu_lancong" not in content
        assert "gannyu_fenni" not in content


def test_onset_contractions_preserve_zero_initial_circumflex_forms() -> None:
    rules = load_rules(RULES_PATH)
    for source, target in (("guei", "gui"), ("guêi", "gui"), ("guen", "gun"), ("guên", "gun"), ("vuei", "vui")):
        assert target in normalize(source, rules)
    for source in ("uen", "uên", "wen", "wên"):
        assert "wên" in normalize(source, rules)
    for source, forbidden in (("uei", "wi"), ("uêi", "wi"), ("uen", "un"), ("uên", "un")):
        assert forbidden not in normalize(source, rules)


def test_zero_initial_iu_suffixes_share_yu_spellings() -> None:
    rules = load_rules(RULES_PATH)
    for source, legacy, target in (("iung", "yiung", "yung"), ("iun", "yiun", "yun"), ("iuk", "yiuk", "yuk"), ("iuek", "yiuek", "yuek"), ("iuok", "yiuok", "yuok")):
        assert target in normalize(source, rules)
        assert target not in normalize(legacy, rules)
    assert "yiu" in normalize("iu", rules)
    assert "yu" not in normalize("iu", rules)
    algebra = compile_algebra({"yung", "yiu", "yu"}, rules)
    assert "    - derive/^Gyung$/Fiung/" in algebra
    assert "    - derive/^Gyung$/Fyiung/" not in algebra


def test_apical_i_rule_preserves_literal_spelling_and_is_one_way() -> None:
    rules = load_rules(RULES_PATH)
    for plain, marked in (("i", "ï"), ("si", "sï"), ("sik", "sïk")):
        assert marked in normalize(plain, rules)
        assert plain in normalize(plain, rules)
        assert plain not in normalize(marked, rules)
    algebra = compile_algebra({"sï", "sïk"}, rules)
    assert "    - derive/^Gsï$/Fsi/" in algebra
    assert "    - derive/^Gsïk$/Fsik/" in algebra


def test_u_to_yu_requires_an_immediately_preceding_initial() -> None:
    rules = load_rules(RULES_PATH)
    for initial in ("b", "p", "m", "f", "v", "d", "t", "n", "l", "g", "k", "h", "j", "q", "x", "r", "z", "c", "s", "ng", "zh", "ch", "sh"):
        for ending in ("u", "un", "ung", "uon", "uot"):
            assert f"{initial}y{ending}" in normalize(f"{initial}{ending}", rules)
    for source, forbidden in (("u", "yu"), ("un", "yun"), ("wu", "wyu"), ("yu", "yyu"), ("nyu", "nyyu"), ("yuu", "yuyu"), ("nuu", "nuyu"), ("nau", "nayu")):
        assert forbidden not in normalize(source, rules)


def test_y_to_yu_expansions_are_removed() -> None:
    rules = load_rules(RULES_PATH)
    for initial in ("", "b", "n", "ng", "j", "q", "x", "zh"):
        for ending in ("y", "yn", "yon", "ye", "yen", "yet", "yek"):
            assert not any(output.startswith(f"{initial}yu") for output in normalize(f"{initial}{ending}", rules))
    assert not any(output == "yung" for output in normalize("y" + "ng", rules))
    algebra = compile_algebra({"yu", "yun", "yung", "yuon", "yue", "nyu"}, rules)
    for source, target in (("y", "yu"), ("yn", "yun"), ("yon", "yuon"), ("ye", "yue"), ("ny", "nyu")):
        assert f"    - derive/^G{target}$/F{source}/" not in algebra
