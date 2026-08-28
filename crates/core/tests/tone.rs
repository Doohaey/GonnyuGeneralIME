use gannyu_input_core::load_region_from_manifest;

const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/manifest.toml");

#[test]
fn lancong_tone_classes_parsed() {
    let resource =
        load_region_from_manifest(MANIFEST_PATH, "lancong").expect("region resource should load");
    let tone_classes = &resource.config.tone_classes;
    assert_eq!(tone_classes.len(), 7);
    let yin_ping = tone_classes.get(&1).expect("tone class 1 should exist");
    assert_eq!(yin_ping.name, "阴平");
    assert_eq!(yin_ping.value, "42");
    let yang_ru = tone_classes.get(&7).expect("tone class 7 should exist");
    assert_eq!(yang_ru.name, "阳入");
    assert_eq!(yang_ru.value, "1/2");
}
