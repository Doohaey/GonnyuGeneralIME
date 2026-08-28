use crate::dictionary::{distinct_mandarin_words, split_list_values, Dictionary};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct MandarinHintEntry {
    pub mandarin: String,
    pub gan: String,
    #[serde(default)]
    pub reading: Option<String>,
    #[serde(default)]
    pub register: Option<crate::pronunciation::Register>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MandarinHintBook {
    entries: Vec<MandarinHintEntry>,
    by_mandarin: HashMap<String, Vec<u32>>,
    by_gan: HashMap<String, Vec<u32>>,
}

#[derive(Debug)]
pub enum MandarinHintError {
    Io(std::io::Error),
    Parse { line: usize, message: String },
}

impl Display for MandarinHintError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MandarinHintError::Io(error) => write!(formatter, "{error}"),
            MandarinHintError::Parse { line, message } => {
                write!(formatter, "mandarin_hints line {line}: {message}")
            }
        }
    }
}

impl Error for MandarinHintError {}

impl From<std::io::Error> for MandarinHintError {
    fn from(error: std::io::Error) -> Self {
        MandarinHintError::Io(error)
    }
}

impl MandarinHintBook {
    pub fn empty() -> MandarinHintBook {
        MandarinHintBook {
            entries: Vec::new(),
            by_mandarin: HashMap::new(),
            by_gan: HashMap::new(),
        }
    }

    pub fn load_jsonl(path: impl AsRef<Path>) -> Result<MandarinHintBook, MandarinHintError> {
        let content = fs::read_to_string(path)?;
        let mut book = MandarinHintBook::empty();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: MandarinHintEntry =
                serde_json::from_str(line).map_err(|error| MandarinHintError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            book.push(entry);
        }
        Ok(book)
    }

    pub fn load_feature_words_tsv(
        path: impl AsRef<Path>,
    ) -> Result<MandarinHintBook, MandarinHintError> {
        let content = fs::read_to_string(path)?;
        let mut book = MandarinHintBook::empty();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                return Err(MandarinHintError::Parse {
                    line: index + 1,
                    message: "feature_words requires gan, reading, mandarin columns".to_string(),
                });
            }
            let gan = parts[0].trim();
            let reading = split_list_values(parts[1]).into_iter().next();
            for mandarin in split_list_values(parts[2]) {
                book.push(MandarinHintEntry {
                    mandarin,
                    gan: gan.to_string(),
                    reading: reading.clone(),
                    register: None,
                    note: None,
                });
            }
        }
        Ok(book)
    }

    pub fn extend_dictionary(&mut self, dictionary: &Dictionary) {
        let mut seen = HashSet::new();

        for entry in dictionary.entries() {
            if entry.is_mandarin_only() {
                continue;
            }

            let mandarins = distinct_mandarin_words(&entry.headword, &entry.mandarin_word);
            if mandarins.is_empty() {
                continue;
            }

            let reading = entry.dialect_pinyin.trim();
            for mandarin in mandarins {
                if !seen.insert((
                    mandarin.clone(),
                    entry.headword.clone(),
                    reading.to_string(),
                )) {
                    continue;
                }

                self.push(MandarinHintEntry {
                    mandarin,
                    gan: entry.headword.clone(),
                    reading: (!reading.is_empty()).then(|| reading.to_string()),
                    register: None,
                    note: None,
                });
            }
        }
    }

    pub fn extend(&mut self, other: MandarinHintBook) {
        for entry in other.entries {
            self.push(entry);
        }
    }

    fn push(&mut self, entry: MandarinHintEntry) {
        if self
            .by_mandarin
            .get(&entry.mandarin)
            .into_iter()
            .flatten()
            .any(|index| {
                let existing = &self.entries[*index as usize];
                existing.gan == entry.gan
                    && existing.reading == entry.reading
                    && existing.register == entry.register
                    && existing.note == entry.note
            })
        {
            return;
        }
        let index = self.entries.len() as u32;
        self.by_mandarin
            .entry(entry.mandarin.clone())
            .or_default()
            .push(index);
        self.by_gan
            .entry(entry.gan.clone())
            .or_default()
            .push(index);
        self.entries.push(entry);
    }

    pub fn lookup_by_mandarin(&self, term: &str) -> Vec<&MandarinHintEntry> {
        self.by_mandarin
            .get(term)
            .map(|list| {
                list.iter()
                    .map(|index| &self.entries[*index as usize])
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn lookup_by_gan(&self, term: &str) -> Vec<&MandarinHintEntry> {
        self.by_gan
            .get(term)
            .map(|list| {
                list.iter()
                    .map(|index| &self.entries[*index as usize])
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MandarinHintEntry> {
        self.entries.iter()
    }
}
