import hashlib
import json
import re
from pathlib import Path

from platforms.rime.build import (
    Entry,
    annotated_reading,
    active_regions,
    build,
    build_metadata,
    build_synonym_relations,
    build_single_character_frequencies,
    build_new_old,
    build_paired_readings,
    entry_codes,
    build_preferred_readings,
    load_entries,
    selected_entries,
    syllables,
    strip_tone,
    write_resource_manifest,
)
from platforms.rime.fuzzy import compile_algebra, essential_rules, load_rules, normalize


RULES_PATH = Path(__file__).resolve().parents[1] / "resources" / "fuzzy_scheme.tsv"


def test_selected_modes_use_source_pairs_in_words_and_keep_only_selected_annotations() -> None:
    _, source = load_entries("lancong")
    source = [entry for entry in source if entry.word in {"义", "主义"}]
    for mode, expected, excluded, label in (("new", "zu3 yi5", "老", "新"), ("old", "zu3 ni5", "新", "老")):
        entries = selected_entries(source, mode, True)
        assert next(entry.gan for entry in entries if entry.word == "主义") == expected
        assert all(not entry.new_old.startswith(excluded) for entry in entries)
        annotations, _, _ = build_metadata(entries, mode)
        assert f"[{label}]" in annotations["义"]
        assert f"[{excluded}]" not in annotations["义"]


def test_disabled_relations_remove_source_mandarin_entries_and_references() -> None:
    _, source = load_entries("lancong")
    source = [entry for entry in source if entry.word in {"䁐牛", "放牛"}]
    entries = selected_entries(source, "both", False)
    assert [entry.word for entry in entries] == ["䁐牛"]
    annotations, before, after = build_metadata(entries)
    assert "放牛" not in annotations["䁐牛"]
    assert not before and not after


def test_special_vowel_inputs_survive_without_optional_rules() -> None:
    rules = essential_rules(load_rules(RULES_PATH, "lancong"))
    assert {(rule.source, rule.target) for rule in rules} == {
        ("ir", "e"), ("er", "e"), ("e", "ê"), ("ee", "ê"), ("i", "ï"), ("ii", "ï"),
        ("o", "ô"), ("oo", "ô"),
    }
    _, entries = load_entries("lancong")
    special = next(strip_tone(entry.gan) for entry in entries if entry.word == "且" and entry.new_old.startswith("新"))
    assert special in normalize(special.replace("ê", "ee"), rules)
    plain = next(strip_tone(part) for entry in entries for part in syllables(entry.gan) if "e" in part and "ê" not in part)
    for alias in ("ir", "er"):
        forms = normalize(plain.replace("e", alias), rules)
        assert plain in forms
        assert plain.replace("e", "ê") not in forms


def test_synonyms_are_separate_from_gan_mandarin_relations_and_characters_survive() -> None:
    _, source = load_entries("lancong")
    source = [entry for entry in source if entry.word in {"几多", "多少", "䁐牛", "放牛", "这"}]
    for relations in (True, False):
        entries = selected_entries(source, "both", relations)
        annotations, before, after = build_metadata(entries, include_synonyms=False)
        synonyms = build_synonym_relations(entries)
        assert synonyms["几多"] == ["多少"]
        assert synonyms["多少"] == ["几多"]
        assert "[联]" not in annotations["几多"]
        assert "多少" not in after.get("几多", [])
        assert "这" in annotations
        assert ("[不习用]" in annotations["这"]) == relations
        assert ("放牛" in annotations) == relations
        if relations:
            assert after["䁐牛"] == ["放牛"]
        else:
            assert "放牛" not in annotations["䁐牛"]
            assert not before and not after


def test_wenbai_lookup_codes_are_separate_from_subtitle_metadata() -> None:
    _, source = load_entries("lancong")
    entries = [entry for entry in source if entry.word in {"经", "经济", "人", "人口"}]
    pairs = build_paired_readings(entries)
    economy = next(entry for entry in entries if entry.word == "经济")
    population = next(entry for entry in entries if entry.word == "人口")
    assert "Gjin Gji" in entry_codes(economy, pairs)
    assert "Wjiang Gji" in entry_codes(economy, pairs)
    assert "Gnin Gkiêu" in entry_codes(population, pairs)
    assert "Wlen Gkiêu" in entry_codes(population, pairs)
    annotations, _, _ = build_metadata(entries)
    assert "[文]" in annotations["经"] and "[白]" in annotations["经"]


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
        bare_iu = normalize("iu", rules)
        iu_expected = {"iu" + coda for coda in codas}
        assert {"iup", "iut", "iuk"}.intersection(bare_iu) == iu_expected
        assert not {"yup", "yut", "yuk", "yiup", "yiut", "yiuk"}.intersection(bare_iu)
        iu_algebra = compile_algebra(iu_expected, rules)
        for target in iu_expected:
            assert any(line.endswith(f"/^G{target}$/Fiu/") for line in iu_algebra)
        algebra = compile_algebra(expected, rules)
        for coda in codas:
            spelling = "bae" + coda
            forms = set(normalize(spelling, rules)) | set(normalize(spelling, rules, reverse=True))
            assert expected.issubset(forms)
            assert not {spelling + tail for tail in "ptk"}.intersection(forms)
            assert any(line.endswith(f"/^G{spelling}$/Fbae/") for line in algebra)


def test_ui_shortening_preserves_onset_ui_and_legacy_w_forms() -> None:
    rules = load_rules(RULES_PATH)
    assert "wi" not in normalize("ui", rules)
    for spelling in ("wui", "wei"):
        assert "wi" in normalize(spelling, rules)
    assert "wui" not in normalize("wi", rules)
    assert "gui" in normalize("guei", rules)
    assert "guêi" in normalize("gueei", rules)
    for spelling in ("uei", "uêi"):
        assert "wêi" not in normalize(spelling, rules)
    assert "gwi" not in normalize("gui", rules)
    algebra = compile_algebra({"wi", "gui"}, rules)
    for spelling in ("wui", "wei"):
        assert f"    - derive/^Gwi$/F{spelling}/" in algebra
    assert "    - derive/^Ggui$/Fguei/" in algebra


def test_ueik_rules_follow_uei_with_checked_coda() -> None:
    rules = load_rules(RULES_PATH)
    source, target = "uei", "ui"
    plain = [rule for rule in rules if rule.source == source and rule.target == target]
    checked = [rule for rule in rules if rule.source == source + "k" and rule.target == target + "k"]
    assert len(plain) == len(checked) == 1
    plain, checked = plain[0], checked[0]
    assert checked.applies == plain.applies
    assert checked.bidirectional == plain.bidirectional
    assert checked.tier == plain.tier
    assert plain.starts_with == checked.starts_with
    assert "guik" in normalize("gueik", rules)
    assert "wêi" not in normalize("uêi", rules)
    assert "wêik" not in normalize("uêik", rules)


def test_ueik_shortened_forms_compile_to_stored_codes() -> None:
    rules = load_rules(RULES_PATH)
    assert {"weik", "wêik"}.isdisjoint(normalize("ueik", rules))
    assert "wêik" not in normalize("uêik", rules)
    assert "weik" not in normalize("wêik", rules)
    for spelling in ("ueik", "uêik", "weik", "wêik"):
        assert "uik" not in normalize(spelling, rules)
    for initial in ("g", "n", "ng", "zh"):
        assert initial + "uik" in normalize(initial + "ueik", rules)
    algebra = compile_algebra({"weik", "wêik", "guik"}, rules)
    assert "    - derive/^Gweik$/Fueik/" not in algebra
    assert "    - derive/^Gwêik$/Fueik/" not in algebra
    assert "    - derive/^Gwêik$/Fuêik/" not in algebra
    assert "    - derive/^Gguik$/Fgueik/" in algebra


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
    for region in active_regions():
        rules = load_rules(RULES_PATH, region)
        codas = {rule.target for rule in rules
                 if not rule.source and rule.target in {"p", "t", "k"}}
        assert "eo" in normalize("o6", rules)
        for source, target in (("e", "eo"), ("go", "geo"), ("oo", "oeo"), ("ot", "eot"), ("et", "eot"), ("got", "geot"), ("get", "geot"), ("gop", "geop"), ("gep", "geop"), ("gok", "geok"), ("gek", "geok"), ("yue", "yueo"), ("yuek", "yueok")):
            assert target not in normalize(source, rules), (region, source, target)
        for reverse in (False, True):
            forms = normalize("eo", rules, reverse=reverse)
            assert "o" not in forms and "e" not in forms
        dictionary = RULES_PATH.parent / "regions" / region / "dictionaries" / "chars.tsv"
        yuo_readings = {reading.rstrip("01234567*")
                        for line in dictionary.read_text(encoding="utf-8").splitlines()[1:]
                        for reading in line.split("\t")[2].split() if "yuo" in reading}
        for source in yuo_readings:
            stem = source[:-1] if source[-1:] in {"p", "t", "k"} else source
            open_rime = source
            for ending in ("ng", "n", "p", "t", "k"):
                if source.endswith("yuo" + ending):
                    open_rime = source[:-len(ending)]
                    break
            if open_rime.endswith("yuo"):
                for reverse in (False, True):
                    forms = normalize(open_rime, rules, reverse=reverse)
                    assert open_rime + "n" not in forms, (region, open_rime, reverse)
                    assert open_rime + "ng" not in forms, (region, open_rime, reverse)
            if source[-1:] in codas:
                assert source in normalize(stem, rules), (region, stem, source)
                for coda in codas:
                    alias = stem + coda
                    assert source in normalize(alias, rules), (region, alias, source)
        if region == "yikyan-henfeng":
            assert {"yuon", "yuong"} <= yuo_readings
            assert "yuong" in normalize("yuon", rules)
        algebra = compile_algebra({"eo", "geot", "yueok"}, rules)
        assert "    - derive/^Gyueok$/Fyuok/" not in algebra
        assert "    - derive/^Geo$/Fo/" in algebra
        for target, source in (("eo", "e"), ("geot", "got"), ("geot", "get"),
                               ("yueok", "yuek")):
            assert f"    - derive/^G{target}$/F{source}/" not in algebra


def test_ir_and_er_match_plain_e_and_double_e_matches_circumflex_one_way() -> None:
    rules = load_rules(RULES_PATH)
    for alias in ("ir", "er"):
        rule = next(rule for rule in rules if rule.source == alias and rule.target == "e")
        assert rule.chainable
    for plain, circumflex in (("ir", "ee"), ("irn", "een"), ("gen", "geen"),
                              ("er", "ee"), ("ern", "een"), ("gen", "geen")):
        assert plain in normalize(plain, rules)
        assert circumflex in normalize(circumflex, rules)
        assert "ê" not in normalize(plain, rules)
        assert plain not in normalize(circumflex, rules, reverse=True)
    algebra = compile_algebra({"e", "ê", "gêk"}, rules)
    assert "    - derive/^Ge$/Fir/" in algebra
    assert "    - derive/^Ge$/Fer/" in algebra
    assert "    - derive/^Ggêk$/Fgeek/" in algebra
    assert "    - derive/^Gê$/Fee/" in algebra
    assert "    - derive/^Ge$/Fê/" not in algebra


def test_open_e_compatibility_can_chain_without_chaining_double_e() -> None:
    rules = load_rules(RULES_PATH, "lancong")
    assert "guê" in normalize("guee", rules)
    assert "ê" in normalize("ee", rules)
    assert "gêk" in normalize("geek", rules)


def test_voiced_initial_compatibility_composes_with_special_vowels():
    rules = load_rules(RULES_PATH, "singzi")
    for source, target in (("jieen", "jjiên"), ("zi", "zzï")):
        assert target in normalize(source, rules)
    assert not any(form.startswith("jjj") for form in normalize("jjiên", rules))
    algebra = compile_algebra({"jjiên", "zzï"}, rules)
    assert "    - derive/^Gjjiên$/Fjieen/" in algebra
    assert "    - derive/^Gzzï$/Fzi/" in algebra
    assert "    - derive/^Gjjiên$/Fjjjien/" not in algebra


def test_ir_and_double_e_inputs_select_plain_and_circumflex_e() -> None:
    rules = load_rules(RULES_PATH)
    for plain_input, hat_input, plain, circumflex in (
        ("ir", "ee", "e", "ê"),
        ("irn", "een", "en", "ên"),
        ("girn", "geen", "gen", "gên"),
        ("bir", "beek", "be", "bêk"),
        ("er", "ee", "e", "ê"),
        ("ern", "een", "en", "ên"),
        ("gern", "geen", "gen", "gên"),
        ("ber", "beek", "be", "bêk"),
    ):
        assert plain in normalize(plain_input, rules)
        assert circumflex not in normalize(plain_input, rules)
        assert circumflex in normalize(hat_input, rules)
        assert plain not in normalize(hat_input, rules)
        for reverse in (False, True):
            assert plain_input.rstrip("1") not in normalize(plain, rules, reverse=reverse)
            assert hat_input.rstrip("1") not in normalize(circumflex, rules, reverse=reverse)
    algebra = compile_algebra({"e", "ê", "gêk", "gek"}, rules)
    assert "    - derive/^Gê$/Fee/" in algebra
    assert "    - derive/^Ggêk$/Fgeek/" in algebra
    assert "    - derive/^Ge$/Fir/" in algebra
    assert "    - derive/^Ge$/Fer/" in algebra
    assert "    - derive/^Ggek$/Fgirk/" in algebra
    assert "    - derive/^Ggek$/Fgerk/" in algebra


def test_plain_e_combinations_are_common_terminal_and_preserve_tiers() -> None:
    rules = load_rules(RULES_PATH)
    additions = [rule for rule in rules if "ir" in rule.source and rule.source != "ir" or
                 rule.source == "ir" and rule.target not in {"e", "ê"}]
    for rule in additions:
        assert rule.region == "common"
        assert not rule.bidirectional and not rule.chainable
        for source in rule.starts_with:
            assert source.endswith(rule.source)
            target = source[:-len(rule.source)] + rule.target
            assert normalize(source, rules)[target] == {"primary": 0, "secondary": 1, "fallback": 2}[rule.tier]
            assert not any(part in source.replace("ir", "e") or part in target
                           for part in ("ae", "eo", "oe", "ie"))
            assert target.replace("e", "ê") not in normalize(source, rules)


def test_yun_to_yung_compatibility_is_yikyan_only_and_one_way() -> None:
    regions = [None, "fenni", "fungcen", "jingon", "jisuibaedu", "lancong",
               "seusong", "sinyi", "songau", "tiqien", "xyucuenn", "yikyan-henfeng"]
    for region in regions:
        rules = load_rules(RULES_PATH, region)
        for source, target in (("yun2", "yung"), ("jyun1", "jyung"),
                               ("qyun5", "qyung"), ("xyun1", "xyung")):
            assert (target in normalize(source, rules)) == (region == "yikyan-henfeng")
            assert source[:-1] not in normalize(target, rules)
            assert source[:-1] not in normalize(target, rules, reverse=True)


def test_zero_initial_iu_input_stays_plain() -> None:
    rules = load_rules(RULES_PATH)
    assert "yiu" not in normalize("iu", rules)
    for spelling in ("you", "yiu"):
        forms = normalize(spelling, rules)
        assert "yiu" in forms
        assert "yu" not in forms
    assert "yiu" not in normalize("yu", rules)
    algebra = compile_algebra({"yiu", "yu"}, rules)
    assert "    - derive/^Gyiu$/Fiu/" not in algebra
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


def test_norel_schemas_reuse_rel_dictionaries_and_keep_their_data(tmp_path: Path) -> None:
    build("jingon", tmp_path)

    for mode in ("both", "new", "old"):
        rel_name = "gannyu_jingon" if mode == "both" else f"gannyu_jingon_{mode}_rel"
        norel_name = f"gannyu_jingon_{mode}_norel"
        schema = (tmp_path / f"{norel_name}.schema.yaml").read_text(encoding="utf-8")
        assert f"schema_id: {norel_name}\n" in schema
        assert f"dictionary: {rel_name}\n" in schema
        assert f"prism: {rel_name}" in schema
        assert "  dictionary_exclude:\n" in schema
        assert f"user_dict: {norel_name}\n" in schema
        assert not (tmp_path / f"{norel_name}.dict.yaml").exists()
        assert (tmp_path / "lua" / f"{norel_name}_data.lua").is_file()
        assert (tmp_path / "lua" / f"{norel_name}_annotations.bin").is_file()

    _, source = load_entries("jingon")
    for mode in ("both", "new", "old"):
        rel = source if mode == "both" else selected_entries(source, mode, True)
        norel = selected_entries(source, mode, False)
        removed = {entry.word for entry in rel} - {entry.word for entry in norel}
        data = (tmp_path / "lua" / f"gannyu_jingon_{mode}_norel_data.lua").read_text(encoding="utf-8")
        forbidden = set(re.findall(r'\["((?:[^"\\]|\\.)*)"\] = true', data))
        assert removed <= forbidden
        schema = (tmp_path / f"gannyu_jingon_{mode}_norel.schema.yaml").read_text(encoding="utf-8")
        excluded = {
            json.loads(item)
            for item in re.findall(r'^    - ("(?:[^"\\]|\\.)*")$', schema, re.MULTILINE)
        }
        assert excluded == removed


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
    assert (tmp_path / "LICENSE").read_bytes() == (Path(__file__).resolve().parents[1] / "LICENSE").read_bytes()
    assert any(file["path"] == "LICENSE" for file in manifest["files"])
    reader = (tmp_path / "lua/gannyu_annotation_store.lua").read_bytes()
    source = Path(__file__).resolve().parents[1] / "platforms/rime/gannyu_annotation_store.lua"
    assert reader == source.read_bytes()
    record = next(file for file in manifest["files"] if file["path"] == "lua/gannyu_annotation_store.lua")
    assert record["size"] == len(reader)
    assert record["sha256"] == hashlib.sha256(reader).hexdigest()


def test_rime_package_manifest_only_lists_packaged_files(tmp_path: Path) -> None:
    for name in ("default.yaml", "default.custom.yaml", "LICENSE"):
        (tmp_path / name).write_text(name, encoding="utf-8")
    files = ("default.custom.yaml", "resource-manifest.json", "LICENSE")

    write_resource_manifest(tmp_path, ("goungon",), files)

    manifest = json.loads((tmp_path / "resource-manifest.json").read_text(encoding="utf-8"))
    assert {record["path"] for record in manifest["files"]} == set(files) - {"resource-manifest.json"}
    for record in manifest["files"]:
        content = (tmp_path / record["path"]).read_bytes()
        assert record["size"] == len(content)
        assert record["sha256"] == hashlib.sha256(content).hexdigest()


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
    assert "ion" in normalize("ion", rules)
    assert "yon" not in normalize("ion", rules)
    assert "yuon" not in normalize("ion", rules)


def test_yuo_family_accepts_all_supported_mandarin_style_spellings() -> None:
    rules = load_rules(RULES_PATH)
    variants = ("yue", "ue", "ve")

    for onset in ("", "j", "n", "q", "x"):
        expected = f"{onset}yuon"
        for variant in variants:
            if not onset and variant != "yue":
                continue
            input_syllable = f"{onset}{variant}n"
            assert expected in normalize(input_syllable, rules)

    for onset in ("", "j", "l", "n", "q", "x"):
        expected = f"{onset}yuot"
        invalid = f"{onset}yuok"
        for variant in variants:
            if not onset and variant != "yue":
                continue
            for coda in ("", "t", "k"):
                input_syllable = f"{onset}{variant}{coda}"
                outputs = normalize(input_syllable, rules)
                assert expected in outputs
                assert invalid not in outputs


def test_circumflex_targets_preserve_dictionary_spellings() -> None:
    rules = load_rules(RULES_PATH)

    for source, target, removed in (("yan", "yên", None), ("ngen", "ngiên", "ngien"), ("nget", "ngiêt", "ngiet"), ("nge", "ngiêt", "ngiet")):
        outputs = normalize(source, rules)
        assert target in outputs
        if removed is not None:
            assert removed not in outputs

    for source, target in (("ien", "yên"), ("iep", "yêp"), ("iet", "yêt"),
                           ("iek", "yêk"), ("ieu", "yêu")):
        assert source in normalize(source, rules)
        assert target not in normalize(source, rules)

    yan_outputs = normalize("yan", rules)
    assert "yên" in yan_outputs
    assert "yen" not in yan_outputs


def test_added_theoretical_spellings_normalize_to_stored_forms() -> None:
    rules = load_rules(RULES_PATH)

    for input_syllable, expected in {
        "hieu": "hêu",
        "yao": "yêu",
        "hou": "hêu",
        "fi": "fêi",
        "zuon": "zon",
        "cuon": "con",
        "ciu": "ceu",
    }.items():
        assert expected in normalize(input_syllable, rules)

    for input_syllable, forbidden in {
        "iau": "yêu",
        "uet": "wêt",
        "uek": "wêk",
        "ueu": "wêu",
    }.items():
        assert forbidden not in normalize(input_syllable, rules)


def test_u_v_to_yu_bridges_require_an_onset() -> None:
    rules = load_rules(RULES_PATH)
    for source, forbidden in (("ue", "yue"), ("ve", "yue"), ("uir", "yuek"), ("vir", "yuek")):
        assert forbidden not in normalize(source, rules)
    for source, expected in (("jue", "jyue"), ("jve", "jyue"), ("juir", "jyuek"), ("jvir", "jyuek")):
        assert expected in normalize(source, rules)
    assert "heu" in normalize("hieu", rules)


def test_xyucuenn_zero_onset_uen_does_not_match_wenn() -> None:
    rules = load_rules(RULES_PATH, "xyucuenn")
    assert "wênn" not in normalize("uen", rules)
    assert "uen" in normalize("uen", rules)


def test_fi_to_fui_is_one_way_and_lancong_only() -> None:
    for region in active_regions():
        rules = load_rules(RULES_PATH, region)
        for source in ("fi", "fi1", "fi3", "fi4", "fi5"):
            assert ("fui" in normalize(source, rules)) == (region == "lancong")
            assert "fêi" in normalize(source, rules)
            assert "fei" not in normalize(source, rules)
        for reverse in (False, True):
            assert "fi" not in normalize("fui", rules, reverse=reverse)
        for source in ("afi", "lfi", "fian", "fin", "fip", "fit", "fik"):
            assert source.replace("fi", "fui") not in normalize(source, rules)
        algebra = compile_algebra({"fui"}, rules)
        assert ("    - derive/^Gfui$/Ffi/" in algebra) == (region == "lancong")


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


def test_onset_contractions_preserve_onset_forms() -> None:
    rules = load_rules(RULES_PATH)
    for source, target in (("guei", "gui"), ("guen", "gun"), ("vuei", "vui")):
        assert target in normalize(source, rules)
    for source in ("uen", "uên"):
        assert "wên" not in normalize(source, rules)
    for source in ("wen", "wên"):
        assert "wên" in normalize(source, rules)
    for source, forbidden in (("uêi", "wi"), ("uen", "un"), ("uên", "un")):
        assert forbidden not in normalize(source, rules)


def test_zero_initial_iu_suffixes_stay_plain() -> None:
    rules = load_rules(RULES_PATH)
    for source, legacy, target in (("iung", "yiung", "yung"), ("iun", "yiun", "yun"), ("iuek", "yiuek", "yuek"), ("iuok", "yiuok", "yuok")):
        assert source in normalize(source, rules)
        assert target not in normalize(source, rules)
        assert target not in normalize(legacy, rules)
    assert "yiu" not in normalize("iu", rules)
    assert "yiuk" not in normalize("iuk", rules)
    for source in ("iu", "iuk", "iut"):
        assert not {"yu", "yup", "yut", "yuk"}.intersection(normalize(source, rules))
    algebra = compile_algebra({"yung", "yiu", "yu"}, rules)
    assert "    - derive/^Gyung$/Fiung/" not in algebra
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


def test_n_l_hong_fuzzy_rules_are_scoped_to_registered_prefixes() -> None:
    cases = {
        "lancong": (
            ["nan", "nong", "nu", "nêt", "nêu"],
            ["ni", "nien", "nyu", "n"],
        ),
        "fenni": (
            ["nai", "nak", "nan", "nang", "nau", "nei", "nen", "neu", "no", "nu"],
            ["ni", "nien", "nyu", "na"],
        ),
        "fungcen": (
            ["naek", "nai", "nan", "nau", "nok", "non", "nu", "nên"],
            ["ni", "nien", "nyu", "na"],
        ),
        "jisuibaedu": (
            ["nae", "nai", "nan", "nau", "no", "nu", "nên"],
            ["ni", "nien", "nyu", "na"],
        ),
        "sinyi": (
            ["na", "nai", "naik", "nan", "nat", "nau", "no", "nok", "nong", "nu", "nui", "nuk", "nun", "nung", "nuoi", "nuon", "nên", "nêu"],
            ["ni", "nien", "nyu", "nï"],
        ),
        "songau": (
            ["na", "naen", "nai", "nan", "nat", "nau", "no", "noi", "nok", "non", "not", "nu", "nuk", "nung"],
            ["ni", "nien", "nyu"],
        ),
    }
    for region, (positives, negatives) in cases.items():
        rules = load_rules(RULES_PATH, region)
        for spelling in positives:
            assert any(
                output.startswith("l") and output != spelling
                for output in normalize(spelling, rules)
            ), (region, spelling)
        for spelling in negatives:
            assert not any(
                output.startswith("l") for output in normalize(spelling, rules)
            ), (region, spelling)


def test_fungcen_n_l_merger_rule_has_no_n_duplicates() -> None:
    chars = (
        RULES_PATH.parent / "regions/fungcen/dictionaries/chars.tsv"
    ).read_text(encoding="utf-8").splitlines()[1:]
    removed = {
        ("乳", "nu3"),
        ("南", "non2"),
        ("男", "non2"),
        ("奶", "nai1"),
        ("奶", "nai3"),
        ("挠", "nau2"),
        ("耐", "nai4"),
        ("诺", "nok6"),
        ("弱", "nok6"),
        ("熔", "nung2"),
        ("难", "nan2"),
    }
    for line in chars:
        fields = line.split("\t")
        if len(fields) >= 3:
            assert (fields[0], fields[2]) not in removed
