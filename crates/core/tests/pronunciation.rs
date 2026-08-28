use gannyu_input_core::{Dictionary, PronunciationBook, Register};
use std::fs;
use std::path::PathBuf;

fn write_fixture(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gannyu-test-{name}"));
    fs::write(&path, content).expect("fixture write");
    path
}

#[test]
fn checked_alternatives_derive_from_coda() {
    let path = write_fixture(
        "pronunciation-checked.jsonl",
        r#"{"id":"t-1","grapheme":"日","readings":[{"syllable":"nit","tone_class":7,"coda":"t"}]}
{"id":"t-1k","grapheme":"日","readings":[{"syllable":"nik","tone_class":7,"coda":"k"}]}
{"id":"t-2","grapheme":"日","readings":[{"syllable":"ni","tone_class":5}]}
{"id":"t-3","grapheme":"色","readings":[{"syllable":"set","tone_class":6,"coda":"t"}]}
"#,
    );
    let book = PronunciationBook::load_jsonl(&path).expect("load fixture");
    let alts = book.checked_alternatives("ni");
    assert!(alts.iter().any(|syllable| **syllable == "nit"));
    assert!(alts.iter().any(|syllable| **syllable == "nik"));
    let k_alts = book.checked_alternatives("nik");
    assert!(k_alts.iter().any(|syllable| **syllable == "nit"));
    assert!(k_alts.iter().any(|syllable| **syllable == "nik"));
    let se_alts = book.checked_alternatives("se");
    assert!(se_alts.iter().any(|syllable| **syllable == "set"));
    fs::remove_file(&path).ok();
}

#[test]
fn register_correction_reports_wen_when_observed_is_bai() {
    let path = write_fixture(
        "pronunciation-register.jsonl",
        r#"{"id":"r-1","grapheme":"明","readings":[{"syllable":"miang","register":"bai"},{"syllable":"ming","register":"wen"}]}
"#,
    );
    let book = PronunciationBook::load_jsonl(&path).expect("load fixture");
    let correction = book
        .register_correction("明", "miang")
        .expect("expected correction");
    assert_eq!(correction.observed_register, Register::Bai);
    assert!(correction
        .alternates
        .iter()
        .any(|alternate| alternate.syllable == "ming" && alternate.register == Register::Wen));
    fs::remove_file(&path).ok();
}

#[test]
fn register_correction_returns_none_for_matching_register_only() {
    let path = write_fixture(
        "pronunciation-register-none.jsonl",
        r#"{"id":"r-2","grapheme":"日","readings":[{"syllable":"nit","register":"common"},{"syllable":"ni","register":"common"}]}
"#,
    );
    let book = PronunciationBook::load_jsonl(&path).expect("load fixture");
    assert!(book.register_correction("日", "nit").is_none());
    fs::remove_file(&path).ok();
}

#[test]
fn dictionary_entries_build_checked_alternatives() {
    let path = write_fixture(
        "pronunciation-source.tsv",
        "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记\n日\t\tnit\t\t赣\t日\tri4\t\t\t\n日\t\tnik\t\t赣\t日\tri4\t\t\t\n日\t\tni\t\t赣\t日\tri4\t\t\t\n",
    );
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary fixture");
    let mut book = PronunciationBook::empty();
    book.extend_dictionary(&dictionary);
    let alts = book.checked_alternatives("ni");
    assert!(alts.iter().any(|syllable| **syllable == "nit"));
    assert!(alts.iter().any(|syllable| **syllable == "nik"));
    fs::remove_file(&path).ok();
}
