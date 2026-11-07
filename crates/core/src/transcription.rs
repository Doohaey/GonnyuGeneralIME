use crate::dictionary::{Dictionary, DictionaryEntry};
use crate::resources::RegionResource;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Scheme2Map {
    by_gon: HashMap<String, String>,
    by_scheme2: HashMap<String, String>,
    tones: HashMap<String, String>,
}

impl Scheme2Map {
    /// Load the second-scheme map generated at build time.  Missing optional
    /// files simply disable the second scheme, so a first-scheme-only build
    /// keeps working without generated artifacts.
    pub fn load(resource: &RegionResource) -> std::io::Result<Option<Self>> {
        if !resource
            .config
            .transcription
            .schemes
            .iter()
            .any(|scheme| scheme == "ipa")
        {
            return Ok(None);
        }
        let gon_path = configured_path(
            resource,
            resource.config.transcription.gon_to_scheme2.as_deref(),
            "transcription/gon_to_scheme2.tsv",
        );
        let tone_path = configured_path(
            resource,
            resource.config.transcription.tone_map.as_deref(),
            "transcription/tone_map.tsv",
        );
        let (Some(gon_path), Some(tone_path)) = (gon_path, tone_path) else {
            return Ok(None);
        };
        if !gon_path.is_file() || !tone_path.is_file() {
            return Ok(None);
        }
        Self::from_paths(&gon_path, &tone_path).map(Some)
    }

    pub fn from_paths(gon_path: &Path, tone_path: &Path) -> std::io::Result<Self> {
        let mut by_gon = HashMap::new();
        let mut by_scheme2 = HashMap::new();
        for (scheme1, scheme2) in read_tsv(gon_path)? {
            if scheme1.is_empty() || scheme2.is_empty() {
                continue;
            }
            by_scheme2
                .entry(scheme2.clone())
                .or_insert_with(|| scheme1.clone());
            by_gon.entry(scheme1).or_insert(scheme2);
        }
        let tones = read_tsv(tone_path)?.into_iter().collect();
        Ok(Self {
            by_gon,
            by_scheme2,
            tones,
        })
    }

    pub fn convert_reading(&self, reading: &str) -> String {
        reading
            .split_whitespace()
            .map(|token| self.convert_token(token))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_scheme1_reading(&self, reading: &str) -> String {
        reading
            .split_whitespace()
            .map(|token| self.convert_token_to_scheme1(token))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn dictionary(&self, source: &Dictionary) -> Dictionary {
        let entries = source.entries().iter().map(|entry| {
            let mut entry: DictionaryEntry = entry.clone();
            entry.dialect_pinyin = self.convert_reading(&entry.dialect_pinyin).into();
            entry.entry_index = 0;
            entry
        });
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries(entries);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation_with_wenbai(true);
        dictionary
    }

    fn convert_token(&self, token: &str) -> String {
        let (body, tone) = split_tone(token);
        let Some(scheme2) = self.by_gon.get(body) else {
            return token.to_string();
        };
        match tone {
            Some(tone) => format!("{scheme2}{}", self.convert_tone(tone)),
            None => scheme2.clone(),
        }
    }

    fn convert_token_to_scheme1(&self, token: &str) -> String {
        let (body, tone) = split_tone(token);
        let Some(scheme1) = self.by_scheme2.get(body) else {
            return token.to_string();
        };
        match tone {
            Some(tone) => format!("{scheme1}{}", self.convert_tone_back(tone)),
            None => scheme1.clone(),
        }
    }

    fn convert_tone(&self, tone: &str) -> String {
        self.tones
            .get(tone)
            .cloned()
            .unwrap_or_else(|| tone.to_string())
    }

    fn convert_tone_back(&self, tone: &str) -> String {
        self.tones
            .iter()
            .find_map(|(scheme1, scheme2)| (scheme2 == tone).then(|| scheme1.clone()))
            .unwrap_or_else(|| tone.to_string())
    }
}

fn configured_path(
    resource: &RegionResource,
    value: Option<&str>,
    default: &str,
) -> Option<PathBuf> {
    value
        .map(|relative| resource.root.join(relative))
        .or_else(|| Some(resource.root.join(default)))
}

fn read_tsv(path: &Path) -> std::io::Result<Vec<(String, String)>> {
    let content = fs::read_to_string(path)?;
    Ok(content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (first, second) = line.split_once('\t')?;
            let first = first.trim();
            let second = second.trim();
            (!first.is_empty() && !second.is_empty())
                .then(|| (first.to_string(), second.to_string()))
        })
        .collect())
}

fn split_tone(token: &str) -> (&str, Option<&str>) {
    let token = token.trim();
    let (unstarred, starred) = match token.strip_suffix('*') {
        Some(base) => (base, true),
        None => (token, false),
    };
    let digit_start = unstarred
        .char_indices()
        .rev()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map(|(index, _)| index);
    match digit_start {
        Some(index) => {
            let tone = if starred {
                &token[index..]
            } else {
                &unstarred[index..]
            };
            (&unstarred[..index], Some(tone))
        }
        None => (token, None),
    }
}

#[cfg(test)]
mod tests {
    use super::Scheme2Map;
    use crate::dictionary::{Dictionary, DictionaryEntry};
    use std::fs;
    use std::path::Path;

    fn fixture_map(root: &Path) -> Scheme2Map {
        let gon = root.join("gon_to_scheme2.tsv");
        let tone = root.join("tone_map.tsv");
        fs::write(&gon, "scheme1\tscheme2\nka\tkha\nma\tma\n").unwrap();
        fs::write(&tone, "scheme1\tscheme2\n1\t2\n5*\t6*\n").unwrap();
        Scheme2Map::from_paths(&gon, &tone).unwrap()
    }

    fn entry(headword: &str, reading: &str) -> DictionaryEntry {
        DictionaryEntry {
            headword: headword.into(),
            ipa: "".into(),
            dialect_pinyin: reading.into(),
            mandarin_pinyin: "".into(),
            category: "赣".into(),
            mandarin_word: "".into(),
            mandarin_word_pinyin: "".into(),
            frequency: None,
            synonyms: "".into(),
            entry_index: 0,
            new_old: "".into(),
        }
    }

    #[test]
    fn converts_readings_and_tone_stars() {
        let root = tempfile::tempdir().unwrap();
        let map = fixture_map(root.path());
        assert_eq!(map.convert_reading("ka1 ma5*"), "kha2 ma6*");
        assert_eq!(map.to_scheme1_reading("kha2 ma6*"), "ka1 ma5*");
        assert_eq!(map.convert_reading("unknown1"), "unknown1");
    }

    #[test]
    fn builds_a_second_scheme_dictionary() {
        let root = tempfile::tempdir().unwrap();
        let map = fixture_map(root.path());
        let mut source = Dictionary::empty();
        source.extend_from_entries([entry("甲", "ka1"), entry("妈", "ma5*")]);
        let scheme2 = map.dictionary(&source);
        assert_eq!(scheme2.by_headword("甲")[0].dialect_pinyin.as_ref(), "kha2");
        assert_eq!(scheme2.by_headword("妈")[0].dialect_pinyin.as_ref(), "ma6*");
        assert!(!scheme2.by_dialect_pinyin("kha2").is_empty());
    }
}
