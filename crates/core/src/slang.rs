use crate::dictionary::{distinct_mandarin_words, split_list_values, Dictionary};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerKind {
    Mandarin,
    GanVocab,
    GanFragment,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SlangTrigger {
    pub text: String,
    pub kind: TriggerKind,
    #[serde(default)]
    pub reading: Option<String>,
    #[serde(default)]
    pub scheme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SlangEntry {
    pub id: String,
    pub slang: String,
    #[serde(default)]
    pub slang_reading: Option<String>,
    #[serde(default)]
    pub slang_scheme: Option<String>,
    #[serde(default)]
    pub local_forms: Vec<String>,
    #[serde(default)]
    pub characters: Vec<String>,
    pub triggers: Vec<SlangTrigger>,
    #[serde(default = "default_reverse_lookup")]
    pub reverse_lookup: bool,
    #[serde(default)]
    pub mandarin_glosses: Vec<String>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_reverse_lookup() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssociationSuggestion {
    pub text: String,
    #[serde(default)]
    pub reading: Option<String>,
    #[serde(default)]
    pub tone_class: Vec<u8>,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub is_fragment: bool,
    #[serde(default)]
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssociationEntry {
    pub trigger: String,
    #[serde(default)]
    pub trigger_kind: Option<TriggerKind>,
    pub suggestions: Vec<AssociationSuggestion>,
}

#[derive(Debug)]
pub enum SlangError {
    Io(std::io::Error),
    Parse { line: usize, message: String },
    FragmentReverseConstraint { slang: String, fragment: String },
}

impl Display for SlangError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SlangError::Io(error) => write!(formatter, "{error}"),
            SlangError::Parse { line, message } => {
                write!(formatter, "slang line {line}: {message}")
            }
            SlangError::FragmentReverseConstraint { slang, fragment } => write!(
                formatter,
                "俚语 {slang} 的 gan-fragment 触发 {fragment} 不能用于反向联想",
            ),
        }
    }
}

impl Error for SlangError {}

impl From<std::io::Error> for SlangError {
    fn from(error: std::io::Error) -> Self {
        SlangError::Io(error)
    }
}

/// Placeholder trigger returned when a fragment-index syllable match has no
/// dedicated SlangTrigger object (fragments are stored compactly to avoid
/// OOM with large dictionaries).
static SLANG_FRAGMENT_PLACEHOLDER: SlangTrigger = SlangTrigger {
    text: String::new(),
    kind: TriggerKind::GanFragment,
    reading: None,
    scheme: None,
};

#[derive(Debug, Clone, Default)]
pub struct SlangBook {
    slangs: Vec<SlangEntry>,
    associations: Vec<AssociationEntry>,
    slang_by_trigger: HashMap<String, Vec<usize>>,
    slang_by_reverse_key: HashMap<String, Vec<usize>>,
    assoc_by_trigger: HashMap<String, Vec<usize>>,
    /// Compact syllable→entry index for large dictionaries that would OOM
    /// if every entry were mirrored with per-syllable SlangTrigger objects.
    fragment_index: HashMap<String, Vec<usize>>,
    /// True when the compact index is active (large dictionary); false when
    /// per-syllable SlangTrigger objects were built for every entry.
    fragment_compact: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlangHit<'a> {
    pub entry: &'a SlangEntry,
    pub matched_trigger: &'a SlangTrigger,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssociationHit<'a> {
    pub entry: &'a AssociationEntry,
    pub matched_text: &'a str,
}

impl SlangBook {
    pub fn empty() -> SlangBook {
        SlangBook {
            slangs: Vec::new(),
            associations: Vec::new(),
            slang_by_trigger: HashMap::new(),
            slang_by_reverse_key: HashMap::new(),
            assoc_by_trigger: HashMap::new(),
            fragment_index: HashMap::new(),
            fragment_compact: false,
        }
    }

    pub fn load_slang_jsonl(&mut self, path: impl AsRef<Path>) -> Result<(), SlangError> {
        let content = fs::read_to_string(path)?;
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: SlangEntry =
                serde_json::from_str(line).map_err(|error| SlangError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            self.push_slang(entry);
        }
        Ok(())
    }

    pub fn load_association_jsonl(&mut self, path: impl AsRef<Path>) -> Result<(), SlangError> {
        let content = fs::read_to_string(path)?;
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: AssociationEntry =
                serde_json::from_str(line).map_err(|error| SlangError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            self.push_association(entry);
        }
        Ok(())
    }

    pub fn load_feature_words_tsv(&mut self, path: impl AsRef<Path>) -> Result<(), SlangError> {
        let content = fs::read_to_string(path)?;
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                return Err(SlangError::Parse {
                    line: index + 1,
                    message: "feature_words requires gan, reading, mandarin columns".to_string(),
                });
            }
            let text = parts[0].trim();
            let readings = split_list_values(parts[1]);
            let mandarins = split_list_values(parts[2]);
            let mut triggers = Vec::new();
            for mandarin in &mandarins {
                triggers.push(SlangTrigger {
                    text: mandarin.clone(),
                    kind: TriggerKind::Mandarin,
                    reading: None,
                    scheme: None,
                });
            }
            for reading in &readings {
                for text in reading_full_forms(reading) {
                    triggers.push(SlangTrigger {
                        text,
                        kind: TriggerKind::GanFragment,
                        reading: Some(reading.clone()),
                        scheme: Some("gon-pin".to_string()),
                    });
                }
                for fragment in reading_fragments(reading) {
                    triggers.push(SlangTrigger {
                        text: fragment,
                        kind: TriggerKind::GanFragment,
                        reading: Some(reading.clone()),
                        scheme: Some("gon-pin".to_string()),
                    });
                }
            }
            self.push_slang(SlangEntry {
                id: format!("feature-{}", index + 1),
                slang: text.to_string(),
                slang_reading: readings.first().cloned(),
                slang_scheme: Some("gon-pin".to_string()),
                local_forms: vec![text.to_string()],
                characters: text
                    .chars()
                    .map(|character| character.to_string())
                    .collect(),
                triggers,
                reverse_lookup: true,
                mandarin_glosses: mandarins,
                example: None,
                source: Some("feature_words.tsv".to_string()),
                note: None,
            });
        }
        Ok(())
    }

    pub fn load_dictionary(&mut self, dictionary: &Dictionary) {
        /// Full per-syllable SlangTrigger objects are built only when the
        /// dictionary is small enough that the memory overhead is acceptable.
        /// Larger dictionaries use the compact `fragment_index` instead,
        /// avoiding OOM while keeping the same `slang_by_trigger` API.
        const COMPACT_THRESHOLD: usize = 200_000;

        let use_compact = dictionary.entries().len() > COMPACT_THRESHOLD;
        if use_compact {
            self.fragment_compact = true;
        }

        let mut seen = HashSet::new();

        for entry in dictionary.entries() {
            if entry.is_mandarin_only() {
                continue;
            }

            let reading = entry.dialect_pinyin.trim();
            let mandarins = distinct_mandarin_words(&entry.headword, &entry.mandarin_word);
            if reading.is_empty() && mandarins.is_empty() {
                continue;
            }

            if !seen.insert((
                entry.headword.clone(),
                reading.to_string(),
                mandarins.join("/"),
            )) {
                continue;
            }

            let mut triggers = Vec::new();
            for mandarin in &mandarins {
                triggers.push(SlangTrigger {
                    text: mandarin.clone(),
                    kind: TriggerKind::Mandarin,
                    reading: None,
                    scheme: None,
                });
            }
            if !reading.is_empty() {
                // GanFragment trigger 的 reading/scheme 没有任何消费方
                // (正向/反查/CLI 只用 text 与 kind; 反查还会过滤掉
                // GanFragment), 词典来源条目不再为每个 trigger 复制整串读音。
                for text in reading_full_forms(reading) {
                    triggers.push(SlangTrigger {
                        text,
                        kind: TriggerKind::GanFragment,
                        reading: None,
                        scheme: None,
                    });
                }
                for fragment in reading_fragments(reading) {
                    if use_compact {
                        self.fragment_index
                            .entry(fragment)
                            .or_default()
                            .push(self.slangs.len());
                    } else {
                        triggers.push(SlangTrigger {
                            text: fragment,
                            kind: TriggerKind::GanFragment,
                            reading: None,
                            scheme: None,
                        });
                    }
                }
            }

            if triggers.is_empty() {
                continue;
            }

            let text = entry.headword.as_str();
            self.push_slang(SlangEntry {
                // id/slang_scheme/source 均无消费方, 词典来源条目留空以省内存。
                id: String::new(),
                slang: text.to_string(),
                slang_reading: (!reading.is_empty()).then(|| reading.to_string()),
                slang_scheme: None,
                local_forms: vec![text.to_string()],
                characters: text
                    .chars()
                    .map(|character| character.to_string())
                    .collect(),
                triggers,
                reverse_lookup: true,
                mandarin_glosses: mandarins,
                example: None,
                source: None,
                note: None,
            });
        }
    }

    fn push_slang(&mut self, entry: SlangEntry) {
        let index = self.slangs.len();
        for trigger in &entry.triggers {
            self.slang_by_trigger
                .entry(trigger.text.clone())
                .or_default()
                .push(index);
        }
        if entry.reverse_lookup {
            let mut reverse_keys: Vec<String> = Vec::new();
            reverse_keys.push(entry.slang.clone());
            for form in &entry.local_forms {
                reverse_keys.push(form.clone());
            }
            for character in &entry.characters {
                reverse_keys.push(character.clone());
            }
            for key in reverse_keys {
                self.slang_by_reverse_key
                    .entry(key)
                    .or_default()
                    .push(index);
            }
        }
        self.slangs.push(entry);
    }

    fn push_association(&mut self, entry: AssociationEntry) {
        let index = self.associations.len();
        self.assoc_by_trigger
            .entry(entry.trigger.clone())
            .or_default()
            .push(index);
        self.associations.push(entry);
    }

    pub fn slang_by_trigger(&self, text: &str) -> Vec<SlangHit<'_>> {
        let mut hits = Vec::new();
        let mut seen_entry: HashSet<usize> = HashSet::new();

        // Exact trigger matches (mandarin words, full reading forms).
        if let Some(indices) = self.slang_by_trigger.get(text) {
            for index in indices {
                if seen_entry.insert(*index) {
                    if let Some(entry) = self.slangs.get(*index) {
                        if let Some(trigger) =
                            entry.triggers.iter().find(|trigger| trigger.text == text)
                        {
                            hits.push(SlangHit {
                                entry,
                                matched_trigger: trigger,
                            });
                        }
                    }
                }
            }
        }

        // Compact fragment-index matches — no per-syllable SlangTrigger stored.
        if let Some(indices) = self.fragment_index.get(text) {
            for index in indices {
                if seen_entry.insert(*index) {
                    if let Some(entry) = self.slangs.get(*index) {
                        hits.push(SlangHit {
                            entry,
                            matched_trigger: &SLANG_FRAGMENT_PLACEHOLDER,
                        });
                    }
                }
            }
        }

        hits
    }

    pub fn slang_reverse(&self, slang_or_character: &str) -> Vec<ReverseHit<'_>> {
        let mut hits = Vec::new();
        if let Some(indices) = self.slang_by_reverse_key.get(slang_or_character) {
            for index in indices {
                if let Some(entry) = self.slangs.get(*index) {
                    let triggers: Vec<&SlangTrigger> = entry
                        .triggers
                        .iter()
                        .filter(|trigger| trigger.kind != TriggerKind::GanFragment)
                        .collect();
                    if triggers.is_empty() {
                        continue;
                    }
                    hits.push(ReverseHit { entry, triggers });
                }
            }
        }
        hits
    }

    pub fn association_by_trigger<'a>(&'a self, text: &'a str) -> Vec<AssociationHit<'a>> {
        let mut hits = Vec::new();
        if let Some(indices) = self.assoc_by_trigger.get(text) {
            for index in indices {
                if let Some(entry) = self.associations.get(*index) {
                    hits.push(AssociationHit {
                        entry,
                        matched_text: text,
                    });
                }
            }
        }
        hits
    }

    pub fn slang_entries(&self) -> &[SlangEntry] {
        &self.slangs
    }

    pub fn association_entries(&self) -> &[AssociationEntry] {
        &self.associations
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReverseHit<'a> {
    pub entry: &'a SlangEntry,
    pub triggers: Vec<&'a SlangTrigger>,
}

fn reading_fragments(reading: &str) -> Vec<String> {
    reading_segments(reading)
        .into_iter()
        .map(strip_tone)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn reading_full_forms(reading: &str) -> Vec<String> {
    let normalized_segments = reading_segments(reading)
        .into_iter()
        .map(strip_tone)
        .collect::<Vec<&str>>();
    let mut forms = vec![reading.to_string()];
    for normalized in [normalized_segments.join("'"), normalized_segments.join(" ")] {
        if !normalized.is_empty() && !forms.contains(&normalized) {
            forms.push(normalized);
        }
    }
    forms
}

fn reading_segments(reading: &str) -> Vec<&str> {
    reading
        .split(|character: char| character == '\'' || character.is_ascii_whitespace())
        .filter(|item| !item.is_empty())
        .collect()
}

fn strip_tone(value: &str) -> &str {
    if let Some(last) = value.chars().last() {
        if last.is_ascii_digit() {
            return &value[..value.len() - last.len_utf8()];
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dictionary;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_fixture(name: &str, content: &str) -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gannyu-slang-{name}-{}-{}",
            std::process::id(),
            unique
        ));
        fs::write(&path, content).expect("fixture write");
        path
    }

    #[test]
    fn load_dictionary_expands_multi_mandarin_words() {
        let body = "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记\n\
青菜\t\tqiang cai\tqing1 cai4\t赣\t蔬菜/菜\tshu1 cai4/cai4\t100\t\t\n";
        let path = write_fixture("multi-mandarin.tsv", body);
        let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
        let mut book = SlangBook::empty();
        book.load_dictionary(&dictionary);

        let shucai_hits = book.slang_by_trigger("蔬菜");
        assert!(shucai_hits.iter().any(|hit| hit.entry.slang == "青菜"));
        let cai_hits = book.slang_by_trigger("菜");
        assert!(cai_hits.iter().any(|hit| hit.entry.slang == "青菜"));
        assert_eq!(book.slang_entries()[0].mandarin_glosses, vec!["蔬菜", "菜"]);
    }
}
