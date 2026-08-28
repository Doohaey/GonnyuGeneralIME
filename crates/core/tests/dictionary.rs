use gannyu_input_core::{Dictionary, DictionaryError};
use std::fs;
use std::path::PathBuf;

fn write_fixture(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gannyu-dict-test-{name}"));
    fs::write(&path, content).expect("fixture write");
    path
}

const HEADER: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记";

#[test]
fn loads_eight_column_rows() {
    let body = format!(
        "{HEADER}\n\
         渠\ttɕʰy21\tqu\tqu2\t赣\t他\tta1\t1000\t\n\
         吹牛\ttsʰui21 niu21\tcui niu\tchui1 niu2\t赣\t吹牛\tchui1 niu2\t50\t\n"
    );
    let path = write_fixture("eight-column.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    assert_eq!(dictionary.len(), 2);

    let first = &dictionary.entries()[0];
    assert_eq!(first.headword, "渠");
    assert_eq!(first.ipa, "tɕʰy21");
    assert_eq!(first.dialect_pinyin, "qu");
    assert_eq!(first.mandarin_pinyin, "qu2");
    assert_eq!(first.category, "赣");
    assert_eq!(first.mandarin_word, "他");
    assert_eq!(first.mandarin_word_pinyin, "ta1");
    assert_eq!(first.frequency, Some(1000));
}

#[test]
fn skips_comments_and_blank_lines() {
    let body = format!("# comment\n\n{HEADER}\n\n# another\n他\tta1\tta\tta1\t官\t他\tta1\t9\t\n");
    let path = write_fixture("comments.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    assert_eq!(dictionary.len(), 1);
    assert!(dictionary.entries()[0].is_mandarin_only());
}

#[test]
fn tolerates_column_reordering() {
    let header = "词频\t本词\t方言拼音\t国际音标\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t同义词\t新旧标记";
    let body = format!("{header}\n7\t渠\tqu\ttɕʰy21\tqu2\t赣\t他\tta1\t\t\n");
    let path = write_fixture("reorder.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let entry = &dictionary.entries()[0];
    assert_eq!(entry.headword, "渠");
    assert_eq!(entry.frequency, Some(7));
    assert_eq!(entry.dialect_pinyin, "qu");
}

#[test]
fn lookup_by_dialect_pinyin() {
    let body = format!(
        "{HEADER}\n\
         渠\ttɕʰy21\tqu\tqu2\t赣\t他\tta1\t1000\t\n\
         佢\ttɕʰy21\tqu\tqu2\t赣\t他\tta1\t10\t\n\
         去\t\tqu\tqu4\t官\t去\tqu4\t9\t\n"
    );
    let path = write_fixture("lookup.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let hits = dictionary.by_dialect_pinyin("qu");
    assert_eq!(hits.len(), 3);
}

#[test]
fn normalizes_pinyin_columns_and_lookup_aliases() {
    let body = format!(
        "{HEADER}\n\
         唆奅\t\tsuo1'Pao1\tChui1   Niu2\t赣\t吹牛\tCHUI1 NIU2\t50\t\n\
         細\t\tXi&Si\txi4\t赣\t\t\t9\t\n"
    );
    let path = write_fixture("normalized-lookup.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");

    assert_eq!(dictionary.entries()[0].dialect_pinyin, "suo1'pao1");
    assert_eq!(dictionary.entries()[0].mandarin_pinyin, "chui1 niu2");
    assert_eq!(dictionary.entries()[0].mandarin_word_pinyin, "chui1 niu2");
    assert_eq!(dictionary.by_dialect_pinyin("suo'pao").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("suo1'pao1").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("suo1pao1").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("suopao").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("xi").len(), 1);
    assert_eq!(dictionary.by_dialect_pinyin("si").len(), 1);
    assert_eq!(dictionary.by_mandarin_pinyin("chui1niu2").len(), 1);
    assert_eq!(dictionary.by_mandarin_pinyin("chuiniu").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_pinyin("chuiniu").len(), 1);
}

#[test]
fn expands_multi_mandarin_word_indexes() {
    let body = format!(
        "{HEADER}\n\
         青菜\ttɕʰiaŋ1 tsʰai3\tqiang cai\tqing1 cai4\t赣\t蔬菜/菜\tshu1 cai4/cai4\t50\t\n"
    );
    let path = write_fixture("multi-mandarin.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");

    assert_eq!(dictionary.by_mandarin_word_text("蔬菜").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_text("菜").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_pinyin("shucai").len(), 1);
    assert_eq!(dictionary.by_mandarin_word_pinyin("cai").len(), 1);
}

#[test]
fn missing_column_is_error() {
    let body = "本词\t国际音标\n渠\ttɕʰy21\n".to_string();
    let path = write_fixture("missing-column.tsv", &body);
    let error = Dictionary::load_tsv(&path).expect_err("should fail");
    assert!(matches!(error, DictionaryError::Row { .. }));
}
