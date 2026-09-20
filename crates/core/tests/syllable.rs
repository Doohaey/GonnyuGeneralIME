use gannyu_input_core::{FuzzyMap, PriorityTier, SyllableScheme};
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn fuzzy_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/fuzzy_scheme.tsv"
    ))
}

#[test]
fn fuzzy_map_loads() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    assert!(map.iter().count() >= 6);
}

#[test]
fn fuzzy_map_scopes_regional_rules() {
    let mut file = NamedTempFile::new().expect("temporary fuzzy map");
    writeln!(
        file,
        "region\tcategory\tgon_fuzzy\tgon_pin\tapplies\tbidirectional\tchainable\tpriority_tier\tstarts_with"
    )
    .expect("write header");
    writeln!(
        file,
        "common\tonset\tzz\tz\tsyllable-initial\tfalse\tfalse\tprimary\t"
    )
    .expect("write common rule");
    writeln!(
        file,
        "lancong\tonset\tll\tl\tsyllable-initial\tfalse\tfalse\tprimary\t"
    )
    .expect("write lancong rule");
    writeln!(
        file,
        "fenni\tonset\tff\tf\tsyllable-initial\tfalse\tfalse\tprimary\t"
    )
    .expect("write fenni rule");

    let common = FuzzyMap::load_tsv(file.path()).expect("load common rules");
    assert_eq!(
        common
            .iter()
            .map(|rule| rule.region.as_str())
            .collect::<Vec<_>>(),
        ["common"]
    );
    assert!(common
        .normalize("zza", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "za"));
    assert!(!common
        .normalize("lla", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "la"));

    let lancong =
        FuzzyMap::load_tsv_for_region(file.path(), "lancong").expect("load lancong rules");
    assert_eq!(
        lancong
            .iter()
            .map(|rule| rule.region.as_str())
            .collect::<Vec<_>>(),
        ["common", "lancong"]
    );
    assert!(lancong
        .normalize("lla", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "la"));
    assert!(!lancong
        .normalize("ffa", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "fa"));
}

#[test]
fn gon_fuzzy_to_gon_pin_initial_i_to_y() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("ia", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "ia"));
    assert!(outputs.iter().any(|item| item.text == "ya"));
}

#[test]
fn ao_au_normalization_is_bidirectional() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let to_pin = map.normalize("pao", SyllableScheme::GonPin);
    assert!(to_pin.iter().any(|item| item.text == "pau"));
    let to_fuzzy = map.normalize("pau", SyllableScheme::GonFuzzy);
    assert!(to_fuzzy.iter().any(|item| item.text == "pao"));
}

#[test]
fn tone_digit_is_stripped_and_reattached_via_metadata() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("guon3", SyllableScheme::GonPin);
    assert!(outputs
        .iter()
        .all(|item| item.tone == Some(3) && !item.text.ends_with('3')));
}

#[test]
fn primary_tier_appears_before_fallback() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("ya", SyllableScheme::GonPin);
    let first = outputs.first().expect("at least one output");
    assert_eq!(first.tier, PriorityTier::Primary);
}

#[test]
fn entering_tone_coda_is_expanded_from_bare() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("ka", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "kat"));
    assert!(outputs.iter().any(|item| item.text == "kak"));
}

#[test]
fn updated_on_family_rules_normalize_to_short_forms() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("ioin", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "on"));
    let outputs = map.normalize("uoin", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "won"));
    let outputs = map.normalize("uen", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "won"));
}

#[test]
fn yu_normalizes_to_yu() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    assert!(map
        .normalize("yu", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "yu"));
}

#[test]
fn yu_accepts_u_and_v_as_fuzzy_inputs() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    assert!(map
        .normalize("u", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "yu"));
    assert!(map
        .normalize("v", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "yu"));
    assert!(!map
        .normalize("yu", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "yuyu"));
}

#[test]
fn checked_k_and_t_fuzz_mutually() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let mut outputs = map.normalize("nik", SyllableScheme::GonPin);
    outputs.extend(map.normalize("nik", SyllableScheme::GonFuzzy));
    // t↔k 入声尾互相模糊保留；h/p 不是入声尾
    assert!(outputs.iter().any(|item| item.text == "nit"));
    assert!(!outputs.iter().any(|item| item.text == "nih"));
    assert!(!outputs.iter().any(|item| item.text == "nip"));
    let mut t_outputs = map.normalize("nit", SyllableScheme::GonPin);
    t_outputs.extend(map.normalize("nit", SyllableScheme::GonFuzzy));
    assert!(t_outputs.iter().any(|item| item.text == "nik"));
    // 无尾输入单向回退到 t/k 带尾形式
    let mut bare_outputs = map.normalize("ni", SyllableScheme::GonPin);
    bare_outputs.extend(map.normalize("ni", SyllableScheme::GonFuzzy));
    assert!(bare_outputs.iter().any(|item| item.text == "nit"));
    assert!(bare_outputs.iter().any(|item| item.text == "nik"));
}

#[test]
fn zero_onset_ion_stops_at_yon() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let outputs = map.normalize("ion", SyllableScheme::GonPin);
    assert!(outputs.iter().any(|item| item.text == "yon"));
    assert!(!outputs.iter().any(|item| item.text == "yuon"));
}

#[test]
fn gkng_eu_rule_is_prefix_scoped() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    assert!(map
        .normalize("keu", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "kieu"));
    assert!(!map
        .normalize("leu", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "lieu"));
}

#[test]
fn yuo_family_accepts_all_supported_mandarin_style_spellings() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    let variants = ["yue", "ue", "ve", "ye"];

    for onset in ["", "j", "n", "q", "x"] {
        let expected = format!("{onset}yuon");
        for variant in variants {
            let input = format!("{onset}{variant}n");
            assert!(
                map.normalize(&input, SyllableScheme::GonPin)
                    .iter()
                    .any(|item| item.text == expected),
                "{input} should normalize to {expected}"
            );
        }
    }

    for onset in ["", "j", "l", "n", "q", "x"] {
        let expected = format!("{onset}yuot");
        let invalid = format!("{onset}yuok");
        for variant in variants {
            for coda in ["", "t", "k"] {
                let input = format!("{onset}{variant}{coda}");
                let outputs = map.normalize(&input, SyllableScheme::GonPin);
                assert!(
                    outputs.iter().any(|item| item.text == expected),
                    "{input} should normalize to {expected}"
                );
                assert!(
                    outputs.iter().all(|item| item.text != invalid),
                    "{input} should not generate {invalid}"
                );
            }
        }
    }
}

#[test]
fn added_theoretical_spellings_normalize_to_stored_forms() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");
    for (input, expected) in [
        ("hieu", "heu"),
        ("fi", "fei"),
        ("zuon", "zon"),
        ("cuon", "con"),
        ("ciu", "ceu"),
    ] {
        assert!(
            map.normalize(input, SyllableScheme::GonPin)
                .iter()
                .any(|item| item.text == expected),
            "{input} should normalize to {expected}"
        );
    }
}

#[test]
fn mandarin_ao_and_ou_inputs_normalize_to_au_and_eu() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy_map should load");

    for onset in [
        "b", "c", "d", "g", "h", "k", "l", "m", "ng", "p", "s", "t", "z",
    ] {
        let input = format!("{onset}ao");
        let expected = format!("{onset}au");
        assert!(
            map.normalize(&input, SyllableScheme::GonPin)
                .iter()
                .any(|item| item.text == expected),
            "{input} should normalize to {expected}"
        );
    }
    assert!(map
        .normalize("niao", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "niau"));

    for onset in [
        "c", "d", "f", "g", "h", "j", "k", "l", "m", "ng", "p", "s", "t", "y", "z",
    ] {
        let input = format!("{onset}ou");
        let expected = format!("{onset}eu");
        assert!(
            map.normalize(&input, SyllableScheme::GonPin)
                .iter()
                .any(|item| item.text == expected),
            "{input} should normalize to {expected}"
        );
    }
    assert!(!map
        .normalize("chou", SyllableScheme::GonPin)
        .iter()
        .any(|item| item.text == "cheu"));
}
