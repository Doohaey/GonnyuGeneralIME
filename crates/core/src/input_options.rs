use crate::dictionary::{pinyin_segments, strip_tone, Dictionary, DictionaryEntry, PairKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadingMode {
    #[default]
    Both,
    New,
    Old,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TranscriptionScheme {
    #[default]
    GonPin,
    Ipa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InputOptions {
    pub annotations: bool,
    pub relations: bool,
    pub synonyms: bool,
    pub fuzzy: bool,
    pub mandarin: bool,
    pub fuzzy_rules: bool,
    pub wenbai: bool,
    pub reading_mode: ReadingMode,
    pub scheme: TranscriptionScheme,
}

impl Default for InputOptions {
    fn default() -> Self {
        Self {
            annotations: true,
            relations: true,
            synonyms: true,
            fuzzy: true,
            mandarin: true,
            fuzzy_rules: true,
            wenbai: true,
            reading_mode: ReadingMode::Both,
            scheme: TranscriptionScheme::GonPin,
        }
    }
}

impl InputOptions {
    pub fn relations_enabled(self) -> bool {
        self.relations
    }

    pub fn mandarin_enabled(self) -> bool {
        self.fuzzy && self.mandarin
    }

    pub fn synonyms_enabled(self) -> bool {
        self.synonyms
    }

    pub fn wenbai_enabled(self) -> bool {
        self.fuzzy && self.wenbai
    }

    pub fn fuzzy_rules_enabled(self) -> bool {
        self.scheme == TranscriptionScheme::GonPin && self.fuzzy && self.fuzzy_rules
    }

    pub(crate) fn dictionary(self, source: &Dictionary) -> Dictionary {
        let mandarin_words: HashSet<&str> = source
            .entries()
            .iter()
            .flat_map(|entry| {
                entry
                    .mandarin_word
                    .split(['/', ';'])
                    .map(str::trim)
                    .filter(move |word| !word.is_empty() && *word != entry.headword.as_ref())
            })
            .collect();
        let entries = source.entries().iter().filter_map(|entry| {
            if !self.relations_enabled()
                && entry.headword.chars().count() > 1
                && (entry.is_mandarin_only() || mandarin_words.contains(entry.headword.as_ref()))
            {
                return None;
            }
            if (self.reading_mode == ReadingMode::New && entry.new_old.starts_with('老'))
                || (self.reading_mode == ReadingMode::Old && entry.new_old.starts_with('新'))
            {
                return None;
            }
            let mut entry: DictionaryEntry = entry.clone();
            if self.reading_mode != ReadingMode::Both {
                let stored = pinyin_segments(&entry.dialect_pinyin);
                let chars: Vec<char> = entry.headword.chars().collect();
                if chars.len() > 1 && chars.len() == stored.len() {
                    let selected: Vec<String> = chars
                        .iter()
                        .zip(stored)
                        .map(|(character, syllable)| {
                            let pair = source.paired_readings.get(character).and_then(|pairs| {
                                pairs.iter().find(|pair| {
                                    pair.kind == PairKind::NewOld
                                        && (strip_tone(syllable) == strip_tone(&pair.first)
                                            || strip_tone(syllable) == strip_tone(&pair.second))
                                })
                            });
                            match pair {
                                Some(pair) => {
                                    let reading = if self.reading_mode == ReadingMode::New {
                                        &pair.first
                                    } else {
                                        &pair.second
                                    };
                                    if syllable.ends_with('0') {
                                        format!("{}0", strip_tone(reading))
                                    } else {
                                        reading.clone()
                                    }
                                }
                                None => syllable.to_owned(),
                            }
                        })
                        .collect();
                    entry.dialect_pinyin = selected.join(" ").into();
                }
            }
            if !self.mandarin_enabled() {
                entry.mandarin_pinyin = "".into();
                entry.mandarin_word_pinyin = "".into();
            }
            if !self.relations_enabled() {
                entry.mandarin_word = "".into();
                entry.mandarin_word_pinyin = "".into();
                if entry.is_mandarin_only() {
                    entry.category = "赣".into();
                }
            }
            if !self.synonyms_enabled() {
                entry.synonyms = "".into();
            }
            Some(entry)
        });
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries(entries);
        dictionary.rebuild_mandarin_word_text_index();
        dictionary.rebuild_new_old_map();
        if self.reading_mode != ReadingMode::Both {
            for (&character, pairs) in &source.paired_readings {
                if let Some(pair) = pairs.iter().find(|pair| pair.kind == PairKind::NewOld) {
                    let reading = if self.reading_mode == ReadingMode::New {
                        &pair.first
                    } else {
                        &pair.second
                    };
                    dictionary
                        .new_old_map
                        .insert(character, (reading.clone(), reading.clone()));
                }
            }
        }
        dictionary.rebuild_multi_reading_augmentation_with_wenbai(self.wenbai_enabled());
        dictionary
    }
}
