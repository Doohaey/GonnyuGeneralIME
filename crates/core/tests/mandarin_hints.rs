use gannyu_input_core::{Dictionary, MandarinHintBook};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
const HEADER: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记";
const SOURCES: &[&str] = &[
    include_str!("../../../resources/regions/lancong/dictionaries/chars.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/words.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/gan_chars.tsv"),
    include_str!("../../../resources/regions/lancong/dictionaries/gan_words.tsv"),
];

fn source_row(word: &str, has_equivalence: bool) -> Vec<String> {
    SOURCES
        .iter()
        .flat_map(|text| text.lines().skip(1))
        .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
        .find(|row| {
            row.first().is_some_and(|value| value == word)
                && (!has_equivalence || row.get(5).is_some_and(|value| !value.is_empty()))
        })
        .unwrap_or_else(|| panic!("{word} must be a Lancong dictionary entry"))
}

fn write_fixture_dictionary(rows: &[Vec<String>]) -> PathBuf {
    let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "gannyu-input-mandarin-hints-{}-{fixture_id}.tsv",
        std::process::id()
    ));
    let body = std::iter::once(HEADER.to_string())
        .chain(rows.iter().map(|row| row.join("\t")))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, body).expect("fixture dictionary should write");
    path
}

fn fixture_hints(rows: &[Vec<String>]) -> MandarinHintBook {
    let path = write_fixture_dictionary(rows);
    let dictionary = Dictionary::load_tsv(&path).expect("dictionary fixture should load");
    let mut book = MandarinHintBook::empty();
    book.extend_dictionary(&dictionary);
    book
}

#[test]
fn dictionary_hints_lookup() {
    // Source: resources/regions/lancong/dictionaries/gan_chars.tsv, 渠.
    let book = fixture_hints(&[source_row("渠", true)]);
    let entries = book.lookup_by_mandarin("他");
    assert!(entries.iter().any(|entry| entry.gan == "渠"));
}

#[test]
fn dictionary_hints_reverse_lookup() {
    // Source: resources/regions/lancong/dictionaries/gan_chars.tsv, 渠.
    let book = fixture_hints(&[source_row("渠", true)]);
    assert!(book
        .lookup_by_gan("渠")
        .iter()
        .any(|entry| entry.mandarin == "他"));
}

#[test]
fn dictionary_hints_expand_multi_mandarin_words() {
    // Source: resources/regions/lancong/dictionaries/gan_chars.tsv, 脱.
    let book = fixture_hints(&[source_row("脱", true)]);
    for equivalent in ["很", "极其"] {
        assert!(book
            .lookup_by_mandarin(equivalent)
            .iter()
            .any(|entry| entry.gan == "脱"));
    }
}
