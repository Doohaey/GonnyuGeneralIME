use crate::dictionary::Dictionary;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Register {
    Wen,
    Bai,
    #[default]
    Common,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reading {
    pub syllable: String,
    #[serde(default)]
    pub tone_class: Option<u8>,
    #[serde(default)]
    pub register: Option<Register>,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub coda: Option<String>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl Reading {
    pub fn is_checked(&self) -> bool {
        if let Some(coda) = self.coda.as_deref() {
            return matches!(coda, "t" | "k" | "ʔ");
        }
        if matches!(self.tone_class, Some(6) | Some(7)) {
            if let Some(last) = self.syllable.chars().last() {
                if matches!(last, 't' | 'k') {
                    return true;
                }
            }
        }
        false
    }

    pub fn checked_base(&self) -> Option<String> {
        if !self.is_checked() {
            return None;
        }
        let coda = self.coda.as_deref().unwrap_or_else(|| {
            let last = self.syllable.chars().last().unwrap_or(' ');
            match last {
                't' => "t",
                'k' => "k",
                _ => "",
            }
        });
        if coda.is_empty() {
            return Some(self.syllable.clone());
        }
        // Strip trailing tone digit before checking for coda suffix
        let syllable_notone = self.syllable.trim_end_matches(|c: char| c.is_ascii_digit());
        if let Some(stripped) = syllable_notone.strip_suffix(coda) {
            return Some(stripped.to_string());
        }
        Some(self.syllable.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PronunciationEntry {
    pub id: String,
    pub grapheme: String,
    pub readings: Vec<Reading>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PronunciationBook {
    /// JSONL-sourced entries, fully materialized.
    by_grapheme: HashMap<String, Vec<PronunciationEntry>>,
    by_syllable: HashMap<String, Vec<String>>,
    checked_fallbacks: HashMap<String, Vec<String>>,
    /// Dictionary-sourced readings, stored compactly: every dictionary entry
    /// would otherwise materialize a PronunciationEntry with a formatted id
    /// and several constant heap strings. Readings are reconstructed on
    /// demand via `dictionary_reading`.
    dict_graphemes: Vec<String>,
    dict_grapheme_ids: HashMap<String, u32>,
    dict_syllables: Vec<String>,
    dict_syllable_ids: HashMap<String, u32>,
    /// (grapheme_id, syllable_id) pairs in insertion order.
    dict_pairs: Vec<(u32, u32)>,
    /// grapheme_id -> positions in dict_pairs (insertion order).
    dict_by_grapheme: Vec<Vec<u32>>,
    /// syllable_id -> grapheme_ids (insertion order).
    dict_by_syllable: Vec<Vec<u32>>,
    /// checked-tone base -> syllable_ids (insertion order).
    dict_checked_fallbacks: HashMap<String, Vec<u32>>,
}

#[derive(Debug)]
pub enum PronunciationError {
    Io(std::io::Error),
    Parse { line: usize, message: String },
}

impl Display for PronunciationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PronunciationError::Io(error) => write!(formatter, "{error}"),
            PronunciationError::Parse { line, message } => {
                write!(formatter, "pronunciations line {line}: {message}")
            }
        }
    }
}

impl Error for PronunciationError {}

impl From<std::io::Error> for PronunciationError {
    fn from(error: std::io::Error) -> Self {
        PronunciationError::Io(error)
    }
}

impl PronunciationBook {
    pub fn empty() -> PronunciationBook {
        PronunciationBook {
            by_grapheme: HashMap::new(),
            by_syllable: HashMap::new(),
            checked_fallbacks: HashMap::new(),
            dict_graphemes: Vec::new(),
            dict_grapheme_ids: HashMap::new(),
            dict_syllables: Vec::new(),
            dict_syllable_ids: HashMap::new(),
            dict_pairs: Vec::new(),
            dict_by_grapheme: Vec::new(),
            dict_by_syllable: Vec::new(),
            dict_checked_fallbacks: HashMap::new(),
        }
    }

    pub fn load_jsonl(path: impl AsRef<Path>) -> Result<PronunciationBook, PronunciationError> {
        let content = fs::read_to_string(path)?;
        let mut book = PronunciationBook::empty();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: PronunciationEntry =
                serde_json::from_str(line).map_err(|error| PronunciationError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            book.push(entry);
        }
        Ok(book)
    }

    pub fn extend_from_jsonl(&mut self, path: impl AsRef<Path>) -> Result<(), PronunciationError> {
        let other = PronunciationBook::load_jsonl(path)?;
        for entries in other.by_grapheme.into_values() {
            for entry in entries {
                self.push(entry);
            }
        }
        Ok(())
    }

    pub fn extend_dictionary(&mut self, dictionary: &Dictionary) {
        let mut seen = HashSet::new();

        for entry in dictionary.entries() {
            let syllable = entry.dialect_pinyin.trim();
            if entry.headword.is_empty() || syllable.is_empty() {
                continue;
            }

            if !seen.insert((entry.headword.clone(), syllable.to_string())) {
                continue;
            }

            self.push_dictionary_reading(&entry.headword, syllable);
        }
    }

    /// Register a dictionary-sourced (grapheme, syllable) pair without
    /// materializing a PronunciationEntry; the Reading is rebuilt on demand.
    fn push_dictionary_reading(&mut self, grapheme: &str, syllable: &str) {
        let grapheme_id = match self.dict_grapheme_ids.get(grapheme) {
            Some(&id) => id,
            None => {
                let id = self.dict_graphemes.len() as u32;
                self.dict_graphemes.push(grapheme.to_string());
                self.dict_by_grapheme.push(Vec::new());
                self.dict_grapheme_ids.insert(grapheme.to_string(), id);
                id
            }
        };
        let syllable_id = match self.dict_syllable_ids.get(syllable) {
            Some(&id) => id,
            None => {
                let id = self.dict_syllables.len() as u32;
                self.dict_syllables.push(syllable.to_string());
                self.dict_by_syllable.push(Vec::new());
                self.dict_syllable_ids.insert(syllable.to_string(), id);
                id
            }
        };
        let position = self.dict_pairs.len() as u32;
        self.dict_pairs.push((grapheme_id, syllable_id));
        self.dict_by_grapheme[grapheme_id as usize].push(position);
        self.dict_by_syllable[syllable_id as usize].push(grapheme_id);
        let reading = dictionary_reading(syllable);
        if let Some(base) = reading.checked_base() {
            if base != reading.syllable {
                self.dict_checked_fallbacks
                    .entry(base)
                    .or_default()
                    .push(syllable_id);
            }
        }
    }

    fn push(&mut self, entry: PronunciationEntry) {
        for reading in &entry.readings {
            self.by_syllable
                .entry(reading.syllable.clone())
                .or_default()
                .push(entry.grapheme.clone());
            if let Some(base) = reading.checked_base() {
                if base != reading.syllable {
                    self.checked_fallbacks
                        .entry(base)
                        .or_default()
                        .push(reading.syllable.clone());
                }
            }
        }
        self.by_grapheme
            .entry(entry.grapheme.clone())
            .or_default()
            .push(entry);
    }

    /// Readings for a grapheme. Dictionary-sourced readings (registered
    /// before any JSONL entries at every call site) come first, preserving
    /// the original insertion order; values are constructed on demand.
    pub fn readings_of(&self, grapheme: &str) -> Vec<Reading> {
        let mut readings = Vec::new();
        if let Some(&grapheme_id) = self.dict_grapheme_ids.get(grapheme) {
            for &position in &self.dict_by_grapheme[grapheme_id as usize] {
                let (_, syllable_id) = self.dict_pairs[position as usize];
                readings.push(dictionary_reading(
                    &self.dict_syllables[syllable_id as usize],
                ));
            }
        }
        if let Some(entries) = self.by_grapheme.get(grapheme) {
            for entry in entries {
                readings.extend(entry.readings.iter().cloned());
            }
        }
        readings
    }

    pub fn graphemes_for_syllable(&self, syllable: &str) -> Vec<&String> {
        let mut graphemes = Vec::new();
        if let Some(&syllable_id) = self.dict_syllable_ids.get(syllable) {
            for &grapheme_id in &self.dict_by_syllable[syllable_id as usize] {
                graphemes.push(&self.dict_graphemes[grapheme_id as usize]);
            }
        }
        if let Some(list) = self.by_syllable.get(syllable) {
            graphemes.extend(list.iter());
        }
        graphemes
    }

    pub fn checked_alternatives(&self, base_syllable: &str) -> Vec<&String> {
        let lookup_key = checked_lookup_key(base_syllable);
        let mut deduplicated: Vec<&String> = Vec::new();
        if let Some(list) = self.dict_checked_fallbacks.get(lookup_key.as_str()) {
            for &syllable_id in list {
                let value = &self.dict_syllables[syllable_id as usize];
                if !deduplicated.contains(&value) {
                    deduplicated.push(value);
                }
            }
        }
        if let Some(list) = self.checked_fallbacks.get(lookup_key.as_str()) {
            for value in list {
                if !deduplicated.contains(&value) {
                    deduplicated.push(value);
                }
            }
        }
        deduplicated
    }

    pub fn register_correction(
        &self,
        grapheme: &str,
        observed_syllable: &str,
    ) -> Option<RegisterCorrection> {
        let readings = self.readings_of(grapheme);
        if readings.is_empty() {
            return None;
        }
        let observed = readings
            .iter()
            .find(|reading| reading.syllable == observed_syllable)?;
        let observed_register = observed.register.unwrap_or(Register::Common);
        let alternates: Vec<(&Reading, Register)> = readings
            .iter()
            .filter_map(|reading| {
                if reading.syllable == observed_syllable {
                    return None;
                }
                let register = reading.register.unwrap_or(Register::Common);
                if register == observed_register {
                    return None;
                }
                Some((reading, register))
            })
            .collect();
        if alternates.is_empty() {
            return None;
        }
        Some(RegisterCorrection {
            grapheme: grapheme.to_string(),
            observed_syllable: observed_syllable.to_string(),
            observed_register,
            alternates: alternates
                .into_iter()
                .map(|(reading, register)| RegisterAlternate {
                    syllable: reading.syllable.clone(),
                    register,
                    tone_class: reading.tone_class,
                })
                .collect(),
        })
    }

    pub fn checked_index_size(&self) -> usize {
        self.checked_fallbacks.len() + self.dict_checked_fallbacks.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PronunciationEntry> {
        self.by_grapheme.values().flat_map(|list| list.iter())
    }
}

fn dictionary_reading(syllable: &str) -> Reading {
    Reading {
        syllable: syllable.to_string(),
        tone_class: None,
        register: None,
        scheme: Some("gon-pin".to_string()),
        coda: infer_coda(syllable),
        weight: None,
        source: Some("split-dictionary".to_string()),
        note: None,
    }
}

fn infer_coda(syllable: &str) -> Option<String> {
    let segment = syllable
        .split(|character: char| character == '\'' || character.is_ascii_whitespace())
        .rfind(|item| !item.is_empty())?;
    let segment = segment.trim_end_matches(|character: char| character.is_ascii_digit());
    match segment.chars().last()? {
        't' => Some("t".to_string()),
        'k' => Some("k".to_string()),
        'ʔ' => Some("ʔ".to_string()),
        _ => None,
    }
}

fn checked_lookup_key(syllable: &str) -> String {
    let stripped = syllable.trim_end_matches(|character: char| character.is_ascii_digit());
    if let Some(base) = stripped.strip_suffix(['t', 'k']) {
        return base.to_string();
    }
    stripped.to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisterCorrection {
    pub grapheme: String,
    pub observed_syllable: String,
    pub observed_register: Register,
    pub alternates: Vec<RegisterAlternate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisterAlternate {
    pub syllable: String,
    pub register: Register,
    pub tone_class: Option<u8>,
}
