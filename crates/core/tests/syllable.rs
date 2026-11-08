use gannyu_input_core::{FuzzyMap, SyllableScheme};
use std::path::PathBuf;

const LANCONG_SOURCES: &[&str] = &[
    include_str!("../../../resources/regions/lancong/dictionaries/chars.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/words.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/gan_chars.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/gan_words.tsv"),
];
const YIKYAN_SOURCES: &[&str] = &[
    include_str!("../../../resources/regions/yikyan-henfeng/dictionaries/chars.tsv"),
    include_str!("../../../resources/regions/yikyan-henfeng/dictionaries/words.tsv"),
    include_str!("../../../resources/regions/yikyan-henfeng/dictionaries/gan_chars.tsv"),
    include_str!("../../../resources/regions/yikyan-henfeng/dictionaries/gan_words.tsv"),
];

fn fuzzy_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/fuzzy_scheme.tsv"
    ))
}

fn source_syllables(sources: &[&str], word: &str) -> Vec<String> {
    sources
        .iter()
        .flat_map(|source| source.lines().skip(1))
        .filter_map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            (fields.first() == Some(&word)).then(|| fields[2].split_whitespace().map(str::to_owned))
        })
        .flatten()
        .collect()
}

fn assert_dictionary_spellings_are_preserved(region: &str, syllables: &[String]) {
    let map = FuzzyMap::load_tsv_for_region(fuzzy_path(), region).expect("load regional rules");
    for syllable in syllables {
        let untone = syllable
            .trim_end_matches(|character: char| character.is_ascii_digit() || character == '*');
        assert!(
            map.normalize(syllable, SyllableScheme::GonPin)
                .iter()
                .any(|result| result.text == untone),
            "{region}: {syllable}"
        );
    }
}

#[test]
fn fuzzy_map_loads() {
    let map = FuzzyMap::load_tsv(fuzzy_path()).expect("fuzzy map should load");
    assert!(map.iter().count() >= 6);
}

#[test]
fn lancong_source_dictionary_spellings_are_preserved() {
    // Sources: Lancong chars.tsv (明、渠) and words.tsv (车马、明白).
    for word in ["明", "渠", "车马", "明白"] {
        let syllables = source_syllables(LANCONG_SOURCES, word);
        assert!(
            !syllables.is_empty(),
            "{word} must be present in the source dictionary"
        );
        assert_dictionary_spellings_are_preserved("lancong", &syllables);
    }
}

#[test]
fn yikyan_source_dictionary_spellings_are_preserved() {
    // Sources: Yikyan-Henfeng chars.tsv (明、渠) and words.tsv (明天).
    for word in ["明", "渠", "明天"] {
        let syllables = source_syllables(YIKYAN_SOURCES, word);
        assert!(
            !syllables.is_empty(),
            "{word} must be present in the source dictionary"
        );
        assert_dictionary_spellings_are_preserved("yikyan-henfeng", &syllables);
    }
}

#[test]
fn n_l_hong_fuzzy_rules_are_scoped_to_registered_prefixes() {
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("lancong", &["nan", "nong", "nu", "nêt", "nêu"], &["ni", "nien", "nyu", "n"]),
        (
            "fenni",
            &["nai", "nak", "nan", "nang", "nau", "nei", "nen", "neu", "no", "nu"],
            &["ni", "nien", "nyu", "na"],
        ),
        (
            "fungcen",
            &["naek", "nai", "nan", "nau", "nok", "non", "nu", "nên"],
            &["ni", "nien", "nyu", "na"],
        ),
        (
            "jisuibaedu",
            &["nae", "nai", "nan", "nau", "no", "nu", "nên"],
            &["ni", "nien", "nyu", "na"],
        ),
        (
            "sinyi",
            &[
                "na", "nai", "naik", "nan", "nat", "nau", "no", "nok", "nong", "nu",
                "nui", "nuk", "nun", "nung", "nuoi", "nuon", "nên", "nêu",
            ],
            &["ni", "nien", "nyu", "nï"],
        ),
        (
            "songau",
            &[
                "na", "naen", "nai", "nan", "nat", "nau", "no", "noi", "nok", "non",
                "not", "nu", "nuk", "nung",
            ],
            &["ni", "nien", "nyu"],
        ),
        ("goungon", &["nai", "nen", "nin"], &["ni", "nian", "niong", "nong"]),
    ];
    for (region, positives, negatives) in cases {
        let map = FuzzyMap::load_tsv_for_region(fuzzy_path(), region).expect("load regional rules");
        for spelling in *positives {
            let forms = map.normalize(spelling, SyllableScheme::GonPin);
            assert!(
                forms
                    .iter()
                    .any(|form| form.text.starts_with('l') && form.text != *spelling),
                "{region}: {spelling}"
            );
        }
        for spelling in *negatives {
            let forms = map.normalize(spelling, SyllableScheme::GonPin);
            assert!(
                !forms.iter().any(|form| form.text.starts_with('l')),
                "{region}: {spelling}"
            );
        }
    }
}
