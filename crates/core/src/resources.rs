use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub default_region: String,
    pub regions: Vec<RegionEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegionEntry {
    pub id: String,
    pub name_zh: String,
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RegionConfig {
    pub region: RegionMetadata,
    #[serde(default)]
    pub phonology: PhonologyFiles,
    #[serde(default)]
    pub dictionaries: DictionaryFiles,
    #[serde(default)]
    pub language: LanguageFiles,
    #[serde(default, deserialize_with = "deserialize_tone_classes")]
    pub tone_classes: BTreeMap<u8, ToneClass>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RegionMetadata {
    pub id: String,
    pub name_zh: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct PhonologyFiles {
    pub syllables: Option<String>,
    pub pronunciations: Option<String>,
    pub fuzzy_map: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct DictionaryFiles {
    pub feature_words: Option<String>,
    pub candidates: Option<String>,
    pub associations: Option<String>,
    pub slang: Option<String>,
    pub mandarin_hints: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct LanguageFiles {
    pub chars: Option<String>,
    pub words: Option<String>,
    pub gan_chars: Option<String>,
    pub gan_words: Option<String>,
    pub pinyin_rules: Option<String>,
    pub raw_input: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ToneClass {
    pub name: String,
    pub value: String,
}

fn deserialize_tone_classes<'de, D>(deserializer: D) -> Result<BTreeMap<u8, ToneClass>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: BTreeMap<String, ToneClass> = BTreeMap::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(key, value)| {
            let parsed = key.parse::<u8>().map_err(|error| {
                serde::de::Error::custom(format!("tone class key {key}: {error}"))
            })?;
            Ok((parsed, value))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionResource {
    pub entry: RegionEntry,
    pub config: RegionConfig,
    pub root: PathBuf,
}

#[derive(Debug)]
pub enum ResourceError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    EmptyRegionList,
    UnknownRegion(String),
    RegionIdMismatch {
        expected: String,
        actual: String,
        path: PathBuf,
    },
    MissingResourceFile(PathBuf),
}

impl Display for ResourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::Io(error) => write!(formatter, "{error}"),
            ResourceError::Toml(error) => write!(formatter, "{error}"),
            ResourceError::EmptyRegionList => write!(formatter, "resource manifest has no regions"),
            ResourceError::UnknownRegion(region_id) => {
                write!(formatter, "unknown region id: {region_id}")
            }
            ResourceError::RegionIdMismatch {
                expected,
                actual,
                path,
            } => write!(
                formatter,
                "region id mismatch in {}: expected {expected}, got {actual}",
                path.display()
            ),
            ResourceError::MissingResourceFile(path) => {
                write!(formatter, "missing resource file: {}", path.display())
            }
        }
    }
}

impl Error for ResourceError {}

impl From<std::io::Error> for ResourceError {
    fn from(error: std::io::Error) -> Self {
        ResourceError::Io(error)
    }
}

impl From<toml::de::Error> for ResourceError {
    fn from(error: toml::de::Error) -> Self {
        ResourceError::Toml(error)
    }
}

impl RegionConfig {
    pub fn resource_files(&self) -> Vec<&str> {
        let candidates = [
            self.phonology.syllables.as_deref(),
            self.phonology.pronunciations.as_deref(),
            self.phonology.fuzzy_map.as_deref(),
            self.dictionaries.candidates.as_deref(),
            self.dictionaries.feature_words.as_deref(),
            self.dictionaries.associations.as_deref(),
            self.dictionaries.slang.as_deref(),
            self.dictionaries.mandarin_hints.as_deref(),
            self.language.chars.as_deref(),
            self.language.words.as_deref(),
            self.language.gan_chars.as_deref(),
            self.language.gan_words.as_deref(),
            self.language.pinyin_rules.as_deref(),
        ];
        candidates.into_iter().flatten().collect()
    }
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<Manifest, ResourceError> {
    let content = fs::read_to_string(path)?;
    let manifest = toml::from_str::<Manifest>(&content)?;
    if manifest.regions.is_empty() {
        return Err(ResourceError::EmptyRegionList);
    }
    Ok(manifest)
}

pub fn list_region_entries(path: impl AsRef<Path>) -> Result<Vec<RegionEntry>, ResourceError> {
    Ok(load_manifest(path)?.regions)
}

pub fn default_region_entry(path: impl AsRef<Path>) -> Result<RegionEntry, ResourceError> {
    let manifest = load_manifest(path)?;
    manifest
        .regions
        .iter()
        .find(|region| region.id == manifest.default_region)
        .cloned()
        .ok_or(ResourceError::UnknownRegion(manifest.default_region))
}

pub fn load_region_from_manifest(
    manifest_path: impl AsRef<Path>,
    region_id: &str,
) -> Result<RegionResource, ResourceError> {
    let manifest_path = manifest_path.as_ref();
    let manifest = load_manifest(manifest_path)?;
    let entry = manifest
        .regions
        .iter()
        .find(|region| region.id == region_id)
        .cloned()
        .ok_or_else(|| ResourceError::UnknownRegion(region_id.to_owned()))?;

    let resource_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = resource_root.join(&entry.path);
    let content = fs::read_to_string(&config_path)?;
    let config = toml::from_str::<RegionConfig>(&content)?;

    if config.region.id != entry.id {
        return Err(ResourceError::RegionIdMismatch {
            expected: entry.id,
            actual: config.region.id,
            path: config_path,
        });
    }

    let region_root = resource_root.join(&entry.path);
    let region_root = region_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(resource_root);

    // Validasi berkas sumber daya — lewati TSV sumber jika cache runtime tersedia
    // (TSV sumber hanya diperlukan saat build, runtime menggunakan cache + indeks FST).
    let cache_candidate = region_root.join("dictionaries/dictionary_runtime_cache.zst");
    let has_runtime_cache = cache_candidate.is_file();
    let skip_when_cached: &[&str] = &["chars.tsv", "words.tsv", "gan_chars.tsv", "gan_words.tsv"];
    let always_optional: &[&str] = &["pinyin_rules.md"];
    for file in config.resource_files() {
        let file_path = region_root.join(file);
        if !file_path.is_file() {
            let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if has_runtime_cache && skip_when_cached.contains(&file_name) {
                continue;
            }
            if always_optional.contains(&file_name) {
                continue;
            }
            return Err(ResourceError::MissingResourceFile(file_path));
        }
    }

    Ok(RegionResource {
        entry,
        config,
        root: region_root,
    })
}
