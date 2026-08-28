use gannyu_input_core::{load_region_from_manifest, CandidateSource, CandidateTier, InputPipeline};
use std::path::PathBuf;

fn manifest_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/manifest.toml"
    ))
}

fn load_pipeline() -> InputPipeline {
    let resource =
        load_region_from_manifest(manifest_path(), "lancong").expect("region should load");
    InputPipeline::load(&resource).expect("pipeline should load")
}

#[test]
fn compose_mandarin_trigger_returns_slang_primary() {
    let pipeline = load_pipeline();
    let candidates = pipeline.compose("吹牛");
    assert!(!candidates.is_empty());
    let first = &candidates[0];
    assert_eq!(first.source, CandidateSource::Slang);
    assert_eq!(first.text, "唆奅");
    assert_eq!(first.tier, CandidateTier::Primary);
}

#[test]
fn compose_unknown_input_returns_empty() {
    let pipeline = load_pipeline();
    let candidates = pipeline.compose("不存在的输入xyz");
    assert!(candidates.is_empty());
}

#[test]
fn compose_mandarin_hint_returns_gan_word() {
    let pipeline = load_pipeline();
    let candidates = pipeline.compose("他");
    assert!(candidates
        .iter()
        .any(|item| item.source == CandidateSource::Slang && item.text == "佢"));
}
