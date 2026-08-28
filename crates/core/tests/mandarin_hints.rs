use gannyu_input_core::{Dictionary, MandarinHintBook};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE_DICTIONARY: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记
佢\t\tkui3\t\t赣\t他\tta1\t\t\t
许\t\the2\t\t赣\t那\tna4\t\t\t
唆奅\t\tso1'pao1\t\t赣\t吹牛\tchui1 niu2\t\t\t
";

fn write_fixture_dictionary() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gannyu-input-mandarin-hints-{suffix}.tsv"));
    fs::write(&path, FIXTURE_DICTIONARY).expect("fixture dictionary should write");
    path
}

fn fixture_hints() -> MandarinHintBook {
    let path = write_fixture_dictionary();
    let dictionary = Dictionary::load_tsv(&path).expect("dictionary fixture should load");
    let mut book = MandarinHintBook::empty();
    book.extend_dictionary(&dictionary);
    book
}

#[test]
fn dictionary_hints_lookup() {
    let book = fixture_hints();
    let entries = book.lookup_by_mandarin("他");
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|entry| entry.gan == "佢"));
}

#[test]
fn dictionary_hints_reverse_lookup() {
    let book = fixture_hints();
    let entries = book.lookup_by_gan("许");
    assert!(entries.iter().any(|entry| entry.mandarin == "那"));
}

#[test]
fn dictionary_hints_expand_multi_mandarin_words() {
    let body = "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记\n\
青菜\t\tqiang cai\t\t赣\t蔬菜/菜\tshu1 cai4/cai4\t\t\t\n";
    let path = write_fixture_dictionary();
    fs::write(&path, body).expect("fixture dictionary should rewrite");
    let dictionary = Dictionary::load_tsv(&path).expect("dictionary fixture should load");
    let mut book = MandarinHintBook::empty();
    book.extend_dictionary(&dictionary);

    assert!(book
        .lookup_by_mandarin("蔬菜")
        .iter()
        .any(|entry| entry.gan == "青菜"));
    assert!(book
        .lookup_by_mandarin("菜")
        .iter()
        .any(|entry| entry.gan == "青菜"));
}
