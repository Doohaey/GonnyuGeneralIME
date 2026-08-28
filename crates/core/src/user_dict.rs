use std::collections::HashMap;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::dictionary::DictionaryEntry;

#[derive(Debug)]
pub struct UserDictionary {
    entries: HashMap<String, DictionaryEntry>,
    path: PathBuf,
}

fn default_user_dict_path() -> PathBuf {
    if let Some(data) = dirs_next() {
        return data.join("user_dictionary.tsv");
    }
    PathBuf::from("user_dictionary.tsv")
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("gannyu-input"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join(".local/share/gannyu-input"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join("Library/Application Support/gannyu-input"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join("GannyuInput"));
        }
    }
    #[cfg(target_os = "android")]
    {
        if let Ok(data_home) = std::env::var("GANNYU_DATA_HOME") {
            if !data_home.is_empty() {
                return Some(PathBuf::from(data_home).join("gannyu-input"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                return Some(PathBuf::from(home).join("gannyu-input"));
            }
        }
    }
    None
}

const USER_DICT_HEADER: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词";

impl UserDictionary {
    pub fn load_or_create() -> Self {
        let path = default_user_dict_path();
        let mut entries = HashMap::new();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                for (line_no, line) in content.lines().enumerate() {
                    if line_no == 0 || line.trim().is_empty() || line.trim_start().starts_with('#')
                    {
                        continue;
                    }
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() < 4 {
                        continue;
                    }
                    let headword = cols[0].trim().to_string();
                    if headword.is_empty() {
                        continue;
                    }
                    let dialect_pinyin = cols
                        .get(2)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    let mandarin_pinyin = cols
                        .get(3)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    let category = cols
                        .get(4)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    let frequency = cols.get(7).and_then(|s| s.trim().parse::<u64>().ok());
                    let entry = DictionaryEntry {
                        headword: headword.clone(),
                        ipa: String::new(),
                        dialect_pinyin,
                        mandarin_pinyin,
                        category,
                        mandarin_word: String::new(),
                        mandarin_word_pinyin: String::new(),
                        frequency,
                        synonyms: String::new(),
                        entry_index: 0,
                        new_old: String::new(),
                    };
                    entries.insert(headword, entry);
                }
            }
        }
        UserDictionary { entries, path }
    }

    pub fn contains(&self, headword: &str) -> bool {
        self.entries.contains_key(headword)
    }

    pub fn add(&mut self, headword: &str, dialect_pinyin: &str, mandarin_pinyin: &str) -> bool {
        if headword.chars().count() < 2 || headword.is_empty() || dialect_pinyin.is_empty() {
            return false;
        }
        if headword.contains('\t')
            || headword.contains('\n')
            || dialect_pinyin.contains('\t')
            || dialect_pinyin.contains('\n')
            || mandarin_pinyin.contains('\t')
            || mandarin_pinyin.contains('\n')
        {
            return false;
        }
        if self.entries.contains_key(headword) {
            return false;
        }
        let entry = DictionaryEntry {
            headword: headword.to_string(),
            ipa: String::new(),
            dialect_pinyin: dialect_pinyin.to_string(),
            mandarin_pinyin: mandarin_pinyin.to_string(),
            category: "自".to_string(),
            mandarin_word: String::new(),
            mandarin_word_pinyin: String::new(),
            frequency: Some(1),
            synonyms: String::new(),
            entry_index: 0,
            new_old: String::new(),
        };
        let mut staged = self.entries.clone();
        staged.insert(headword.to_string(), entry);
        if !self.flush_entries(&staged) {
            return false;
        }
        self.entries = staged;
        true
    }

    pub fn entries(&self) -> impl Iterator<Item = &DictionaryEntry> {
        self.entries.values()
    }

    pub fn frequency(&self, headword: &str) -> Option<u64> {
        self.entries.get(headword).and_then(|entry| entry.frequency)
    }

    /// 自造词词频累加：每次 +20000，封顶 200000。词不存在时返回 false。
    pub fn boost_frequency(&mut self, headword: &str) -> bool {
        const BOOST_STEP: u64 = 20000;
        const MAX_FREQUENCY: u64 = 200000;
        let Some(current) = self
            .entries
            .get(headword)
            .map(|entry| entry.frequency.unwrap_or(0))
        else {
            return false;
        };
        if current >= MAX_FREQUENCY {
            return true;
        }
        let mut staged = self.entries.clone();
        staged.get_mut(headword).unwrap().frequency =
            Some((current + BOOST_STEP).min(MAX_FREQUENCY));
        if !self.flush_entries(&staged) {
            return false;
        }
        self.entries = staged;
        true
    }

    /// 批量设置多个词的词频（封顶 200000），只写盘一次。返回实际更新的词数。
    pub fn set_frequencies(&mut self, updates: &[(String, u64)]) -> usize {
        const MAX_FREQUENCY: u64 = 200000;
        let mut staged = self.entries.clone();
        let mut changed = 0;
        for (headword, frequency) in updates {
            if let Some(entry) = staged.get_mut(headword) {
                entry.frequency = Some((*frequency).min(MAX_FREQUENCY));
                changed += 1;
            }
        }
        if changed == 0 {
            return 0;
        }
        if !self.flush_entries(&staged) {
            return 0;
        }
        self.entries = staged;
        changed
    }

    pub fn clear(&mut self) -> bool {
        let entries = HashMap::new();
        if !self.flush_entries(&entries) {
            return false;
        }
        self.entries = entries;
        true
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn prune_existing(&mut self, exists_in_main: impl Fn(&str) -> bool) {
        let mut staged = self.entries.clone();
        staged.retain(|headword, _| !exists_in_main(headword));
        if staged.len() != self.entries.len() && self.flush_entries(&staged) {
            self.entries = staged;
        }
    }

    #[cfg(test)]
    pub(crate) fn empty_at(path: PathBuf) -> Self {
        UserDictionary {
            entries: HashMap::new(),
            path,
        }
    }

    fn flush_entries(&self, entries: &HashMap<String, DictionaryEntry>) -> bool {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
        let mut file = match tempfile::NamedTempFile::new_in(parent) {
            Ok(file) => file,
            Err(_) => return false,
        };
        #[cfg(unix)]
        if file
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .is_err()
        {
            return false;
        }
        if writeln!(file, "{USER_DICT_HEADER}").is_err() {
            return false;
        }
        let mut rows: Vec<&_> = entries.values().collect();
        rows.sort_by(|left, right| left.headword.cmp(&right.headword));
        for entry in rows {
            let freq = entry.frequency.map(|f| f.to_string()).unwrap_or_default();
            if writeln!(
                file,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                entry.headword,
                entry.ipa,
                entry.dialect_pinyin,
                entry.mandarin_pinyin,
                entry.category,
                entry.mandarin_word,
                entry.mandarin_word_pinyin,
                freq,
                entry.synonyms
            )
            .is_err()
            {
                return false;
            }
        }
        if file.flush().is_err() || file.as_file().sync_all().is_err() {
            return false;
        }
        file.persist(&self.path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dict() -> UserDictionary {
        let mut dict = UserDictionary {
            entries: HashMap::new(),
            path: std::env::temp_dir().join(format!("user_dict_test_{}.tsv", std::process::id())),
        };
        dict.add("测试词", "ce4 si4 ci2", "ce4 shi4 ci2");
        dict
    }

    #[test]
    fn boost_frequency_increments_by_step_and_caps() {
        let mut dict = temp_dict();
        assert!(dict.boost_frequency("测试词"));
        assert_eq!(
            dict.entries()
                .find(|e| e.headword == "测试词")
                .unwrap()
                .frequency,
            Some(20001)
        );
        // 累加到封顶
        for _ in 0..20 {
            dict.boost_frequency("测试词");
        }
        assert_eq!(
            dict.entries()
                .find(|e| e.headword == "测试词")
                .unwrap()
                .frequency,
            Some(200000)
        );
        // 封顶后不再累加
        dict.boost_frequency("测试词");
        assert_eq!(
            dict.entries()
                .find(|e| e.headword == "测试词")
                .unwrap()
                .frequency,
            Some(200000)
        );
    }

    #[test]
    fn boost_frequency_missing_word_returns_false() {
        let mut dict = temp_dict();
        assert!(!dict.boost_frequency("不存在的词"));
    }

    #[test]
    fn failed_add_does_not_mutate_memory() {
        let blocked = tempfile::tempdir().unwrap();
        let mut dict = UserDictionary::empty_at(blocked.path().to_path_buf());
        assert!(!dict.add("失败词", "si1 bai4 ci2", ""));
        assert!(!dict.contains("失败词"));
    }

    #[test]
    fn failed_boost_keeps_previous_frequency() {
        let mut dict = temp_dict();
        let before = dict.frequency("测试词");
        let blocked = tempfile::tempdir().unwrap();
        dict.path = blocked.path().to_path_buf();
        assert!(!dict.boost_frequency("测试词"));
        assert_eq!(dict.frequency("测试词"), before);
    }

    #[test]
    fn set_frequencies_batch_updates_and_caps() {
        let mut dict = temp_dict();
        dict.add("第二个词", "di4 er4 ci2", "di4 er4 ci2");
        let updated = dict.set_frequencies(&[
            ("测试词".to_string(), 500000),
            ("第二个词".to_string(), 12000),
            ("不存在的词".to_string(), 9999),
        ]);
        assert_eq!(updated, 2);
        assert_eq!(
            dict.entries()
                .find(|e| e.headword == "测试词")
                .unwrap()
                .frequency,
            Some(200000)
        );
        assert_eq!(
            dict.entries()
                .find(|e| e.headword == "第二个词")
                .unwrap()
                .frequency,
            Some(12000)
        );
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut dict = temp_dict();
        assert!(dict.clear());
        assert!(dict.entries().next().is_none());
    }
}
