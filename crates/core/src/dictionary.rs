use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use bincode::Options;
use fst::Map as FstMap;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DICTIONARY_ID: AtomicU64 = AtomicU64::new(1);

fn next_dictionary_id() -> u64 {
    NEXT_DICTIONARY_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DictionaryEntry {
    pub headword: String,
    pub ipa: String,
    pub dialect_pinyin: String,
    pub mandarin_pinyin: String,
    pub category: String,
    pub mandarin_word: String,
    pub mandarin_word_pinyin: String,
    pub frequency: Option<u64>,
    pub synonyms: String,
    /// O(1) lookup: index of this entry in Dictionary::entries. Set by push_entry.
    pub entry_index: usize,
    /// 新旧标记: "新X" / "老X" or empty.  X is any suffix for pairing.
    pub new_old: String,
}

impl DictionaryEntry {
    pub fn is_mandarin_only(&self) -> bool {
        self.category == "官"
    }

    fn has_distinct_mandarin_word(&self) -> bool {
        !distinct_mandarin_words(&self.headword, &self.mandarin_word).is_empty()
    }

    /// Returns the display label (`"[文]"`, `"[白]"`, or `""`) derived from the
    /// `category` column.  `文` and `白` are treated as literary / colloquial
    /// variants of a Gan reading; everything else (including `赣` and empty)
    /// has no label.
    pub fn register_label(&self) -> &str {
        match self.category.as_str() {
            "文" => "[文]",
            "白" => "[白]",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairKind {
    NewOld,
    Heteronym,
    WenBai,
}

#[derive(Debug)]
pub struct PairedReading {
    pub first: String,
    pub second: String,
    pub kind: PairKind,
}

#[derive(Debug)]
pub struct Dictionary {
    cache_id: u64,
    entries: Vec<DictionaryEntry>,
    dialect_index: HashMap<String, Vec<u32>>,
    mandarin_index: HashMap<String, Vec<u32>>,
    mandarin_word_index: HashMap<String, Vec<u32>>,
    mandarin_word_text_index: HashMap<String, Vec<u32>>,
    headword_index: HashMap<String, Vec<u32>>,
    syllable_index: HashMap<usize, HashMap<String, Vec<u32>>>,
    syllable_trie: crate::trie::Trie,
    initial_index: HashMap<usize, HashMap<char, Vec<u32>>>,
    fst_map: Option<fst::Map<Vec<u8>>>,
    postings: Option<Vec<Vec<u32>>>,
    postings_topk: Option<Vec<Vec<u32>>>,
    /// 成对读音缓存: 字符 → (第一音, 第二音)，新老或本又各取完整一对。
    pub new_old_map: HashMap<char, (String, String)>,
    /// new_old_map 中采用本又显示规则的字；第一音无标签，第二音标 [又]。
    pub heteronym_chars: HashSet<char>,
    /// 全部成对读音缓存（按词典遇到顺序）：词语副标题轻声配对扫描用。
    pub paired_readings: HashMap<char, Vec<PairedReading>>,
    pub syllable_profile: HashMap<String, i64>,
    /// 多读音表: 字符 → 所有已知方言读音(去调), 驱动多读音等权增强。
    pub char_readings: HashMap<String, Vec<String>>,
    /// 关联缓存(同义词组 + 赣官配对): 首次访问时构建一次, 之后跨线程共享;
    /// 词典条目变更(extend_*)时失效, 下次访问重建。
    association_cache: std::sync::OnceLock<crate::association_cache::AssociationCache>,
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeDictionaryCache {
    entries: Vec<DictionaryEntry>,
    headword_index: HashMap<String, Vec<u32>>,
    mandarin_word_text_index: HashMap<String, Vec<u32>>,
    syllable_index: HashMap<usize, HashMap<String, Vec<u32>>>,
    syllable_trie: crate::trie::Trie,
    initial_index: HashMap<usize, HashMap<char, Vec<u32>>>,
    new_old_map: HashMap<char, (String, String)>,
    syllable_profile: HashMap<String, i64>,
}

#[derive(Debug)]
pub enum DictionaryError {
    Io(std::io::Error),
    MissingHeader,
    Row { line: usize, message: String },
    Cache(String),
}

impl Display for DictionaryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DictionaryError::Io(error) => write!(formatter, "{error}"),
            DictionaryError::MissingHeader => write!(formatter, "dictionary has no header row"),
            DictionaryError::Row { line, message } => {
                write!(formatter, "dictionary line {line}: {message}")
            }
            DictionaryError::Cache(message) => write!(formatter, "{message}"),
        }
    }
}

impl Error for DictionaryError {}

impl From<std::io::Error> for DictionaryError {
    fn from(error: std::io::Error) -> Self {
        DictionaryError::Io(error)
    }
}

const COLUMNS: [&str; 10] = [
    "本词",
    "国际音标",
    "方言拼音",
    "汉语拼音",
    "词汇属性",
    "对应官话词",
    "官话拼音",
    "词频",
    "同义词",
    "新旧标记",
];

pub(crate) fn normalize_pinyin(value: &str) -> String {
    let compact = value.trim().replace(['’', '`'], "'");
    let mut output = String::with_capacity(compact.len());
    let mut pending_space = false;
    for character in compact.chars() {
        if character.is_ascii_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if character == '\'' {
            while output.ends_with(' ') {
                output.pop();
            }
            if !output.ends_with('\'') {
                output.push('\'');
            }
            pending_space = false;
            continue;
        }
        if pending_space && !output.ends_with('\'') {
            output.push(' ');
        }
        pending_space = false;
        output.push(character.to_ascii_lowercase());
    }
    output.trim_matches([' ', '\'']).to_string()
}

pub(crate) fn strip_tone(value: &str) -> &str {
    value.trim_end_matches(|character: char| character.is_ascii_digit())
}

/// e ↔ ĕ/ě 互模糊：e 与带第三声/短音符的 e 匹配。
fn normalize_e_tone(input: &str) -> String {
    match input {
        "e" => "e\u{0306}".to_string(),
        "ě" | "e\u{0306}" | "ĕ" => "e".to_string(),
        _ => input.to_string(),
    }
}

fn alternate_e_syllable(key: &str, mapping: &str) -> String {
    if key.len() == 1 {
        return normalize_e_tone(key);
    }
    let replaced = key.replace('e', mapping);
    if replaced != *key {
        return replaced;
    }
    key.to_string()
}

fn push_unique(forms: &mut Vec<String>, candidate: String) {
    if !candidate.is_empty() && !forms.contains(&candidate) {
        forms.push(candidate);
    }
}

/// Collect every known tone-stripped dialect reading of each character from
/// all entries (single- and multi-character). Drives multi-reading support:
/// every reading of a character must rank equally (等权) for word candidates.
fn collect_all_char_readings(entries: &[DictionaryEntry]) -> HashMap<String, Vec<String>> {
    let mut char_readings: HashMap<String, Vec<String>> = HashMap::new();
    let mut seen: HashMap<String, HashSet<String>> = HashMap::new();
    for entry in entries {
        let syllables: Vec<String> = pinyin_segments(&entry.dialect_pinyin)
            .iter()
            .map(|s| strip_tone(s).to_string())
            .collect();
        for (pos, ch) in entry.headword.chars().enumerate() {
            if let Some(syl) = syllables.get(pos) {
                let ch_str = ch.to_string();
                if seen.entry(ch_str.clone()).or_default().insert(syl.clone()) {
                    char_readings.entry(ch_str).or_default().push(syl.clone());
                }
            }
        }
    }
    char_readings
}

// Only explicit 新老、本又、文白 pairs participate in alternate-reading lookup.
fn collect_char_readings(entries: &[DictionaryEntry]) -> HashMap<String, Vec<String>> {
    let mut marks: HashMap<(String, String), u8> = HashMap::new();
    let mut registers: HashMap<String, u8> = HashMap::new();
    for entry in entries {
        if entry.headword.chars().count() != 1 {
            continue;
        }
        let tag = entry.new_old.as_str();
        let bit = match tag.chars().next() {
            Some('新') => 1,
            Some('老') => 2,
            Some('本') => 4,
            Some('又') => 8,
            _ => 0,
        };
        if bit != 0 {
            let suffix: String = tag.chars().skip(1).collect();
            *marks.entry((entry.headword.clone(), suffix)).or_default() |= bit;
        }
        let bit = match entry.category.as_str() {
            "文" => 1,
            "白" => 2,
            _ => 0,
        };
        if bit != 0 {
            *registers.entry(entry.headword.clone()).or_default() |= bit;
        }
    }
    let mut allowed: HashSet<String> = registers
        .into_iter()
        .filter_map(|(ch, bits)| (bits == 3).then_some(ch))
        .collect();
    allowed.extend(
        marks
            .into_iter()
            .filter_map(|((ch, _), bits)| (bits == 3 || bits == 12).then_some(ch)),
    );
    collect_all_char_readings(entries)
        .into_iter()
        .filter(|(ch, _)| allowed.contains(ch))
        .collect()
}

/// Cap on exact lookup forms generated per entry by multi-reading cartesian
/// expansion (space-for-time). Entries whose per-position reading product
/// exceeds this stay reachable via `syllable_index` only.
const MAX_MULTI_READING_FORMS: usize = 64;

/// Cartesian product of per-position readings for a multi-character entry.
/// Returns an empty vec for single-character entries, entries whose syllable
/// count does not match their character count, or entries whose expansion
/// would exceed `MAX_MULTI_READING_FORMS`.
pub(crate) fn paired_alternates_for_stored(
    ch: &str,
    stored: &str,
    paired_readings: &HashMap<char, Vec<PairedReading>>,
) -> Option<Vec<String>> {
    let ch = ch.chars().next()?;
    let pair = paired_readings
        .get(&ch)?
        .iter()
        .find(|pair| stored == strip_tone(&pair.first) || stored == strip_tone(&pair.second))?;
    let mut readings = vec![strip_tone(&pair.first).to_string()];
    let second = strip_tone(&pair.second).to_string();
    if !readings.contains(&second) {
        readings.push(second);
    }
    Some(readings)
}

fn multi_reading_combinations(
    entry: &DictionaryEntry,
    paired_readings: &HashMap<char, Vec<PairedReading>>,
) -> Vec<Vec<String>> {
    let chars: Vec<char> = entry.headword.chars().collect();
    if chars.len() <= 1 {
        return Vec::new();
    }
    let syllables: Vec<&str> = pinyin_segments(&entry.dialect_pinyin);
    if syllables.len() != chars.len() {
        return Vec::new();
    }
    let mut per_position: Vec<Vec<String>> = Vec::with_capacity(chars.len());
    let mut total = 1usize;
    for (pos, &ch) in chars.iter().enumerate() {
        let ch_str: String = ch.into();
        let stored = strip_tone(syllables[pos]).to_string();
        let readings = paired_alternates_for_stored(&ch_str, &stored, paired_readings)
            .unwrap_or_else(|| vec![stored]);
        total = total.saturating_mul(readings.len());
        if total > MAX_MULTI_READING_FORMS {
            return Vec::new();
        }
        per_position.push(readings);
    }
    let mut combinations: Vec<Vec<String>> = vec![Vec::with_capacity(chars.len())];
    for readings in per_position {
        let mut next = Vec::with_capacity(combinations.len() * readings.len());
        for combo in &combinations {
            for reading in &readings {
                let mut extended = combo.clone();
                extended.push(reading.clone());
                next.push(extended);
            }
        }
        combinations = next;
    }
    combinations
}

pub(crate) fn pinyin_segments(value: &str) -> Vec<&str> {
    value
        .split(|character: char| character == '\'' || character.is_ascii_whitespace())
        .filter(|item| !item.is_empty())
        .collect()
}

fn pinyin_lookup_forms(value: &str) -> Vec<String> {
    let normalized = normalize_pinyin(value);
    let segments = pinyin_segments(&normalized);
    if segments.is_empty() {
        return Vec::new();
    }
    let stripped = segments
        .iter()
        .map(|segment| strip_tone(segment))
        .collect::<Vec<&str>>();
    let mut forms = Vec::new();
    push_unique(&mut forms, normalized.clone());
    push_unique(&mut forms, segments.join(""));
    push_unique(&mut forms, stripped.join(" "));
    push_unique(&mut forms, stripped.join(""));
    forms
}

pub(crate) fn split_list_values(value: &str) -> Vec<String> {
    let mut values = Vec::new();
    for part in value
        .split(['/', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let candidate = part.to_string();
        if !values.contains(&candidate) {
            values.push(candidate);
        }
    }
    values
}

pub(crate) fn distinct_mandarin_words(headword: &str, value: &str) -> Vec<String> {
    split_list_values(value)
        .into_iter()
        .filter(|item| item != headword)
        .collect()
}

fn dialect_lookup_forms(value: &str) -> Vec<String> {
    let mut forms = Vec::new();
    for variant in normalize_pinyin(value)
        .split(['&', '/', ';'])
        .filter(|item| !item.is_empty())
    {
        let segments = pinyin_segments(variant);
        if segments.is_empty() {
            continue;
        }
        let stripped = segments
            .iter()
            .map(|segment| strip_tone(segment))
            .collect::<Vec<&str>>();
        push_unique(&mut forms, variant.to_string());
        push_unique(&mut forms, segments.join(""));
        push_unique(&mut forms, stripped.join("'"));
        push_unique(&mut forms, stripped.join(" "));
        push_unique(&mut forms, stripped.join(""));
    }
    forms
}

fn add_index_entry(index: &mut HashMap<String, Vec<u32>>, forms: Vec<String>, entry_index: usize) {
    let idx = entry_index as u32;
    for form in forms {
        index.entry(form).or_default().push(idx);
    }
}

fn add_text_index_entries(
    index: &mut HashMap<String, Vec<u32>>,
    values: impl IntoIterator<Item = String>,
    entry_index: usize,
) {
    let idx = entry_index as u32;
    for value in values {
        index.entry(value).or_default().push(idx);
    }
}

fn file_has_content(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false)
}

type PrebuiltIndexes = (
    Option<FstMap<Vec<u8>>>,
    Option<Vec<Vec<u32>>>,
    Option<Vec<Vec<u32>>>,
);

fn load_prebuilt_indexes(index_dir: Option<&Path>) -> PrebuiltIndexes {
    let Some(index_dir) = index_dir else {
        return (None, None, None);
    };
    let fst_path = index_dir.join("fst_map.bin");
    let postings_path = index_dir.join("postings.bin");
    let topk_path = index_dir.join("topk.bin");
    let mut fst_map_opt: Option<FstMap<Vec<u8>>> = None;
    let mut postings_opt: Option<Vec<Vec<u32>>> = None;
    let mut topk_opt: Option<Vec<Vec<u32>>> = None;
    if fst_path.exists() && postings_path.exists() {
        if let Ok(data) = std::fs::read(&fst_path) {
            if let Ok(map) = FstMap::new(data) {
                fst_map_opt = Some(map);
                if let Ok(file) = File::open(&postings_path) {
                    let mut reader = BufReader::new(file);
                    if let Ok(posting_blob) =
                        bincode::deserialize_from::<_, Vec<Vec<u32>>>(&mut reader)
                    {
                        postings_opt = Some(posting_blob);
                    }
                }
                if topk_path.exists() {
                    if let Ok(file) = File::open(&topk_path) {
                        let mut reader = BufReader::new(file);
                        if let Ok(blob) = bincode::deserialize_from::<_, Vec<Vec<u32>>>(&mut reader)
                        {
                            topk_opt = Some(blob);
                        }
                    }
                }
            }
        }
    }
    (fst_map_opt, postings_opt, topk_opt)
}

fn cache_is_fresh(cache_path: &Path, source_paths: &[std::path::PathBuf]) -> bool {
    let Ok(cache_meta) = std::fs::metadata(cache_path) else {
        return false;
    };
    let Ok(cache_modified) = cache_meta.modified() else {
        return false;
    };
    for path in source_paths {
        let Ok(source_meta) = std::fs::metadata(path) else {
            continue;
        };
        let Ok(source_modified) = source_meta.modified() else {
            continue;
        };
        if source_modified > cache_modified {
            return false;
        }
    }
    true
}

fn runtime_cache_path(paths: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    paths.iter().find_map(|path| {
        path.parent()
            .map(|parent| parent.join("dictionary_runtime_cache.zst"))
    })
}

impl Dictionary {
    pub fn empty() -> Dictionary {
        Dictionary {
            cache_id: next_dictionary_id(),
            entries: Vec::new(),
            dialect_index: HashMap::new(),
            mandarin_index: HashMap::new(),
            mandarin_word_index: HashMap::new(),
            mandarin_word_text_index: HashMap::new(),
            headword_index: HashMap::new(),
            syllable_index: HashMap::new(),
            syllable_trie: crate::trie::Trie::new(),
            initial_index: HashMap::new(),
            fst_map: None,
            postings: None,
            postings_topk: None,
            new_old_map: HashMap::new(),
            heteronym_chars: HashSet::new(),
            paired_readings: HashMap::new(),
            syllable_profile: HashMap::new(),
            char_readings: HashMap::new(),
            association_cache: std::sync::OnceLock::new(),
        }
    }

    /// 关联缓存(同义词组 + 赣官配对): 首次访问惰性构建, 之后只读共享,
    /// rayon 工作线程无需各自重建。内容与词典条目一一对应。
    pub fn associations(&self) -> &crate::association_cache::AssociationCache {
        self.association_cache
            .get_or_init(|| crate::association_cache::AssociationCache::build(self, None))
    }

    pub fn load_tsv(path: impl AsRef<Path>) -> Result<Dictionary, DictionaryError> {
        Self::load_tsv_internal(path.as_ref(), true)
    }

    fn load_tsv_internal(
        path: &Path,
        build_exact_indices: bool,
    ) -> Result<Dictionary, DictionaryError> {
        let content = fs::read_to_string(path)?;
        let mut lines = content.lines().enumerate();
        let mut indices: Option<[usize; 10]> = None;
        for (index, raw) in lines.by_ref() {
            let line = raw.trim_end_matches(['\r', '\n']);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let header: Vec<&str> = line.split('\t').map(str::trim).collect();
            let mut resolved = [0usize; 10];
            for (slot, column) in COLUMNS.iter().enumerate() {
                let position = header.iter().position(|value| value == column).ok_or(
                    DictionaryError::Row {
                        line: index + 1,
                        message: format!("missing column: {column}"),
                    },
                )?;
                resolved[slot] = position;
            }
            indices = Some(resolved);
            break;
        }
        let indices = indices.ok_or(DictionaryError::MissingHeader)?;

        let mut entries = Vec::new();
        let mut dialect_index = HashMap::new();
        let mut mandarin_index = HashMap::new();
        let mut mandarin_word_index = HashMap::new();
        let mut mandarin_word_text_index: HashMap<String, Vec<u32>> = HashMap::new();
        let mut headword_index = HashMap::new();
        let mut syllable_index: HashMap<usize, HashMap<String, Vec<u32>>> = HashMap::new();
        // Collect single-character alternative readings for multi-reading support.
        let mut char_readings: HashMap<String, Vec<String>> = HashMap::new();
        let mut char_readings_seen: HashMap<String, HashSet<String>> = HashMap::new();
        for (_, raw) in lines {
            let line = raw.trim_end_matches(['\r', '\n']);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            let get = |slot: usize| -> &str {
                columns
                    .get(indices[slot])
                    .map(|value| value.trim())
                    .unwrap_or("")
            };
            let headword = get(0).to_string();
            if headword.is_empty() {
                continue;
            }
            // Collect per-character readings from ALL entries (single and multi-char)
            // for multi-reading augmentation of syllable_index.
            let pinyin = normalize_pinyin(get(2));
            let syllables: Vec<String> = pinyin_segments(&pinyin)
                .iter()
                .map(|s| strip_tone(s).to_string())
                .collect();
            for (pos, ch) in headword.chars().enumerate() {
                if let Some(syl) = syllables.get(pos) {
                    let ch_str = ch.to_string();
                    if char_readings_seen
                        .entry(ch_str.clone())
                        .or_default()
                        .insert(syl.clone())
                    {
                        char_readings.entry(ch_str).or_default().push(syl.clone());
                    }
                }
            }
            let frequency = {
                let raw_freq = get(7);
                if raw_freq.is_empty() {
                    None
                } else {
                    raw_freq.parse::<u64>().ok()
                }
            };
            let entry_index = entries.len();
            let entry = DictionaryEntry {
                headword,
                ipa: get(1).to_string(),
                dialect_pinyin: normalize_pinyin(get(2)),
                mandarin_pinyin: normalize_pinyin(get(3)),
                category: get(4).to_string(),
                mandarin_word: get(5).to_string(),
                mandarin_word_pinyin: normalize_pinyin(get(6)),
                frequency,
                synonyms: get(8).to_string(),
                entry_index,
                new_old: get(9).to_string(),
            };
            headword_index
                .entry(entry.headword.clone())
                .or_insert_with(Vec::new)
                .push(entry_index as u32);
            if build_exact_indices {
                add_index_entry(
                    &mut dialect_index,
                    dialect_lookup_forms(&entry.dialect_pinyin),
                    entry_index,
                );
                add_index_entry(
                    &mut mandarin_index,
                    pinyin_lookup_forms(&entry.mandarin_pinyin),
                    entry_index,
                );
                if entry.has_distinct_mandarin_word() {
                    for mandarin_word_pinyin in split_list_values(&entry.mandarin_word_pinyin) {
                        add_index_entry(
                            &mut mandarin_word_index,
                            pinyin_lookup_forms(&mandarin_word_pinyin),
                            entry_index,
                        );
                    }
                }
            }
            if entry.has_distinct_mandarin_word() {
                add_text_index_entries(
                    &mut mandarin_word_text_index,
                    distinct_mandarin_words(&entry.headword, &entry.mandarin_word),
                    entry_index,
                );
            }
            // ── Per-position syllable index for mixed Gan/Mandarin pinyin ──
            {
                let dialect_syllables: Vec<String> = pinyin_segments(&entry.dialect_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let mandarin_syllables: Vec<String> = pinyin_segments(&entry.mandarin_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let max_len = dialect_syllables.len().max(mandarin_syllables.len());
                for pos in 0..max_len {
                    let mut syllables_at_pos: Vec<String> = Vec::new();
                    if let Some(s) = dialect_syllables.get(pos) {
                        push_unique(&mut syllables_at_pos, s.clone());
                    }
                    if let Some(s) = mandarin_syllables.get(pos) {
                        push_unique(&mut syllables_at_pos, s.clone());
                    }
                    for syl in &syllables_at_pos {
                        syllable_index
                            .entry(pos)
                            .or_default()
                            .entry(syl.clone())
                            .or_default()
                            .push(entry_index as u32);
                    }
                }
            }
            entries.push(entry);
        }

        // At this point, entries contains all entries and syllable_index is built.
        // Augment syllable_index and build syllable_trie from collected readings
        // and the previously generated forms.
        // Use a per-entry set to avoid O(n) Vec::contains checks.
        for (entry_index, entry) in entries.iter().enumerate() {
            let chars: Vec<char> = entry.headword.chars().collect();
            if chars.len() > 1 {
                let mut seen: HashSet<(usize, String)> = HashSet::new();
                for (pos, &ch) in chars.iter().enumerate() {
                    let ch_str: String = ch.into();
                    if let Some(readings) = char_readings.get(&ch_str) {
                        for reading in readings {
                            if seen.insert((pos, reading.clone())) {
                                let pos_map = syllable_index.entry(pos).or_default();
                                pos_map
                                    .entry(reading.clone())
                                    .or_default()
                                    .push(entry_index as u32);
                            }
                        }
                    }
                }
            }
        }

        let mut syllable_profile: HashMap<String, i64> = HashMap::new();
        for pos_map in syllable_index.values() {
            for (syl, ids) in pos_map {
                let count = ids.len().min(2000) as i64;
                *syllable_profile.entry(syl.clone()).or_insert(0) += count;
            }
        }
        for score in syllable_profile.values_mut() {
            *score = (*score).min(50_000);
        }

        // Build initial-letter index for O(1) first-keystroke lookups.
        // Use temporary HashSet per (position, char) to avoid O(n²) Vec::contains.
        let mut initial_index: HashMap<usize, HashMap<char, Vec<u32>>> = HashMap::new();
        for (pos, pos_map) in &syllable_index {
            let mut char_sets: HashMap<char, HashSet<u32>> = HashMap::new();
            for (syl, ids) in pos_map {
                if let Some(first_ch) = syl.chars().find(|c| c.is_ascii_alphabetic()) {
                    let ch = first_ch.to_ascii_lowercase();
                    char_sets.entry(ch).or_default().extend(ids);
                }
            }
            let pos_initial = initial_index.entry(*pos).or_default();
            for (ch, id_set) in char_sets {
                pos_initial.insert(ch, id_set.into_iter().collect());
            }
        }

        // Build trie from entries' concatenated syllable forms.
        let mut trie = crate::trie::Trie::new();
        for (entry_index, entry) in entries.iter().enumerate() {
            let dialect_forms = pinyin_segments(&entry.dialect_pinyin)
                .iter()
                .map(|s| strip_tone(s).to_string())
                .collect::<Vec<String>>()
                .join("");
            let mandarin_forms = pinyin_segments(&entry.mandarin_pinyin)
                .iter()
                .map(|s| strip_tone(s).to_string())
                .collect::<Vec<String>>()
                .join("");
            if !dialect_forms.is_empty() {
                trie.insert(&dialect_forms.to_ascii_lowercase(), entry_index);
            }
            if !mandarin_forms.is_empty() {
                trie.insert(&mandarin_forms.to_ascii_lowercase(), entry_index);
            }
            // also insert pinyin_lookup_forms / dialect_lookup_forms variants
            for form in pinyin_lookup_forms(&entry.mandarin_pinyin) {
                let key = form.replace(' ', "").to_ascii_lowercase();
                if !key.is_empty() {
                    trie.insert(&key, entry_index);
                }
            }
            for form in dialect_lookup_forms(&entry.dialect_pinyin) {
                let key = form.replace(' ', "").to_ascii_lowercase();
                if !key.is_empty() {
                    trie.insert(&key, entry_index);
                }
            }
        }

        let mut dictionary = Dictionary {
            cache_id: next_dictionary_id(),
            entries,
            dialect_index,
            mandarin_index,
            mandarin_word_index,
            mandarin_word_text_index,
            headword_index,
            syllable_index,
            initial_index,
            syllable_trie: trie,
            syllable_profile,
            fst_map: None,
            postings: None,
            postings_topk: None,
            new_old_map: HashMap::new(),
            heteronym_chars: HashSet::new(),
            paired_readings: HashMap::new(),
            char_readings,
            association_cache: std::sync::OnceLock::new(),
        };
        dictionary.rebuild_mandarin_word_text_index();
        Ok(dictionary)
    }

    fn to_runtime_cache(&self) -> RuntimeDictionaryCache {
        RuntimeDictionaryCache {
            entries: self.entries.clone(),
            headword_index: self.headword_index.clone(),
            mandarin_word_text_index: self.mandarin_word_text_index.clone(),
            syllable_index: self.syllable_index.clone(),
            syllable_trie: self.syllable_trie.clone(),
            initial_index: self.initial_index.clone(),
            new_old_map: self.new_old_map.clone(),
            syllable_profile: self.syllable_profile.clone(),
        }
    }

    pub fn write_runtime_cache(&self, path: impl AsRef<Path>) -> Result<(), DictionaryError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Serialize + gzip into memory, then scramble the bytes before writing
        // so the on-disk cache is not trivially decompressible.
        let mut compressed = Vec::new();
        {
            let mut writer =
                flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::fast());
            let config = bincode::DefaultOptions::new()
                .with_native_endian()
                .with_varint_encoding();
            config
                .serialize_into(&mut writer, &self.to_runtime_cache())
                .map_err(|error| {
                    DictionaryError::Cache(format!("write dictionary cache: {error}"))
                })?;
            writer
                .finish()
                .map_err(|error| DictionaryError::Cache(format!("finish cache writer: {error}")))?;
        }
        crate::cache_obfuscation::scramble(&mut compressed);
        std::fs::write(path, &compressed)
            .map_err(|error| DictionaryError::Cache(format!("write cache file: {error}")))?;
        Ok(())
    }

    pub fn load_runtime_cache(path: impl AsRef<Path>) -> Result<Dictionary, DictionaryError> {
        let mut bytes = std::fs::read(path)
            .map_err(|error| DictionaryError::Cache(format!("read cache file: {error}")))?;
        crate::cache_obfuscation::scramble(&mut bytes);
        let mut reader = flate2::read::GzDecoder::new(&bytes[..]);
        let config = bincode::DefaultOptions::new()
            .with_native_endian()
            .with_varint_encoding()
            .with_limit(u64::MAX);
        let mut cache: RuntimeDictionaryCache = config
            .deserialize_from(&mut reader)
            .map_err(|error| DictionaryError::Cache(format!("load dictionary cache: {error}")))?;
        for (index, entry) in cache.entries.iter_mut().enumerate() {
            entry.entry_index = index;
        }
        let char_readings = collect_char_readings(&cache.entries);
        let mut dictionary = Dictionary {
            cache_id: next_dictionary_id(),
            entries: cache.entries,
            dialect_index: HashMap::new(),
            mandarin_index: HashMap::new(),
            mandarin_word_index: HashMap::new(),
            mandarin_word_text_index: cache.mandarin_word_text_index,
            headword_index: cache.headword_index,
            syllable_index: cache.syllable_index,
            syllable_trie: cache.syllable_trie,
            initial_index: cache.initial_index,
            fst_map: None,
            postings: None,
            postings_topk: None,
            new_old_map: cache.new_old_map,
            heteronym_chars: HashSet::new(),
            paired_readings: HashMap::new(),
            syllable_profile: cache.syllable_profile,
            char_readings,
            association_cache: std::sync::OnceLock::new(),
        };
        dictionary.rebuild_mandarin_word_text_index();
        // Runtime cache predates 本又显示信息; rebuild both paired-reading maps
        // from entries so cached and uncached dictionaries render identically.
        dictionary.rebuild_new_old_map();
        Ok(dictionary)
    }

    fn rebuild_mandarin_word_text_index(&mut self) {
        self.mandarin_word_text_index.clear();
        for (entry_index, entry) in self.entries.iter().enumerate() {
            add_text_index_entries(
                &mut self.mandarin_word_text_index,
                distinct_mandarin_words(&entry.headword, &entry.mandarin_word),
                entry_index,
            );
        }
    }

    fn rebuild_exact_lookup_indices(&mut self) {
        self.dialect_index.clear();
        self.mandarin_index.clear();
        self.mandarin_word_index.clear();
        self.rebuild_mandarin_word_text_index();
        for (entry_index, entry) in self.entries.iter().enumerate() {
            add_index_entry(
                &mut self.dialect_index,
                dialect_lookup_forms(&entry.dialect_pinyin),
                entry_index,
            );
            add_index_entry(
                &mut self.mandarin_index,
                pinyin_lookup_forms(&entry.mandarin_pinyin),
                entry_index,
            );
            if entry.has_distinct_mandarin_word() {
                for mandarin_word_pinyin in split_list_values(&entry.mandarin_word_pinyin) {
                    add_index_entry(
                        &mut self.mandarin_word_index,
                        pinyin_lookup_forms(&mandarin_word_pinyin),
                        entry_index,
                    );
                }
            }
        }
        // Multi-reading 等权: re-derive char_readings and register every
        // multi-reading full form into the exact dialect index, so cache
        // loads behave identically to freshly parsed dictionaries.
        self.char_readings = collect_char_readings(&self.entries);
        self.augment_dialect_index_multi_readings();
        self.shrink_indices();
    }

    /// Register every multi-reading full form of each multi-character entry
    /// into the exact dialect index (cartesian product of per-position
    /// readings, capped at `MAX_MULTI_READING_FORMS` forms per entry), so a
    /// query typed with any combination of a character's readings is an
    /// exact dialect hit, equal-weight with the stored reading.
    fn augment_dialect_index_multi_readings(&mut self) {
        let paired_readings = &self.paired_readings;
        for (entry_index, entry) in self.entries.iter().enumerate() {
            let combinations = multi_reading_combinations(entry, paired_readings);
            if combinations.is_empty() {
                continue;
            }
            let mut seen_forms: HashSet<String> = dialect_lookup_forms(&entry.dialect_pinyin)
                .into_iter()
                .collect();
            for combo in combinations {
                let mut forms = dialect_lookup_forms(&combo.join(" "));
                forms.retain(|form| seen_forms.insert(form.clone()));
                add_index_entry(&mut self.dialect_index, forms, entry_index);
            }
        }
    }

    fn shrink_indices(&mut self) {
        self.dialect_index.shrink_to_fit();
        self.mandarin_index.shrink_to_fit();
        self.mandarin_word_index.shrink_to_fit();
        self.mandarin_word_text_index.shrink_to_fit();
        self.headword_index.shrink_to_fit();
        for pos_map in self.syllable_index.values_mut() {
            pos_map.shrink_to_fit();
        }
        self.syllable_index.shrink_to_fit();
        for pos_map in self.initial_index.values_mut() {
            pos_map.shrink_to_fit();
        }
        self.initial_index.shrink_to_fit();
    }

    pub fn load_split_tsvs(
        paths: &[std::path::PathBuf],
        index_dir: Option<&Path>,
    ) -> Result<Dictionary, DictionaryError> {
        let cache_path = runtime_cache_path(paths);
        if let Some(cache_path) = cache_path.as_deref() {
            if cache_path.is_file() && cache_is_fresh(cache_path, paths) {
                let mut dictionary = Self::load_runtime_cache(cache_path)?;
                dictionary.rebuild_exact_lookup_indices();
                let (fst_map, postings, postings_topk) = load_prebuilt_indexes(index_dir);
                dictionary.fst_map = fst_map;
                dictionary.postings = postings;
                dictionary.postings_topk = postings_topk;
                // Augmentation already baked into runtime cache by build_fst.
                return Ok(dictionary);
            }
        }
        let existing: Vec<&Path> = paths
            .iter()
            .map(std::path::PathBuf::as_path)
            .filter(|path| path.is_file() && file_has_content(path))
            .collect();
        if existing.is_empty() {
            return Ok(Dictionary::empty());
        }
        let build_exact_indices = true;
        let mut dictionary = Self::load_tsv_internal(existing[0], build_exact_indices)?;
        for path in existing.iter().skip(1) {
            dictionary.extend_from_tsv_internal(path, build_exact_indices)?;
        }
        dictionary.rebuild_new_old_map();
        // Apply pair-restricted multi-reading augmentation with cross-file data
        // so the non-cache path matches what build_fst bakes into the cache.
        dictionary.rebuild_multi_reading_augmentation();
        let (fst_map, postings, postings_topk) = load_prebuilt_indexes(index_dir);
        dictionary.fst_map = fst_map;
        dictionary.postings = postings;
        dictionary.postings_topk = postings_topk;
        // Persist runtime cache so subsequent loads skip TSV parsing.
        if let Some(cache_path) = cache_path.as_deref() {
            let _ = dictionary.write_runtime_cache(cache_path);
        }
        Ok(dictionary)
    }

    pub fn load_split_tsvs_uncached(
        paths: &[std::path::PathBuf],
    ) -> Result<Dictionary, DictionaryError> {
        let existing: Vec<&Path> = paths
            .iter()
            .map(std::path::PathBuf::as_path)
            .filter(|path| path.is_file() && file_has_content(path))
            .collect();
        if existing.is_empty() {
            return Ok(Dictionary::empty());
        }
        let mut dictionary = Self::load_tsv_internal(existing[0], true)?;
        for path in existing.iter().skip(1) {
            dictionary.extend_from_tsv_internal(path, true)?;
        }
        Ok(dictionary)
    }

    /// Lookup postings IDs for a concatenated-syllable key using prebuilt FST
    /// if available. Returns None if FST/postings not present.
    pub fn lookup_fst_postings(&self, key: &str) -> Option<&Vec<u32>> {
        let key_bytes = key.as_bytes();
        if let Some(map) = &self.fst_map {
            if let Some(v) = map.get(key_bytes) {
                let idx = v as usize;
                // Prefer precomputed top-k if available for faster response
                if let Some(topk) = &self.postings_topk {
                    if let Some(t) = topk.get(idx) {
                        return Some(t);
                    }
                }
                if let Some(postings) = &self.postings {
                    return postings.get(idx);
                }
            }
        }
        None
    }

    /// Fast-path: try FST lookup then trie fallback, returning candidate ids.
    pub fn lookup_prefix_ids(&self, key: &str) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        let compact = key.replace(' ', "").to_ascii_lowercase();
        if compact.is_empty() {
            return out;
        }
        let mut seen: HashSet<u32> = HashSet::new();
        for index in [
            self.dialect_index.get(&compact),
            self.mandarin_index.get(&compact),
            self.mandarin_word_index.get(&compact),
        ]
        .into_iter()
        .flatten()
        {
            for &id in index {
                if seen.insert(id) {
                    out.push(id);
                }
            }
        }
        if let Some(posting) = self.lookup_fst_postings(&compact) {
            for &id in posting {
                if seen.insert(id) {
                    out.push(id);
                }
            }
        }
        // Fallback: use trie prefix scan — collect entries for any keys starting with compact
        // naive approach: traverse trie by prefix and collect entries in subtree
        // implement a simple DFS starting at node matching prefix
        let mut node = &self.syllable_trie.root;
        for ch in compact.chars() {
            if let Some(next) = node.children.get(&ch) {
                node = next;
            } else {
                return out;
            }
        }
        // collect entries in subtree, capped to avoid freezing on common prefixes
        const MAX_PREFIX_RESULTS: usize = 500;
        let mut stack: Vec<&crate::trie::TrieNode> = vec![node];
        while let Some(n) = stack.pop() {
            for &e in &n.entries {
                if seen.insert(e as u32) {
                    out.push(e as u32);
                }
                if out.len() >= MAX_PREFIX_RESULTS {
                    return out;
                }
            }
            for child in n.children.values() {
                stack.push(child);
            }
        }
        out
    }

    pub fn entries(&self) -> &[DictionaryEntry] {
        &self.entries
    }

    pub(crate) fn cache_id(&self) -> u64 {
        self.cache_id
    }

    pub(crate) fn set_user_frequency(&mut self, headword: &str, frequency: u64) -> bool {
        let mut changed = false;
        for entry in &mut self.entries {
            if entry.headword == headword && entry.category == "自" {
                entry.frequency = Some(frequency);
                changed = true;
            }
        }
        if changed {
            self.cache_id = next_dictionary_id();
        }
        changed
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Load a TSV file and extend this dictionary with proper rich indexing,
    /// matching the same lookup-form generation used by `load_tsv`.
    pub fn extend_from_tsv(&mut self, path: impl AsRef<Path>) -> Result<(), DictionaryError> {
        self.extend_from_tsv_internal(path.as_ref(), true)
    }

    fn extend_from_tsv_internal(
        &mut self,
        path: &Path,
        build_exact_indices: bool,
    ) -> Result<(), DictionaryError> {
        self.cache_id = next_dictionary_id();
        // 条目变更, 关联缓存失效(下次访问 associations() 时重建)。
        let _ = self.association_cache.take();
        let content = fs::read_to_string(path)?;
        let mut lines = content.lines().enumerate();
        // Skip header
        let mut header_found = false;
        for (_, raw) in lines.by_ref() {
            let line = raw.trim_end_matches(['\r', '\n']);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let header_cols: Vec<&str> = line.split('\t').map(str::trim).collect();
            let _ = header_cols; // header validated by load_tsv; skip here
            header_found = true;
            break;
        }
        if !header_found {
            return Ok(());
        }
        let start_index = self.entries.len();
        // Collect single-char readings for multi-reading augmentation.
        let mut char_readings: HashMap<String, Vec<String>> = HashMap::new();
        let mut char_readings_seen: HashMap<String, HashSet<String>> = HashMap::new();
        let mut new_entries: Vec<DictionaryEntry> = Vec::new();
        for (_, raw) in lines {
            let line = raw.trim_end_matches(['\r', '\n']);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.len() < 9 {
                continue;
            }
            let headword = columns[0].trim().to_string();
            if headword.is_empty() {
                continue;
            }
            // Collect per-character readings from ALL entries for multi-reading augmentation.
            let new_pinyin = normalize_pinyin(columns[2].trim());
            let syllables: Vec<String> = pinyin_segments(&new_pinyin)
                .iter()
                .map(|s| strip_tone(s).to_string())
                .collect();
            for (pos, ch) in headword.chars().enumerate() {
                if let Some(syl) = syllables.get(pos) {
                    let ch_str = ch.to_string();
                    if char_readings_seen
                        .entry(ch_str.clone())
                        .or_default()
                        .insert(syl.clone())
                    {
                        char_readings.entry(ch_str).or_default().push(syl.clone());
                    }
                }
            }
            let frequency = {
                let raw_freq = columns[7].trim();
                if raw_freq.is_empty() {
                    None
                } else {
                    raw_freq.parse::<u64>().ok()
                }
            };
            let entry_index = start_index + new_entries.len();
            let entry = DictionaryEntry {
                headword: headword.clone(),
                ipa: columns[1].trim().to_string(),
                dialect_pinyin: normalize_pinyin(columns[2].trim()),
                mandarin_pinyin: normalize_pinyin(columns[3].trim()),
                category: columns[4].trim().to_string(),
                mandarin_word: columns[5].trim().to_string(),
                mandarin_word_pinyin: normalize_pinyin(columns[6].trim()),
                frequency,
                synonyms: columns[8].trim().to_string(),
                entry_index,
                new_old: columns.get(9).map(|s| s.trim()).unwrap_or("").to_string(),
            };
            self.headword_index
                .entry(entry.headword.clone())
                .or_default()
                .push(entry_index as u32);
            if build_exact_indices {
                add_index_entry(
                    &mut self.dialect_index,
                    dialect_lookup_forms(&entry.dialect_pinyin),
                    entry_index,
                );
                add_index_entry(
                    &mut self.mandarin_index,
                    pinyin_lookup_forms(&entry.mandarin_pinyin),
                    entry_index,
                );
                if entry.has_distinct_mandarin_word() {
                    for mandarin_word_pinyin in split_list_values(&entry.mandarin_word_pinyin) {
                        add_index_entry(
                            &mut self.mandarin_word_index,
                            pinyin_lookup_forms(&mandarin_word_pinyin),
                            entry_index,
                        );
                    }
                }
            }
            if entry.has_distinct_mandarin_word() {
                add_text_index_entries(
                    &mut self.mandarin_word_text_index,
                    distinct_mandarin_words(&entry.headword, &entry.mandarin_word),
                    entry_index,
                );
            }
            {
                let dialect_syllables: Vec<String> = pinyin_segments(&entry.dialect_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let mandarin_syllables: Vec<String> = pinyin_segments(&entry.mandarin_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let max_len = dialect_syllables.len().max(mandarin_syllables.len());
                for pos in 0..max_len {
                    let mut syllables_at_pos: Vec<String> = Vec::new();
                    if let Some(s) = dialect_syllables.get(pos) {
                        push_unique(&mut syllables_at_pos, s.clone());
                    }
                    if let Some(s) = mandarin_syllables.get(pos) {
                        push_unique(&mut syllables_at_pos, s.clone());
                    }
                    for syl in &syllables_at_pos {
                        self.syllable_index
                            .entry(pos)
                            .or_default()
                            .entry(syl.clone())
                            .or_default()
                            .push(entry_index as u32);
                        if let Some(initial) = syl.chars().find(|ch| ch.is_ascii_alphabetic()) {
                            let ids = self
                                .initial_index
                                .entry(pos)
                                .or_default()
                                .entry(initial.to_ascii_lowercase())
                                .or_default();
                            if !ids.contains(&(entry_index as u32)) {
                                ids.push(entry_index as u32);
                            }
                        }
                    }
                }
            }
            new_entries.push(entry);
        }
        // Augment syllable_index with single-char alternative readings.
        // Use a per-entry HashSet to avoid O(n) Vec::contains checks.
        for (offset, entry) in new_entries.iter().enumerate() {
            let chars: Vec<char> = entry.headword.chars().collect();
            if chars.len() <= 1 {
                continue;
            }
            let mut seen: HashSet<(usize, String)> = HashSet::new();
            for (pos, &ch) in chars.iter().enumerate() {
                let ch_str: String = ch.into();
                if let Some(readings) = char_readings.get(&ch_str) {
                    for reading in readings {
                        if seen.insert((pos, reading.clone())) {
                            let pos_map = self.syllable_index.entry(pos).or_default();
                            let entry_index = start_index + offset;
                            pos_map
                                .entry(reading.clone())
                                .or_default()
                                .push(entry_index as u32);
                        }
                    }
                }
            }
        }
        // Merge this file's readings into the dictionary-wide multi-reading table.
        for (ch, readings) in char_readings {
            let existing = self.char_readings.entry(ch).or_default();
            for reading in readings {
                if !existing.contains(&reading) {
                    existing.push(reading);
                }
            }
        }
        self.entries.extend(new_entries);

        // Rebuild initial-index for all positions using HashSet dedup.
        self.initial_index.clear();
        for (pos, pos_map) in &self.syllable_index {
            let mut char_sets: HashMap<char, HashSet<u32>> = HashMap::new();
            for (syl, ids) in pos_map {
                if let Some(first_ch) = syl.chars().find(|c| c.is_ascii_alphabetic()) {
                    let ch = first_ch.to_ascii_lowercase();
                    char_sets.entry(ch).or_default().extend(ids);
                }
            }
            let pos_initial: &mut HashMap<char, Vec<u32>> =
                self.initial_index.entry(*pos).or_default();
            for (ch, id_set) in char_sets {
                pos_initial.insert(ch, id_set.into_iter().collect());
            }
        }

        Ok(())
    }

    pub fn extend_from_entries(&mut self, new_entries: impl IntoIterator<Item = DictionaryEntry>) {
        self.cache_id = next_dictionary_id();
        // 条目变更, 关联缓存失效(下次访问 associations() 时重建)。
        let _ = self.association_cache.take();
        let start_index = self.entries.len();
        for (offset, mut entry) in new_entries.into_iter().enumerate() {
            let entry_index = start_index + offset;
            if entry.headword.is_empty() {
                continue;
            }
            entry.entry_index = entry_index;
            // Record this entry's dialect readings in the multi-reading table.
            {
                let syllables: Vec<String> = pinyin_segments(&entry.dialect_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                for (pos, ch) in entry.headword.chars().enumerate() {
                    if let Some(syl) = syllables.get(pos) {
                        let readings = self.char_readings.entry(ch.to_string()).or_default();
                        if !readings.contains(syl) {
                            readings.push(syl.clone());
                        }
                    }
                }
            }
            self.headword_index
                .entry(entry.headword.clone())
                .or_default()
                .push(entry_index as u32);
            if entry.has_distinct_mandarin_word() {
                add_text_index_entries(
                    &mut self.mandarin_word_text_index,
                    distinct_mandarin_words(&entry.headword, &entry.mandarin_word),
                    entry_index,
                );
            }
            {
                let dialect_syllables: Vec<String> = pinyin_segments(&entry.dialect_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let mandarin_syllables: Vec<String> = pinyin_segments(&entry.mandarin_pinyin)
                    .iter()
                    .map(|s| strip_tone(s).to_string())
                    .collect();
                let max_len = dialect_syllables.len().max(mandarin_syllables.len());
                for pos in 0..max_len {
                    let mut syllables_at_pos: Vec<String> = Vec::new();
                    if let Some(s) = dialect_syllables.get(pos) {
                        if !s.is_empty() {
                            push_unique(&mut syllables_at_pos, s.clone());
                        }
                    }
                    if let Some(s) = mandarin_syllables.get(pos) {
                        if !s.is_empty() {
                            push_unique(&mut syllables_at_pos, s.clone());
                        }
                    }
                    for syl in &syllables_at_pos {
                        self.syllable_index
                            .entry(pos)
                            .or_default()
                            .entry(syl.clone())
                            .or_default()
                            .push(entry_index as u32);
                        if let Some(initial) = syl.chars().find(|ch| ch.is_ascii_alphabetic()) {
                            let ids = self
                                .initial_index
                                .entry(pos)
                                .or_default()
                                .entry(initial.to_ascii_lowercase())
                                .or_default();
                            if !ids.contains(&(entry_index as u32)) {
                                ids.push(entry_index as u32);
                            }
                        }
                    }
                }
            }
            add_index_entry(
                &mut self.dialect_index,
                dialect_lookup_forms(&entry.dialect_pinyin),
                entry_index,
            );
            add_index_entry(
                &mut self.mandarin_index,
                pinyin_lookup_forms(&entry.mandarin_pinyin),
                entry_index,
            );
            if entry.has_distinct_mandarin_word() && !entry.mandarin_word_pinyin.is_empty() {
                for mandarin_word_pinyin in split_list_values(&entry.mandarin_word_pinyin) {
                    add_index_entry(
                        &mut self.mandarin_word_index,
                        pinyin_lookup_forms(&mandarin_word_pinyin),
                        entry_index,
                    );
                }
            }
            for form in dialect_lookup_forms(&entry.dialect_pinyin)
                .into_iter()
                .chain(pinyin_lookup_forms(&entry.mandarin_pinyin))
            {
                let key = form.replace([' ', '\''], "").to_ascii_lowercase();
                if !key.is_empty() {
                    self.syllable_trie.insert(&key, entry_index);
                }
            }
            self.entries.push(entry);
        }
    }

    /// Rebuild multi-reading augmentation for all multi-character entries
    /// using the complete character→readings map from the current entry set.
    /// This must be called after all TSV files are loaded, so that readings
    /// discovered in later files (e.g. gan_chars.tsv) can augment entries
    /// from earlier files (e.g. words.tsv).
    pub fn rebuild_multi_reading_augmentation(&mut self) {
        // Collect per-character readings from ALL entries.
        self.char_readings = collect_char_readings(&self.entries);
        // Augment syllable_index for all multi-character entries.
        let char_readings = &self.char_readings;
        for (entry_index, entry) in self.entries.iter().enumerate() {
            let chars: Vec<char> = entry.headword.chars().collect();
            if chars.len() <= 1 {
                continue;
            }
            let mut seen_pos: HashSet<(usize, String)> = HashSet::new();
            for (pos, &ch) in chars.iter().enumerate() {
                let ch_str: String = ch.into();
                if let Some(readings) = char_readings.get(&ch_str) {
                    for reading in readings {
                        if seen_pos.insert((pos, reading.clone())) {
                            let pos_map = self.syllable_index.entry(pos).or_default();
                            pos_map
                                .entry(reading.clone())
                                .or_default()
                                .push(entry_index as u32);
                        }
                    }
                }
            }
        }
        // Register multi-reading full forms into the exact dialect index so
        // every reading of a character ranks equally (等权, GannyuExact).
        self.augment_dialect_index_multi_readings();
    }

    /// Build paired-reading display maps.  新老 has priority over 本又 when
    /// malformed data happens to mark the same character with both relations.
    pub fn rebuild_new_old_map(&mut self) {
        self.new_old_map.clear();
        self.heteronym_chars.clear();
        self.paired_readings.clear();
        let mut groups: HashMap<char, HashMap<String, Vec<&DictionaryEntry>>> = HashMap::new();
        let mut suffix_order: HashMap<char, Vec<String>> = HashMap::new();
        let mut wen_bai: HashMap<char, (Option<String>, Option<String>)> = HashMap::new();
        for entry in &self.entries {
            if entry.new_old.is_empty() || entry.headword.chars().count() != 1 {
            } else {
                let tag = entry.new_old.as_str();
                if matches!(tag.chars().next(), Some('新' | '老' | '本' | '又')) {
                    let ch = entry.headword.chars().next().unwrap();
                    let suffix: String = tag.chars().skip(1).collect();
                    let char_groups = groups.entry(ch).or_default();
                    if !char_groups.contains_key(&suffix) {
                        suffix_order.entry(ch).or_default().push(suffix.clone());
                    }
                    char_groups.entry(suffix).or_default().push(entry);
                }
            }
            if entry.headword.chars().count() != 1 {
                continue;
            }
            let ch = entry.headword.chars().next().unwrap();
            let register = wen_bai.entry(ch).or_insert((None, None));
            match entry.category.as_str() {
                "文" => {
                    if register.0.is_none() {
                        register.0 = Some(entry.dialect_pinyin.clone());
                    }
                }
                "白" if register.1.is_none() => {
                    register.1 = Some(entry.dialect_pinyin.clone());
                }
                _ => {}
            }
        }
        let mut heteronyms = HashMap::new();
        for (ch, suffixes) in suffix_order {
            let Some(suffix_groups) = groups.get(&ch) else {
                continue;
            };
            for suffix in suffixes {
                let Some(entries) = suffix_groups.get(&suffix) else {
                    continue;
                };
                if entries.len() != 2 {
                    continue;
                }
                let find = |prefix| {
                    entries
                        .iter()
                        .copied()
                        .find(|entry| entry.new_old.starts_with(prefix))
                };
                if let (Some(new), Some(old)) = (find("新"), find("老")) {
                    self.paired_readings
                        .entry(ch)
                        .or_default()
                        .push(PairedReading {
                            first: new.dialect_pinyin.clone(),
                            second: old.dialect_pinyin.clone(),
                            kind: PairKind::NewOld,
                        });
                    self.new_old_map.entry(ch).or_insert_with(|| {
                        (new.dialect_pinyin.clone(), old.dialect_pinyin.clone())
                    });
                } else if let (Some(base), Some(variant)) = (find("本"), find("又")) {
                    self.paired_readings
                        .entry(ch)
                        .or_default()
                        .push(PairedReading {
                            first: base.dialect_pinyin.clone(),
                            second: variant.dialect_pinyin.clone(),
                            kind: PairKind::Heteronym,
                        });
                    heteronyms.entry(ch).or_insert_with(|| {
                        (base.dialect_pinyin.clone(), variant.dialect_pinyin.clone())
                    });
                }
            }
        }
        for (ch, (wen, bai)) in wen_bai {
            if let (Some(wen), Some(bai)) = (wen, bai) {
                self.paired_readings
                    .entry(ch)
                    .or_default()
                    .push(PairedReading {
                        first: wen,
                        second: bai,
                        kind: PairKind::WenBai,
                    });
            }
        }
        for (ch, pair) in heteronyms {
            if !self.new_old_map.contains_key(&ch) {
                self.new_old_map.insert(ch, pair);
                self.heteronym_chars.insert(ch);
            }
        }
    }

    pub fn by_dialect_pinyin(&self, syllable: &str) -> Vec<&DictionaryEntry> {
        let lookup = normalize_pinyin(syllable);
        let mut matches: Vec<&DictionaryEntry> = self
            .dialect_index
            .get(&lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        // e ↔ ĕ 互模糊
        let alt = alternate_e_syllable(syllable, "e\u{0306}");
        if alt != *syllable {
            if let Some(ids) = self.dialect_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    /// Like `by_dialect_pinyin` but assumes `syllable` is already normalized
    /// (lowercased, separators collapsed). Avoids a redundant normalize call
    /// in hot paths where callers hold a normalized key.
    pub fn by_dialect_pinyin_normalized(&self, lookup: &str) -> Vec<&DictionaryEntry> {
        let mut matches: Vec<&DictionaryEntry> = self
            .dialect_index
            .get(lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        // e ↔ ĕ 互模糊
        let alt = alternate_e_syllable(lookup, "e\u{0306}");
        if alt != *lookup {
            if let Some(ids) = self.dialect_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    pub fn by_mandarin_pinyin(&self, syllable: &str) -> Vec<&DictionaryEntry> {
        let lookup = normalize_pinyin(syllable);
        let mut matches: Vec<&DictionaryEntry> = self
            .mandarin_index
            .get(&lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        let alt = alternate_e_syllable(syllable, "e\u{0306}");
        if alt != syllable {
            if let Some(ids) = self.mandarin_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    /// Like `by_mandarin_pinyin` but assumes `syllable` is already normalized.
    pub fn by_mandarin_pinyin_normalized(&self, lookup: &str) -> Vec<&DictionaryEntry> {
        let mut matches: Vec<&DictionaryEntry> = self
            .mandarin_index
            .get(lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        let alt = alternate_e_syllable(lookup, "e\u{0306}");
        if alt != *lookup {
            if let Some(ids) = self.mandarin_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    pub fn by_mandarin_word_pinyin(&self, syllable: &str) -> Vec<&DictionaryEntry> {
        let lookup = normalize_pinyin(syllable);
        let mut matches: Vec<&DictionaryEntry> = self
            .mandarin_word_index
            .get(&lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        let alt = alternate_e_syllable(syllable, "e\u{0306}");
        if alt != syllable {
            if let Some(ids) = self.mandarin_word_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    /// Like `by_mandarin_word_pinyin` but assumes `syllable` is already normalized.
    pub fn by_mandarin_word_pinyin_normalized(&self, lookup: &str) -> Vec<&DictionaryEntry> {
        let mut matches: Vec<&DictionaryEntry> = self
            .mandarin_word_index
            .get(lookup)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect();
        let alt = alternate_e_syllable(lookup, "e\u{0306}");
        if alt != *lookup {
            if let Some(ids) = self.mandarin_word_index.get(&normalize_pinyin(&alt)) {
                for index in ids {
                    if let Some(entry) = self.entries.get(*index as usize) {
                        matches.push(entry);
                    }
                }
            }
        }
        matches
    }

    pub fn by_headword(&self, headword: &str) -> Vec<&DictionaryEntry> {
        self.headword_index
            .get(headword)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect()
    }

    pub fn by_mandarin_word_text(&self, word: &str) -> Vec<&DictionaryEntry> {
        self.mandarin_word_text_index
            .get(word)
            .into_iter()
            .flatten()
            .filter_map(|index| self.entries.get(*index as usize))
            .collect()
    }

    /// Find entries whose dialect_pinyin OR mandarin_pinyin has `syllable`
    /// at a specific character position (0-indexed).
    /// Supports mixed Gan/Mandarin pinyin input for multi-character words.
    pub fn by_syllable_at_position(
        &self,
        position: usize,
        syllable: &str,
    ) -> Vec<&DictionaryEntry> {
        let lookup = syllable.trim().to_ascii_lowercase();
        let pos_map = match self.syllable_index.get(&position) {
            Some(m) => m,
            None => return Vec::new(),
        };
        // 先用精确查找（O(1)）
        if let Some(ids) = pos_map.get(&lookup) {
            return ids
                .iter()
                .filter_map(|i| self.entries.get(*i as usize))
                .collect();
        }
        // 未命中时再试 e↔ĕ 替代版
        let alt = normalize_e_tone(&lookup);
        if alt != lookup {
            if let Some(ids) = pos_map.get(&alt) {
                return ids
                    .iter()
                    .filter_map(|i| self.entries.get(*i as usize))
                    .collect();
            }
        }
        let alt_breve = lookup.replace('e', "e\u{0306}");
        if alt_breve != lookup {
            if let Some(ids) = pos_map.get(&alt_breve) {
                return ids
                    .iter()
                    .filter_map(|i| self.entries.get(*i as usize))
                    .collect();
            }
        }
        Vec::new()
    }

    /// O(1) initial-letter lookup: get all entry indices at a position
    /// whose first alphabetic character matches `initial`.
    pub fn initial_match_ids(&self, position: usize, initial: char) -> &[u32] {
        self.initial_index
            .get(&position)
            .and_then(|pos_map| pos_map.get(&initial.to_ascii_lowercase()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Return the per-position syllable index map for read-only inspection.
    pub fn syllable_map_at_position(&self, position: usize) -> Option<&HashMap<String, Vec<u32>>> {
        self.syllable_index.get(&position)
    }

    /// Expose the built trie for callers that need prefix-accelerated checks.
    pub fn syllable_trie(&self) -> &crate::trie::Trie {
        &self.syllable_trie
    }

    /// O(1) reverse-lookup: find the index of an entry in self.entries.
    pub fn entry_id(&self, entry: &DictionaryEntry) -> Option<usize> {
        Some(entry.entry_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(headword: &str, category: &str, mandarin_word: &str) -> DictionaryEntry {
        DictionaryEntry {
            headword: headword.to_string(),
            ipa: String::new(),
            dialect_pinyin: "kai1".to_string(),
            mandarin_pinyin: "kai1".to_string(),
            category: category.to_string(),
            mandarin_word: mandarin_word.to_string(),
            mandarin_word_pinyin: "kai1".to_string(),
            frequency: Some(100000),
            synonyms: String::new(),
            entry_index: 0,
            new_old: String::new(),
        }
    }

    #[test]
    fn mandarin_word_text_index_ignores_self_reference() {
        let mut dictionary = Dictionary::default();
        dictionary.extend_from_entries([entry("开", "赣", "开"), entry("开会", "赣", "会议")]);

        assert!(dictionary.by_mandarin_word_text("开").is_empty());
        assert_eq!(dictionary.by_mandarin_word_text("会议").len(), 1);
    }

    #[test]
    fn mandarin_word_text_index_ignores_self_reference_inside_multi_value_field() {
        let mut dictionary = Dictionary::default();
        dictionary.extend_from_entries([entry("开", "赣", "开/会议")]);

        assert!(dictionary.by_mandarin_word_text("开").is_empty());
        assert_eq!(dictionary.by_mandarin_word_text("会议").len(), 1);
    }

    #[test]
    fn extend_from_entries_splits_multi_value_mandarin_word_pinyin() {
        let mut dictionary = Dictionary::default();
        let mut entry = entry("青菜", "赣", "蔬菜/菜");
        entry.mandarin_word_pinyin = "shu1 cai4/cai4".to_string();
        dictionary.extend_from_entries([entry]);

        assert_eq!(dictionary.by_mandarin_word_pinyin("shucai").len(), 1);
        assert_eq!(dictionary.by_mandarin_word_pinyin("cai").len(), 1);
    }
}
