use gannyu_input_core::{
    load_region_from_manifest, InputOptions, InputPipeline, TranscriptionScheme,
};

#[test]
fn second_scheme_reads_and_displays_the_projected_reading() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let region = root.join("regions/test");
    let dictionaries = region.join("dictionaries");
    std::fs::create_dir_all(&dictionaries).unwrap();
    std::fs::write(
        root.join("manifest.toml"),
        "default_region = \"test\"\n\n[[regions]]\nid = \"test\"\nname_zh = \"测试\"\nstatus = \"active\"\npath = \"regions/test/region.toml\"\n",
    )
    .unwrap();
    std::fs::write(
        region.join("region.toml"),
        "[region]\nid = \"test\"\nname_zh = \"测试\"\nstatus = \"active\"\n\n[language]\nchars = \"dictionaries/chars.tsv\"\n\n[transcription]\nschemes = [\"gon-pin\", \"ipa\"]\ndefault = \"gon-pin\"\n",
    )
    .unwrap();
    std::fs::write(
        dictionaries.join("chars.tsv"),
        "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记\n甲\tka1\tka1\tjia3\t赣\t\t\t100\t\t\n",
    )
    .unwrap();
    std::fs::create_dir_all(region.join("transcription")).unwrap();
    std::fs::write(
        region.join("transcription/gon_to_scheme2.tsv"),
        "scheme1\tscheme2\nka\tkha\n",
    )
    .unwrap();
    std::fs::write(
        region.join("transcription/tone_map.tsv"),
        "scheme1\tscheme2\n1\t2\n",
    )
    .unwrap();

    let resource = load_region_from_manifest(root.join("manifest.toml"), "test").unwrap();
    let mut pipeline =
        InputPipeline::load_with_user_data_dir(&resource, Some(&root.join("user"))).unwrap();
    assert!(pipeline
        .retrieve("ka1")
        .iter()
        .any(|candidate| candidate.text == "甲"));

    pipeline.set_input_options(InputOptions {
        scheme: TranscriptionScheme::Ipa,
        ..InputOptions::default()
    });
    let candidate = pipeline
        .retrieve("kha2")
        .into_iter()
        .find(|candidate| candidate.text == "甲")
        .expect("second-scheme reading should resolve");
    assert_eq!(candidate.reading.as_deref(), Some("kha2"));

    assert!(pipeline.add_user_word("甲乙", "kha3", "yi3"));
    assert!(pipeline
        .retrieve("kha3")
        .iter()
        .any(|candidate| candidate.text == "甲乙"));

    pipeline.set_input_options(InputOptions::default());
    assert!(pipeline
        .retrieve("ka3")
        .iter()
        .any(|candidate| candidate.text == "甲乙"));
    assert!(pipeline
        .retrieve("ka1")
        .iter()
        .any(|candidate| candidate.text == "甲"));
}
