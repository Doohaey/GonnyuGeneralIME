use gannyu_input_core::{
    default_region_entry, list_region_entries, load_region_from_manifest, ResourceError,
};

const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/manifest.toml");

#[test]
fn default_region_is_registered() {
    let region = default_region_entry(MANIFEST_PATH).expect("default region should load");
    assert_eq!(region.id, "lancong");
}

#[test]
fn region_entries_can_be_listed() {
    let regions = list_region_entries(MANIFEST_PATH).expect("regions should load");
    assert!(regions.iter().any(|region| region.id == "lancong"));
    assert!(regions.iter().any(|region| region.id == "fenni"));
}

#[test]
fn region_resource_files_exist() {
    let resource =
        load_region_from_manifest(MANIFEST_PATH, "lancong").expect("region resource should load");
    assert_eq!(resource.config.region.name_zh, "南昌");
}

#[test]
fn unknown_region_returns_error() {
    let error = load_region_from_manifest(MANIFEST_PATH, "unknown").unwrap_err();
    assert!(matches!(error, ResourceError::UnknownRegion(region) if region == "unknown"));
}

#[test]
fn manifest_registers_active_regions() {
    let regions = list_region_entries(MANIFEST_PATH).expect("regions should load");
    assert_eq!(regions.len(), 2);
    assert!(regions.iter().all(|region| region.status == "active"));
    assert!(regions.iter().any(|region| region.id == "lancong"));
    assert!(regions.iter().any(|region| region.id == "fenni"));
}
