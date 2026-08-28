use gannyu_input_core::{Dictionary, SlangBook, TriggerKind};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE_DICTIONARY: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记
唆奅\t\tso1'pao1\t\t赣\t吹牛\tchui1 niu2\t\t\t
唆奅\t\tsuo1'pao1\t\t赣\t吹牛\tchui1 niu2\t\t\t
刻时里\t\tke4'xi2'li3\t\t赣\t这时里\tzhe4 shi2 li3\t\t\t
";

fn write_fixture_dictionary() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gannyu-input-slang-{suffix}.tsv"));
    fs::write(&path, FIXTURE_DICTIONARY).expect("fixture dictionary should write");
    path
}

fn fixture_book() -> SlangBook {
    let path = write_fixture_dictionary();
    let dictionary = Dictionary::load_tsv(&path).expect("dictionary fixture should load");
    let mut book = SlangBook::empty();
    book.load_dictionary(&dictionary);
    book
}

#[test]
fn forward_lookup_by_mandarin_returns_slang() {
    let book = fixture_book();
    let hits = book.slang_by_trigger("吹牛");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].entry.slang, "唆奅");
    assert_eq!(hits[0].matched_trigger.kind, TriggerKind::Mandarin);
}

#[test]
fn forward_lookup_by_fragment_returns_slang() {
    let book = fixture_book();
    let hits = book.slang_by_trigger("suo'pao");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].entry.slang, "唆奅");
    assert_eq!(hits[0].matched_trigger.kind, TriggerKind::GanFragment);
}

#[test]
fn reverse_lookup_excludes_fragments() {
    let book = fixture_book();
    let hits = book.slang_reverse("唆奅");
    assert!(!hits.is_empty());
    for hit in &hits {
        for trigger in &hit.triggers {
            assert_ne!(
                trigger.kind,
                TriggerKind::GanFragment,
                "fragment 不应出现在反查结果"
            );
        }
    }
}

#[test]
fn feature_lookup_by_fragment_returns_full_form() {
    let book = fixture_book();
    let hits = book.slang_by_trigger("ke");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].entry.slang, "刻时里");
}
