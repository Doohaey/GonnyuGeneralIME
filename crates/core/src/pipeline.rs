use crate::association_cache::AssociationCache;
use crate::candidate::{owned_text_set, retain_unique_text_and_reading, text_set, CandidateView};
use crate::dictionary::Dictionary;
use crate::mandarin_hints::MandarinHintBook;
use crate::pronunciation::PronunciationBook;
use crate::resources::{RegionResource, ResourceError, ToneClass};
use crate::retrieval::{
    clear_retrieve_cache, format_preedit_display, retrieve_sentence_input,
    retrieve_sentence_input_cached, retrieve_top_with_boosts, retrieve_with_manual_segments,
    segment_boundaries, segment_sentence, RankedCandidate,
};
use crate::slang::{SlangBook, SlangError, TriggerKind};
use crate::syllable::{FuzzyMap, SyllableError};
use crate::user_dict::UserDictionary;
use ahash::AHashMap as HashMap;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateSource {
    Slang,
    Association,
    SlangReverse,
    MandarinHint,
    Pronunciation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateTier {
    Primary,
    Secondary,
    Fallback,
}

impl CandidateTier {
    fn rank(self) -> u8 {
        match self {
            CandidateTier::Primary => 0,
            CandidateTier::Secondary => 1,
            CandidateTier::Fallback => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ComposedCandidate {
    pub text: String,
    pub reading: Option<String>,
    pub source: CandidateSource,
    pub tier: CandidateTier,
    pub weight: f64,
    pub note: Option<String>,
}

impl CandidateView for ComposedCandidate {
    fn text(&self) -> &str {
        &self.text
    }

    fn reading(&self) -> Option<&str> {
        self.reading.as_deref()
    }
}

#[derive(Debug)]
pub struct InputPipeline {
    slang: SlangBook,
    hints: MandarinHintBook,
    pronunciation: PronunciationBook,
    dictionary: Dictionary,
    fuzzy: FuzzyMap,
    tone_values: HashMap<String, u8>,
    user_dict: UserDictionary,
    frequency_boosts: HashMap<String, u64>,
    boosts_path: std::path::PathBuf,
    sentence_prefix_cache: RefCell<HashMap<(String, usize), Vec<RankedCandidate>>>,
}

#[derive(Debug)]
pub enum PipelineError {
    Resource(ResourceError),
    Slang(SlangError),
    Pronunciation(crate::pronunciation::PronunciationError),
    MandarinHint(crate::mandarin_hints::MandarinHintError),
    Dictionary(crate::dictionary::DictionaryError),
    Syllable(SyllableError),
}

impl Display for PipelineError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Resource(error) => write!(formatter, "{error}"),
            PipelineError::Slang(error) => write!(formatter, "{error}"),
            PipelineError::Pronunciation(error) => write!(formatter, "{error}"),
            PipelineError::MandarinHint(error) => write!(formatter, "{error}"),
            PipelineError::Dictionary(error) => write!(formatter, "{error}"),
            PipelineError::Syllable(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PipelineError {}

impl From<ResourceError> for PipelineError {
    fn from(error: ResourceError) -> Self {
        PipelineError::Resource(error)
    }
}

impl From<SlangError> for PipelineError {
    fn from(error: SlangError) -> Self {
        PipelineError::Slang(error)
    }
}

impl From<crate::pronunciation::PronunciationError> for PipelineError {
    fn from(error: crate::pronunciation::PronunciationError) -> Self {
        PipelineError::Pronunciation(error)
    }
}

impl From<crate::mandarin_hints::MandarinHintError> for PipelineError {
    fn from(error: crate::mandarin_hints::MandarinHintError) -> Self {
        PipelineError::MandarinHint(error)
    }
}

impl From<crate::dictionary::DictionaryError> for PipelineError {
    fn from(error: crate::dictionary::DictionaryError) -> Self {
        PipelineError::Dictionary(error)
    }
}

impl From<SyllableError> for PipelineError {
    fn from(error: SyllableError) -> Self {
        PipelineError::Syllable(error)
    }
}

fn file_has_content(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false)
}

impl InputPipeline {
    pub fn empty() -> InputPipeline {
        InputPipeline {
            slang: SlangBook::empty(),
            hints: MandarinHintBook::empty(),
            pronunciation: PronunciationBook::empty(),
            dictionary: Dictionary::empty(),
            fuzzy: FuzzyMap {
                entries: Vec::new(),
            },
            tone_values: HashMap::new(),
            user_dict: UserDictionary::load_or_create(),
            frequency_boosts: HashMap::new(),
            boosts_path: PathBuf::new(),
            sentence_prefix_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn load(resource: &RegionResource) -> Result<InputPipeline, PipelineError> {
        let mut pipeline = InputPipeline::empty();

        // Load dictionary from the configured split files.
        let dict_files: Vec<&str> = [
            resource.config.language.chars.as_deref(),
            resource.config.language.words.as_deref(),
            resource.config.language.gan_chars.as_deref(),
            resource.config.language.gan_words.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !dict_files.is_empty() {
            let paths: Vec<PathBuf> = dict_files
                .iter()
                .map(|relative| resource.root.join(relative))
                .collect();
            let index_dir = Some(resource.root.join("indexes"));
            pipeline.dictionary = Dictionary::load_split_tsvs(&paths, index_dir.as_deref())?;
        }
        // Build auxiliary indices from dictionary in parallel.
        if !pipeline.dictionary.is_empty() {
            let dict = &pipeline.dictionary;
            std::thread::scope(|s| {
                s.spawn(|| pipeline.slang.load_dictionary(dict));
                s.spawn(|| {
                    pipeline.hints.extend_dictionary(dict);
                    pipeline.pronunciation.extend_dictionary(dict);
                });
            });
        }
        if let Some(relative) = resource.config.dictionaries.feature_words.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.slang.load_feature_words_tsv(&path)?;
                pipeline
                    .hints
                    .extend(MandarinHintBook::load_feature_words_tsv(&path)?);
            }
        }
        if let Some(relative) = resource.config.dictionaries.slang.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.slang.load_slang_jsonl(&path)?;
            }
        }
        if let Some(relative) = resource.config.dictionaries.associations.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.slang.load_association_jsonl(&path)?;
            }
        }
        if let Some(relative) = resource.config.dictionaries.mandarin_hints.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.hints.extend(MandarinHintBook::load_jsonl(&path)?);
            }
        }
        if let Some(relative) = resource.config.phonology.pronunciations.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.pronunciation.extend_from_jsonl(&path)?;
            }
        }
        if let Some(relative) = resource.config.phonology.fuzzy_map.as_deref() {
            let path = resource.root.join(relative);
            if path.is_file() && file_has_content(&path) {
                pipeline.fuzzy = FuzzyMap::load_tsv(&path)?;
            }
        }
        pipeline.tone_values = eight_tone_class_map(&resource.config.tone_classes);
        pipeline.boosts_path = pipeline
            .user_dict
            .path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("frequency_boosts.jsonl");
        pipeline.load_frequency_boosts();
        pipeline
            .user_dict
            .prune_existing(|hw| !pipeline.dictionary.by_headword(hw).is_empty());
        // 合并历史 frequency_boosts 到用户词典：自造词的 boost 并入「词频」列
        // （封顶 200000），并从 frequency_boosts 移除（正式词仍保留）。
        pipeline.merge_boosts_into_user_dict();
        // 给尚无词频的自造词赋予 15000–10000 的随机初始词频。
        pipeline.seed_user_dict_frequencies();
        let user_entries: Vec<_> = pipeline.user_dict.entries().cloned().collect();
        pipeline.dictionary.extend_from_entries(user_entries);
        Ok(pipeline)
    }

    pub fn add_user_word(&mut self, headword: &str, pinyin: &str, mandarin_pinyin: &str) -> bool {
        if !self.dictionary.by_headword(headword).is_empty() {
            return false;
        }
        if !self.user_dict.add(headword, pinyin, mandarin_pinyin) {
            return false;
        }
        let entry = self
            .user_dict
            .entries()
            .find(|e| e.headword == headword)
            .cloned();
        if let Some(entry) = entry {
            self.dictionary.extend_from_entries(std::iter::once(entry));
        }
        clear_retrieve_cache();
        self.sentence_prefix_cache.get_mut().clear();
        true
    }

    pub fn clear_user_data(&mut self, clear_words: bool, clear_frequencies: bool) -> bool {
        if !clear_words && !clear_frequencies {
            return false;
        }
        if clear_words && !self.user_dict.clear() {
            return false;
        }
        if clear_frequencies {
            let boosts = HashMap::new();
            if !self.save_frequency_boosts(&boosts) {
                return false;
            }
            self.frequency_boosts = boosts;
        }
        clear_retrieve_cache();
        self.sentence_prefix_cache.get_mut().clear();
        true
    }

    pub fn user_dict_path(&self) -> String {
        self.user_dict.path().to_string_lossy().to_string()
    }

    pub fn boost_frequency(&mut self, headword: &str) -> bool {
        if self.user_dict.contains(headword) {
            if !self.user_dict.boost_frequency(headword) {
                return false;
            }
            if let Some(frequency) = self.user_dict.frequency(headword) {
                self.dictionary.set_user_frequency(headword, frequency);
            }
            if self.frequency_boosts.contains_key(headword) {
                let mut staged = self.frequency_boosts.clone();
                staged.remove(headword);
                if self.save_frequency_boosts(&staged) {
                    self.frequency_boosts = staged;
                }
            }
            clear_retrieve_cache();
            self.sentence_prefix_cache.get_mut().clear();
            return true;
        }
        if self.dictionary.by_headword(headword).is_empty() {
            return false;
        }
        const MAX_BOOST: u64 = 200000;
        let mut staged = self.frequency_boosts.clone();
        let entry = staged.entry(headword.to_string()).or_insert(0);
        if *entry < MAX_BOOST {
            *entry = (*entry + 20000).min(MAX_BOOST);
        }
        if !self.save_frequency_boosts(&staged) {
            return false;
        }
        self.frequency_boosts = staged;
        self.sentence_prefix_cache.get_mut().clear();
        true
    }

    fn load_frequency_boosts(&mut self) {
        const MAX_BOOST: u64 = 200000;
        let Ok(content) = std::fs::read_to_string(&self.boosts_path) else {
            return;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let (Some(word), Some(boost)) =
                    (parsed["word"].as_str(), parsed["boost"].as_u64())
                {
                    // 历史数据可能超过封顶，加载时统一截断到 200000。
                    self.frequency_boosts
                        .insert(word.to_string(), boost.min(MAX_BOOST));
                }
            }
        }
    }

    /// 把历史 frequency_boosts 中属于自造词的 boost 并入用户词典「词频」列
    /// （封顶 200000），并从 frequency_boosts 移除；正式词仍保留在 boosts。
    fn merge_boosts_into_user_dict(&mut self) {
        let mut updates: Vec<(String, u64)> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        for (word, boost) in self.frequency_boosts.iter() {
            if self.user_dict.contains(word) {
                // 自造词：并入用户词典词频
                updates.push((word.clone(), *boost));
                removed.push(word.clone());
            }
        }
        if !updates.is_empty() && self.user_dict.set_frequencies(&updates) == updates.len() {
            let mut staged = self.frequency_boosts.clone();
            for word in &removed {
                staged.remove(word);
            }
            if self.save_frequency_boosts(&staged) {
                self.frequency_boosts = staged;
            }
        }
    }

    /// 给尚无词频（或词频为 0/1）的自造词赋予 15000–10000 的随机初始词频。
    fn seed_user_dict_frequencies(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let updates: Vec<(String, u64)> = self
            .user_dict
            .entries()
            .filter(|e| e.frequency.is_none_or(|f| f <= 1))
            .map(|e| (e.headword.clone(), rng.gen_range(10000..=15000)))
            .collect();
        if !updates.is_empty() {
            self.user_dict.set_frequencies(&updates);
        }
    }

    fn save_frequency_boosts(&self, boosts: &HashMap<String, u64>) -> bool {
        if self.boosts_path.as_os_str().is_empty() {
            return true;
        }
        let parent = self
            .boosts_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
        let mut file = match tempfile::NamedTempFile::new_in(parent) {
            Ok(file) => file,
            Err(_) => return false,
        };
        #[cfg(unix)]
        if file
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .is_err()
        {
            return false;
        }
        use std::io::Write;
        let mut rows: Vec<_> = boosts.iter().filter(|(_, boost)| **boost > 0).collect();
        rows.sort_by(|left, right| left.0.cmp(right.0));
        for (word, boost) in rows {
            let Ok(word_json) = serde_json::to_string(word) else {
                return false;
            };
            if writeln!(file, r#"{{"word":{},"boost":{}}}"#, word_json, boost).is_err() {
                return false;
            }
        }
        if file.flush().is_err() || file.as_file().sync_all().is_err() {
            return false;
        }
        file.persist(&self.boosts_path).is_ok()
    }

    pub fn slang_book(&self) -> &SlangBook {
        &self.slang
    }

    pub fn hint_book(&self) -> &MandarinHintBook {
        &self.hints
    }

    pub fn pronunciation_book(&self) -> &PronunciationBook {
        &self.pronunciation
    }

    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }

    pub fn fuzzy_map(&self) -> &FuzzyMap {
        &self.fuzzy
    }

    pub fn pair_cache(&self) -> &AssociationCache {
        // 关联缓存随词典单例共享; AssociationCache::build 的 slang 参数
        // 本就未被使用, 与原 get_or_init(build(dictionary, slang)) 内容一致。
        self.dictionary.associations()
    }

    pub fn entry_count(&self) -> usize {
        self.dictionary.entries().len()
    }

    pub fn retrieve(&self, input: &str) -> Vec<RankedCandidate> {
        const MAX_CANDIDATES: usize = 100;
        let normalized = crate::dictionary::normalize_pinyin(input);
        let has_separator = normalized.contains(' ') || normalized.contains('\'');
        let cache = self.pair_cache();
        let mut candidates = if has_separator {
            retrieve_with_manual_segments(
                &self.dictionary,
                &self.fuzzy,
                &self.tone_values,
                input,
                None,
            )
        } else if normalized.len() >= 4 {
            let mut prefix_cache = self.sentence_prefix_cache.borrow_mut();
            if prefix_cache.len() > 2048 {
                prefix_cache.clear();
            }
            retrieve_sentence_input_cached(
                &self.dictionary,
                &self.fuzzy,
                &self.tone_values,
                input,
                None,
                &mut prefix_cache,
            )
        } else {
            retrieve_top_with_boosts(
                &self.dictionary,
                &self.fuzzy,
                &self.tone_values,
                input,
                MAX_CANDIDATES,
                &self.frequency_boosts,
            )
        };
        if !has_separator && normalized.len() >= 4 && !self.frequency_boosts.is_empty() {
            let present = text_set(&candidates);
            let mut learned = retrieve_top_with_boosts(
                &self.dictionary,
                &self.fuzzy,
                &self.tone_values,
                input,
                MAX_CANDIDATES,
                &self.frequency_boosts,
            );
            learned.retain(|candidate| {
                self.frequency_boosts.contains_key(&candidate.text)
                    && !present.contains(candidate.text.as_str())
            });
            for mut candidate in learned {
                candidate.consumed_bytes = normalized.len();
                let insert_at = candidates
                    .iter()
                    .rposition(|item| item.consumed_bytes == candidate.consumed_bytes)
                    .map_or(candidates.len(), |index| index + 1);
                candidates.insert(insert_at, candidate);
            }
        }
        if !self.frequency_boosts.is_empty() {
            for c in &mut candidates {
                if !self.user_dict.contains(&c.text) {
                    if let Some(boost) = self.frequency_boosts.get(&c.text) {
                        c.weight += *boost as f64 / 100000.0;
                    }
                }
            }
            // 词频加成参与排序，但保持「多音节完整匹配优先」的边界结构：
            // 候选按 consumed_bytes（音节边界）分成连续组，组内按 boost 稳定排序
            // （同 boost 保持原词频顺序），组间顺序不变（多音节组在前）。
            let mut i = 0usize;
            while i < candidates.len() {
                let consumed = candidates[i].consumed_bytes;
                let mut j = i + 1;
                while j < candidates.len() && candidates[j].consumed_bytes == consumed {
                    j += 1;
                }
                candidates[i..j].sort_by(|a, b| {
                    let a_boost = self.frequency_boosts.get(&a.text).copied().unwrap_or(0);
                    let b_boost = self.frequency_boosts.get(&b.text).copied().unwrap_or(0);
                    b_boost.cmp(&a_boost)
                });
                i = j;
            }
        }
        // Post-process: inject Gan-Mandarin pairs and associations.
        if !cache.is_empty() {
            {
                let mut seen = owned_text_set(&candidates);
                let mut i = 0;
                while i < candidates.len() {
                    let headword = candidates[i].text.clone();

                    // Gan-Mandarin pair: Gan word hit → ensure Mandarin right after
                    let mandarins = cache.mandarins_of_gan(&headword);
                    if !mandarins.is_empty() {
                        let mut insert_at = i + 1;
                        for mw in mandarins {
                            if !seen.contains(mw) {
                                if let Some(entry) = self.dictionary.by_headword(mw).first() {
                                    let mut cand = crate::retrieval::gan_candidate(
                                        &self.dictionary,
                                        entry,
                                        crate::retrieval::RetrievalLayer::GannyuExact,
                                        &self.tone_values,
                                    );
                                    cand.weight = candidates[i].weight - 0.02;
                                    seen.insert(mw.clone());
                                    candidates.insert(insert_at, cand);
                                    insert_at += 1;
                                }
                            } else {
                                // Already in the list but not adjacent — move it right after.
                                let mw_pos =
                                    candidates[insert_at..].iter().position(|c| c.text == *mw);
                                if let Some(offset) = mw_pos {
                                    if offset > 0 {
                                        let moved = candidates.remove(insert_at + offset);
                                        candidates.insert(insert_at, moved);
                                    }
                                }
                            }
                        }
                    }

                    // Gan-Mandarin pair: Mandarin hit → insert Gan before it
                    if !cache.gan_of_mandarin(&headword).is_empty() {
                        let gan_words = cache.gan_of_mandarin(&headword).to_vec();
                        if let Some(first_gan) = gan_words.first() {
                            if !seen.contains(first_gan) {
                                if let Some(gan_entry) =
                                    self.dictionary.by_headword(first_gan).first()
                                {
                                    let mut gan_cand = crate::retrieval::gan_candidate(
                                        &self.dictionary,
                                        gan_entry,
                                        crate::retrieval::RetrievalLayer::GannyuExact,
                                        &self.tone_values,
                                    );
                                    gan_cand.weight = candidates[i].weight + 0.01;
                                    seen.insert(first_gan.clone());
                                    candidates.insert(i, gan_cand);
                                    i += 1; // skip past inserted Gan
                                }
                            }
                        }
                    }

                    // Association groups
                    for assoc in cache.associates_of(&headword) {
                        if seen.contains(assoc) {
                            // Already in list — move it right after if not adjacent.
                            let assoc_pos =
                                candidates[i + 1..].iter().position(|c| &c.text == assoc);
                            if let Some(offset) = assoc_pos {
                                if offset > 0 {
                                    let moved = candidates.remove(i + 1 + offset);
                                    candidates.insert(i + 1, moved);
                                    i += 1;
                                }
                            }
                            continue;
                        }
                        if let Some(entry) = self.dictionary.by_headword(assoc).first() {
                            let mut cand = crate::retrieval::gan_candidate(
                                &self.dictionary,
                                entry,
                                crate::retrieval::RetrievalLayer::Synonym,
                                &self.tone_values,
                            );
                            cand.weight = candidates[i].weight - 0.03;
                            seen.insert(assoc.clone());
                            candidates.insert(i + 1, cand);
                            i += 1;
                        }
                    }

                    i += 1;
                }
            }
        } // if !cache.is_empty()
        candidates
    }

    pub fn retrieve_sentence_input(&self, input: &str) -> Vec<RankedCandidate> {
        retrieve_sentence_input(
            &self.dictionary,
            &self.fuzzy,
            &self.tone_values,
            input,
            None,
        )
    }

    pub fn segment_sentence(&self, input: &str) -> Vec<Vec<RankedCandidate>> {
        segment_sentence(&self.dictionary, &self.fuzzy, &self.tone_values, input)
    }

    pub fn segment_boundaries(&self, input: &str) -> Vec<usize> {
        segment_boundaries(&self.dictionary, &self.fuzzy, &self.tone_values, input)
    }

    pub fn format_preedit_display(&self, input: &str, consumed_bytes: usize) -> String {
        format_preedit_display(
            &self.dictionary,
            &self.fuzzy,
            &self.tone_values,
            input,
            consumed_bytes,
        )
    }

    pub fn compose(&self, input: &str) -> Vec<ComposedCandidate> {
        let mut candidates: Vec<ComposedCandidate> = Vec::new();

        for hit in self.slang.slang_by_trigger(input) {
            let weight = self.weight_for_token(&hit.entry.slang);
            candidates.push(ComposedCandidate {
                text: hit.entry.slang.clone(),
                reading: hit.entry.slang_reading.clone(),
                source: CandidateSource::Slang,
                tier: CandidateTier::Primary,
                weight,
                note: Some(format!(
                    "trigger={} ({})",
                    hit.matched_trigger.text,
                    trigger_kind_label(hit.matched_trigger.kind)
                )),
            });
        }

        for hit in self.slang.association_by_trigger(input) {
            for suggestion in &hit.entry.suggestions {
                let weight = suggestion
                    .weight
                    .unwrap_or_else(|| self.weight_for_token(&suggestion.text));
                let tier = if suggestion.is_fragment {
                    CandidateTier::Secondary
                } else {
                    CandidateTier::Primary
                };
                candidates.push(ComposedCandidate {
                    text: suggestion.text.clone(),
                    reading: suggestion.reading.clone(),
                    source: CandidateSource::Association,
                    tier,
                    weight,
                    note: suggestion.relation.clone(),
                });
            }
        }

        for hit in self.hints.lookup_by_mandarin(input) {
            let weight = self.weight_for_token(&hit.gan);
            candidates.push(ComposedCandidate {
                text: hit.gan.clone(),
                reading: hit.reading.clone(),
                source: CandidateSource::MandarinHint,
                tier: CandidateTier::Secondary,
                weight,
                note: hit.note.clone(),
            });
        }

        for hit in self.slang.slang_reverse(input) {
            for trigger in &hit.triggers {
                if trigger.kind == TriggerKind::GanFragment {
                    continue;
                }
                candidates.push(ComposedCandidate {
                    text: trigger.text.clone(),
                    reading: trigger.reading.clone(),
                    source: CandidateSource::SlangReverse,
                    tier: CandidateTier::Fallback,
                    weight: self.weight_for_token(&trigger.text),
                    note: Some(format!(
                        "slang={} ({})",
                        hit.entry.slang,
                        trigger_kind_label(trigger.kind)
                    )),
                });
            }
        }

        let readings = self.pronunciation.readings_of(input);
        for reading in readings {
            candidates.push(ComposedCandidate {
                text: input.to_string(),
                reading: Some(reading.syllable.clone()),
                source: CandidateSource::Pronunciation,
                tier: CandidateTier::Secondary,
                weight: reading
                    .weight
                    .unwrap_or_else(|| self.weight_for_token(input)),
                note: reading.note.clone(),
            });
        }

        finalize_composed_candidates(candidates)
    }

    fn weight_for_token(&self, _token: &str) -> f64 {
        0.3
    }
}

fn finalize_composed_candidates(mut candidates: Vec<ComposedCandidate>) -> Vec<ComposedCandidate> {
    retain_unique_text_and_reading(&mut candidates);
    candidates.sort_by(|left, right| {
        left.tier
            .rank()
            .cmp(&right.tier.rank())
            .then_with(|| source_rank(left.source).cmp(&source_rank(right.source)))
            .then_with(|| {
                right
                    .weight
                    .partial_cmp(&left.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.text.cmp(&right.text))
    });
    candidates
}

const EIGHT_TONE_NAMES: [(&str, &[&str]); 8] = [
    ("1", &["阴平", "陰平", "平声", "平聲"]),
    ("2", &["阳平", "陽平", "平声", "平聲"]),
    ("3", &["阴上", "陰上", "上声", "上聲"]),
    ("4", &["阳上", "陽上", "上声", "上聲"]),
    ("5", &["阴去", "陰去", "去声", "去聲"]),
    ("6", &["阳去", "陽去", "去声", "去聲"]),
    ("7", &["阴入", "陰入", "入声", "入聲"]),
    ("8", &["阳入", "陽入", "入声", "入聲"]),
];

/// Map an 八调 (traditional eight-tone) digit to the region's tone class
/// by matching tone-class names. Mergers (e.g. 阴上/阳上 collapsing into
/// 上声) are resolved through the fallback names.
fn eight_tone_class_map(tone_classes: &BTreeMap<u8, ToneClass>) -> HashMap<String, u8> {
    let mut by_name: HashMap<&str, u8> = HashMap::new();
    for (class, info) in tone_classes {
        by_name.insert(info.name.as_str(), *class);
    }
    let mut map = HashMap::new();
    for (digit, candidates) in EIGHT_TONE_NAMES {
        for name in candidates {
            if let Some(class) = by_name.get(name) {
                map.insert(digit.to_string(), *class);
                break;
            }
        }
    }
    map
}

fn source_rank(source: CandidateSource) -> u8 {
    match source {
        CandidateSource::Slang => 0,
        CandidateSource::Association => 1,
        CandidateSource::MandarinHint => 2,
        CandidateSource::Pronunciation => 3,
        CandidateSource::SlangReverse => 4,
    }
}

fn trigger_kind_label(kind: TriggerKind) -> &'static str {
    match kind {
        TriggerKind::Mandarin => "mandarin",
        TriggerKind::GanVocab => "gan-vocab",
        TriggerKind::GanFragment => "gan-fragment",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn pipeline_with_temp_user_dict() -> InputPipeline {
        let mut pipeline = InputPipeline::empty();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gannyu_pipeline_{}_{}.tsv",
            std::process::id(),
            nonce
        ));
        pipeline.user_dict = UserDictionary::empty_at(path);
        pipeline.boosts_path = PathBuf::new();
        pipeline
    }

    #[test]
    fn composed_candidate_json_contract_stays_stable() {
        let candidate = ComposedCandidate {
            text: "阿公".to_string(),
            reading: Some("a1 gung1".to_string()),
            source: CandidateSource::Slang,
            tier: CandidateTier::Primary,
            weight: 1.25,
            note: Some("fixture".to_string()),
        };

        assert_eq!(
            serde_json::to_string(&candidate).unwrap(),
            r#"{"text":"阿公","reading":"a1 gung1","source":"slang","tier":"primary","weight":1.25,"note":"fixture"}"#
        );
    }

    #[test]
    fn add_user_word_invalidates_sentence_prefix_cache() {
        let mut pipeline = pipeline_with_temp_user_dict();
        let input = "ce4sisin1ci2";
        pipeline
            .sentence_prefix_cache
            .borrow_mut()
            .insert(("stale".to_string(), 1), Vec::new());

        assert!(pipeline.add_user_word("测试新词", "ce4 si4 sin1 ci2", ""));
        assert!(pipeline.sentence_prefix_cache.borrow().is_empty());
        assert!(pipeline
            .retrieve(input)
            .iter()
            .any(|candidate| candidate.text == "测试新词"));
    }

    #[test]
    fn runtime_user_word_updates_prefix_indexes() {
        let mut pipeline = pipeline_with_temp_user_dict();
        assert!(pipeline.add_user_word("测试新词", "ce4 si4 sin1 ci2", ""));
        let ids = pipeline.dictionary.lookup_prefix_ids("ce4si4");
        assert!(ids.into_iter().any(|id| {
            pipeline
                .dictionary
                .entries()
                .get(id as usize)
                .is_some_and(|entry| entry.headword == "测试新词")
        }));
        assert!(pipeline
            .dictionary
            .initial_match_ids(0, 'c')
            .iter()
            .any(|id| {
                pipeline
                    .dictionary
                    .entries()
                    .get(*id as usize)
                    .is_some_and(|entry| entry.headword == "测试新词")
            }));
    }

    #[test]
    fn boosted_candidate_enters_before_base_limit() {
        let mut pipeline = pipeline_with_temp_user_dict();
        let mut entries = Vec::new();
        for index in 0..101 {
            entries.push(crate::dictionary::DictionaryEntry {
                headword: format!("候选{index:03}"),
                ipa: String::new(),
                dialect_pinyin: "ga".to_string(),
                mandarin_pinyin: String::new(),
                category: "赣".to_string(),
                mandarin_word: String::new(),
                mandarin_word_pinyin: String::new(),
                frequency: Some(1000 - index),
                synonyms: String::new(),
                entry_index: 0,
                new_old: String::new(),
            });
        }
        pipeline.dictionary.extend_from_entries(entries);
        pipeline
            .frequency_boosts
            .insert("候选100".to_string(), 20000);
        let result = pipeline.retrieve("ga");
        assert!(result.iter().any(|candidate| candidate.text == "候选100"));
        assert_eq!(
            result.first().map(|candidate| candidate.text.as_str()),
            Some("候选100")
        );
    }

    #[test]
    fn missing_word_boost_is_rejected() {
        let mut pipeline = pipeline_with_temp_user_dict();
        assert!(!pipeline.boost_frequency("不存在的词"));
        assert!(pipeline.frequency_boosts.is_empty());
    }

    #[test]
    fn boost_user_word_updates_runtime_dictionary() {
        let mut pipeline = pipeline_with_temp_user_dict();
        let input = "ce4sici2";
        assert!(pipeline.add_user_word("测试词", "ce4 si4 ci2", ""));
        let before = pipeline
            .retrieve(input)
            .into_iter()
            .find(|candidate| candidate.text == "测试词")
            .unwrap()
            .weight;

        pipeline.boost_frequency("测试词");
        let after = pipeline
            .retrieve(input)
            .into_iter()
            .find(|candidate| candidate.text == "测试词")
            .unwrap()
            .weight;
        assert!(after > before);
    }

    #[test]
    fn clear_user_data_respects_scopes() {
        let mut pipeline = pipeline_with_temp_user_dict();
        assert!(pipeline.add_user_word("测试词", "ce4 si4 ci2", ""));
        pipeline
            .frequency_boosts
            .insert("正式词".to_string(), 20000);
        assert!(pipeline.clear_user_data(false, true));
        assert!(pipeline.user_dict.contains("测试词"));
        assert!(pipeline.frequency_boosts.is_empty());
        pipeline
            .frequency_boosts
            .insert("正式词".to_string(), 20000);
        assert!(pipeline.clear_user_data(true, false));
        assert!(!pipeline.user_dict.contains("测试词"));
        assert!(!pipeline.frequency_boosts.is_empty());
    }
}
