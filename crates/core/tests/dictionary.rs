use gannyu_input_core::{Dictionary, DictionaryError};
use std::fs;
use std::path::PathBuf;

const HEADER: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记";

const SOURCES: &[(&str, &str)] = &[
    (
        "chars.tsv",
        include_str!("../../../resources/regions/lancong/dictionaries/chars.tsv"),
    ),
    (
        "words.tsv",
        include_str!("../../../resources/regions/lancong/dictionaries/words.tsv"),
    ),
    (
        "gan_chars.tsv",
        include_str!("../../../resources/regions/lancong/dictionaries/gan_chars.tsv"),
    ),
    (
        "gan_words.tsv",
        include_str!("../../../resources/regions/lancong/dictionaries/gan_words.tsv"),
    ),
];

fn source_row(word: &str) -> Vec<String> {
    let rows: Vec<Vec<String>> = SOURCES
        .iter()
        .flat_map(|(origin, text)| {
            text.lines().skip(1).filter_map(move |line| {
                let fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
                let _ = origin;
                fields
                    .first()
                    .is_some_and(|value| value == word)
                    .then_some(fields)
            })
        })
        .collect();
    rows.iter()
        .find(|fields| {
            fields
                .get(5)
                .is_some_and(|equivalent| !equivalent.is_empty())
        })
        .or_else(|| rows.first())
        .cloned()
        .unwrap_or_else(|| panic!("{word} must come from the Lancong source dictionaries"))
}

fn write_fixture(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gannyu-dict-test-{name}"));
    fs::write(&path, content).expect("fixture write");
    path
}

fn row_text(fields: &[String]) -> String {
    fields.join("\t")
}

#[test]
fn loads_eight_column_rows_from_the_source_dictionary() {
    // Source: resources/regions/lancong/dictionaries/words.tsv, 车马.
    let fields = source_row("车马");
    let body = format!("{HEADER}\n{}\n", fields[..8].join("\t"));
    let path = write_fixture("eight-column.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    assert_eq!(dictionary.len(), 1);

    let entry = &dictionary.entries()[0];
    assert_eq!(entry.headword.as_ref(), "车马");
    assert_eq!(entry.ipa.as_ref(), "tsʰa1 ma1");
    assert_eq!(entry.dialect_pinyin.as_ref(), "ca1 ma1");
    assert_eq!(entry.mandarin_pinyin.as_ref(), "che1 ma3");
}

#[test]
fn skips_comments_and_blank_lines() {
    // Source: resources/regions/lancong/dictionaries/words.tsv, 车马.
    let fields = source_row("车马");
    let body = format!(
        "# comment\n\n{HEADER}\n{}\n\n# another\n",
        row_text(&fields)
    );
    let path = write_fixture("comments.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    assert_eq!(dictionary.len(), 1);
    assert_eq!(dictionary.entries()[0].headword.as_ref(), "车马");
}

#[test]
fn tolerates_column_reordering() {
    // Source: resources/regions/lancong/dictionaries/words.tsv, 车马.
    let fields = source_row("车马");
    let header = "词频\t本词\t方言拼音\t国际音标\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t同义词\t新旧标记";
    let reordered = [7, 0, 2, 1, 3, 4, 5, 6, 8, 9]
        .iter()
        .map(|index| fields[*index].as_str())
        .collect::<Vec<_>>()
        .join("\t");
    let path = write_fixture("reorder.tsv", &format!("{header}\n{reordered}\n"));
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let entry = &dictionary.entries()[0];
    assert_eq!(entry.headword.as_ref(), "车马");
    assert_eq!(entry.dialect_pinyin.as_ref(), "ca1 ma1");
}

#[test]
fn lookup_by_dialect_pinyin() {
    // Repeat the same source row so the fixture checks index multiplicity
    // without introducing a new lexical item or pronunciation.
    // Source: resources/regions/lancong/dictionaries/gan_chars.tsv, 明.
    let fields = source_row("明");
    let body = format!(
        "{HEADER}\n{}\n{}\n{}\n",
        row_text(&fields),
        row_text(&fields),
        row_text(&fields)
    );
    let path = write_fixture("lookup.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    assert_eq!(dictionary.by_dialect_pinyin("min4").len(), 3);
}

#[test]
fn normalizes_case_and_separator_variants_of_source_spellings() {
    // Source: resources/regions/lancong/dictionaries/gan_words.tsv, 唆奅.
    let mut source = source_row("唆奅");
    source[2] = "SO1'PAU1".into(); // case and separator normalization only
    let path = write_fixture(
        "normalized-lookup.tsv",
        &format!("{HEADER}\n{}\n", row_text(&source)),
    );
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");

    assert_eq!(dictionary.entries()[0].dialect_pinyin.as_ref(), "so1'pau1");
    assert_eq!(dictionary.by_dialect_pinyin("so'pau").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("so1pau1").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("sopau").len(), 1);
    assert_eq!(dictionary.by_mandarin_pinyin("suo1pao4").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_pinyin("chui1niu2").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_text("吹牛").len(), 1);
}

#[test]
fn expands_mandarin_word_indexes_from_source_equivalents() {
    // Source: resources/regions/lancong/dictionaries/gan_chars.tsv, 脱.
    let fields = source_row("脱");
    let path = write_fixture(
        "multi-mandarin.tsv",
        &format!("{HEADER}\n{}\n", row_text(&fields)),
    );
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");

    for equivalent in ["很", "极其"] {
        assert_eq!(dictionary.by_mandarin_word_text(equivalent).len(), 1);
    }
    for pinyin in ["hen3", "ji2qi2"] {
        assert_eq!(dictionary.by_mandarin_word_pinyin(pinyin).len(), 1);
    }
}

#[test]
fn runtime_cache_preserves_final_lookup_indexes() {
    // Sources: resources/regions/lancong/dictionaries/gan_chars.tsv, 脱;
    // resources/regions/lancong/dictionaries/words.tsv, 小菜.
    let first = source_row("脱");
    let second = source_row("小菜");
    let source = write_fixture(
        "runtime-cache-source.tsv",
        &format!("{HEADER}\n{}\n{}\n", row_text(&first), row_text(&second)),
    );
    let cache = std::env::temp_dir().join("gannyu-dict-test-runtime-cache.zst");
    let mut dictionary =
        Dictionary::load_split_tsvs_uncached(std::slice::from_ref(&source)).expect("load source");
    dictionary.rebuild_new_old_map();
    dictionary.rebuild_multi_reading_augmentation();
    dictionary.write_runtime_cache(&cache).expect("write cache");

    let loaded = Dictionary::load_runtime_cache(&cache).expect("load cache");
    let texts = |entries: Vec<&gannyu_input_core::DictionaryEntry>| {
        entries
            .into_iter()
            .map(|entry| entry.headword.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        texts(loaded.by_dialect_pinyin("tot6")),
        texts(dictionary.by_dialect_pinyin("tot6"))
    );
    assert_eq!(
        texts(loaded.by_mandarin_pinyin("tuo1")),
        texts(dictionary.by_mandarin_pinyin("tuo1"))
    );
    assert_eq!(
        texts(loaded.by_mandarin_word_pinyin("fei1chang2")),
        texts(dictionary.by_mandarin_word_pinyin("fei1chang2"))
    );
    assert_eq!(
        texts(loaded.by_mandarin_word_text("非常")),
        texts(dictionary.by_mandarin_word_text("非常"))
    );
    assert_eq!(
        loaded.lookup_prefix_ids("tot"),
        dictionary.lookup_prefix_ids("tot")
    );
    let _ = fs::remove_file(cache);
}

#[test]
fn missing_column_is_error() {
    // Source: resources/regions/lancong/dictionaries/words.tsv, 车马;
    // only the input header is truncated to exercise schema validation.
    let fields = source_row("车马");
    let body = format!("本词\t国际音标\n{}\n", fields[..2].join("\t"));
    let path = write_fixture("missing-column.tsv", &body);
    let error = Dictionary::load_tsv(&path).expect_err("should fail");
    assert!(matches!(error, DictionaryError::Row { .. }));
}
