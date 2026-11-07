use gannyu_input_core::{load_region_from_manifest, InputOptions, InputPipeline, ReadingMode};
use std::path::Path;

fn source_pipeline() -> (tempfile::TempDir, InputPipeline) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut region = load_region_from_manifest(root.join("manifest.toml"), "lancong").unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let words = [
        "车马", "䁐牛", "放牛", "几多", "多少", "这", "义", "主义", "经", "经济", "人", "人口",
        "系", "五", "级",
    ];
    let characters: std::collections::HashSet<char> =
        words.iter().flat_map(|word| word.chars()).collect();
    for relative in [
        "dictionaries/chars.tsv",
        "dictionaries/words.tsv",
        "dictionaries/gan_chars.tsv",
        "dictionaries/gan_words.tsv",
    ] {
        let contents = std::fs::read_to_string(region.root.join(relative)).unwrap();
        let mut lines = contents.lines();
        let mut selected = vec![lines.next().unwrap()];
        selected.extend(lines.filter(|line| {
            let headword = line.split('\t').next().unwrap_or("");
            words.contains(&headword)
                || (headword.chars().count() == 1
                    && characters.contains(&headword.chars().next().unwrap()))
        }));
        let target = fixture.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, selected.join("\n") + "\n").unwrap();
    }
    region.config.phonology.fuzzy_map =
        Some(root.join("fuzzy_scheme.tsv").to_string_lossy().into_owned());
    region.root = fixture.path().to_path_buf();
    let pipeline =
        InputPipeline::load_with_user_data_dir(&region, Some(&fixture.path().join("user")))
            .unwrap();
    (fixture, pipeline)
}

#[test]
fn hot_options_filter_before_lookup_and_restore_original_candidates() {
    let (_fixture, mut pipeline) = source_pipeline();
    let original = pipeline.retrieve("ca1ma1");
    assert!(original.iter().any(|candidate| candidate.text == "车马"));
    assert!(pipeline
        .retrieve("che1ma3")
        .iter()
        .any(|candidate| candidate.text == "车马"));
    for relations in [true, false] {
        for synonyms in [true, false] {
            pipeline.set_input_options(InputOptions {
                relations,
                synonyms,
                ..InputOptions::default()
            });
            let candidates = pipeline.retrieve("ji3do1");
            assert!(candidates.iter().any(|candidate| candidate.text == "几多"));
            assert_eq!(
                candidates.iter().any(|candidate| candidate.text == "多少"),
                synonyms
            );
            let source = candidates
                .iter()
                .find(|candidate| candidate.text == "几多")
                .unwrap();
            assert_eq!(
                source
                    .annotation
                    .as_deref()
                    .unwrap_or("")
                    .contains("[联]多少"),
                synonyms
            );
            assert_eq!(
                pipeline
                    .retrieve("yang4niu4")
                    .iter()
                    .any(|candidate| candidate.text == "放牛"),
                relations
            );
            assert!(pipeline
                .retrieve("do1sêu3")
                .iter()
                .any(|candidate| candidate.text == "多少"));
            let chars = pipeline.retrieve("za3");
            let character = chars
                .iter()
                .find(|candidate| candidate.text == "这")
                .unwrap();
            assert_eq!(character.mandarin_only, relations);
            assert_eq!(
                character
                    .annotation
                    .as_deref()
                    .unwrap_or("")
                    .contains("[不习用]"),
                relations
            );
        }
    }
    pipeline.set_input_options(InputOptions {
        annotations: false,
        ..InputOptions::default()
    });
    let candidates = pipeline.retrieve("ji3do1");
    assert!(candidates.iter().any(|candidate| candidate.text == "几多"));
    assert!(candidates.iter().any(|candidate| candidate.text == "多少"));
    assert!(candidates
        .iter()
        .all(|candidate| candidate.annotation.is_none()));
    pipeline.set_input_options(InputOptions::default());
    let mut options = InputOptions {
        fuzzy: false,
        relations: false,
        ..InputOptions::default()
    };
    pipeline.set_input_options(options);
    assert!(pipeline
        .retrieve("ca1ma1")
        .iter()
        .any(|candidate| candidate.text == "车马"));
    assert!(pipeline
        .dictionary()
        .by_mandarin_pinyin("che1ma3")
        .is_empty());
    assert!(pipeline
        .dictionary()
        .entries()
        .iter()
        .all(|entry| entry.category.as_ref() != "官"));
    assert!(!pipeline
        .retrieve("fang4niu2")
        .iter()
        .any(|candidate| candidate.text == "放牛"));
    assert!(pipeline.fuzzy_map().entries.iter().all(|entry| matches!(
        (entry.gon_fuzzy.as_str(), entry.gon_pin.as_str()),
        ("i", "ï")
            | ("ii", "ï")
            | ("e", "ê")
            | ("ee", "ê")
            | ("ir", "e")
            | ("er", "e")
            | ("o", "ô")
            | ("oo", "ô")
    )));
    assert!(pipeline.fuzzy_map().entries.len() >= 3);
    assert!(pipeline
        .fuzzy_map()
        .entries
        .iter()
        .any(|entry| entry.gon_fuzzy == "ir" && entry.gon_pin == "e"));
    assert!(pipeline
        .fuzzy_map()
        .entries
        .iter()
        .any(|entry| entry.gon_fuzzy == "er" && entry.gon_pin == "e"));
    options.fuzzy = true;
    pipeline.set_input_options(options);
    assert!(pipeline
        .retrieve("che1ma3")
        .iter()
        .any(|candidate| candidate.text == "车马"));
    options.annotations = false;
    pipeline.set_input_options(options);
    assert!(pipeline
        .retrieve("ca1ma1")
        .iter()
        .all(|candidate| candidate.annotation.is_none()));
    options.reading_mode = ReadingMode::New;
    pipeline.set_input_options(options);
    assert!(pipeline
        .dictionary()
        .entries()
        .iter()
        .all(|entry| !entry.new_old.starts_with('老')));
    assert!(pipeline
        .dictionary()
        .by_dialect_pinyin("yi5")
        .iter()
        .any(|entry| entry.headword.as_ref() == "义"));
    assert!(!pipeline
        .dictionary()
        .by_dialect_pinyin("ni5")
        .iter()
        .any(|entry| entry.headword.as_ref() == "义"));
    options.reading_mode = ReadingMode::Old;
    pipeline.set_input_options(options);
    assert!(pipeline
        .dictionary()
        .entries()
        .iter()
        .all(|entry| !entry.new_old.starts_with('新')));
    pipeline.set_input_options(InputOptions::default());
    assert_eq!(
        serde_json::to_value(pipeline.retrieve("ca1ma1")).unwrap(),
        serde_json::to_value(original).unwrap()
    );
}

#[test]
fn wenbai_options_preserve_labels_and_independent_lookup_paths() {
    let (_fixture, mut pipeline) = source_pipeline();
    let mut wenbai = InputOptions {
        mandarin: false,
        fuzzy_rules: false,
        ..InputOptions::default()
    };
    pipeline.set_input_options(wenbai);
    assert!(pipeline
        .retrieve("jiang1ji4")
        .iter()
        .any(|candidate| candidate.text == "经济"));
    assert!(pipeline
        .retrieve("len4kiêu3")
        .iter()
        .any(|candidate| candidate.text == "人口"));
    wenbai.wenbai = false;
    pipeline.set_input_options(wenbai);
    assert!(!pipeline
        .retrieve("jiang1ji4")
        .iter()
        .any(|candidate| candidate.text == "经济"));
    assert!(!pipeline
        .dictionary()
        .by_dialect_pinyin("len4kiêu3")
        .iter()
        .any(|entry| entry.headword.as_ref() == "人口"));
    assert!(pipeline
        .retrieve("jin1ji4")
        .iter()
        .any(|candidate| candidate.text == "经济"));
    let annotated = pipeline.retrieve("jin1");
    let character = annotated
        .iter()
        .find(|candidate| candidate.text == "经")
        .unwrap();
    let annotation = character.annotation.as_deref().unwrap();
    assert!(annotation.contains("[文]") && annotation.contains("[白]"));
    wenbai.mandarin = true;
    pipeline.set_input_options(wenbai);
    assert!(pipeline
        .dictionary()
        .by_mandarin_pinyin("jing1ji4")
        .iter()
        .any(|entry| entry.headword.as_ref() == "经济"));
    let independent = pipeline.retrieve("jing1ji4");
    assert!(
        independent.iter().any(|candidate| candidate.text == "经济"),
        "{:?}",
        independent
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<Vec<_>>()
    );
    wenbai.wenbai = true;
    wenbai.fuzzy = false;
    pipeline.set_input_options(wenbai);
    assert!(!pipeline
        .retrieve("jiang1ji4")
        .iter()
        .any(|candidate| candidate.text == "经济"));
    wenbai.fuzzy = true;
    pipeline.set_input_options(wenbai);
    assert!(pipeline
        .retrieve("jiang1ji4")
        .iter()
        .any(|candidate| candidate.text == "经济"));
    pipeline.set_input_options(InputOptions::default());
}
