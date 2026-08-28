use crate::dictionary::{distinct_mandarin_words, Dictionary};
use crate::slang::SlangBook;
use std::collections::{HashMap, HashSet};

/// Pre-computed cache mapping words to their associated words and
/// Gan-Mandarin pairs. Built once at pipeline load time (打包时空间换时间).
#[derive(Debug, Default)]
pub struct AssociationCache {
    /// Word → all other words in its synonym association group.
    /// Bidirectional: if A lists B as synonym, both A→B and B→A edges exist.
    word_to_associates: HashMap<String, Vec<String>>,
    /// Gan headword → its Mandarin equivalent words.
    gan_to_mandarin: HashMap<String, Vec<String>>,
    /// Mandarin word → list of Gan headword equivalents.
    mandarin_to_gan: HashMap<String, Vec<String>>,
}

impl AssociationCache {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build the cache from dictionary entries and optional slang book.
    pub fn build(dictionary: &Dictionary, _slang: Option<&SlangBook>) -> Self {
        let mut cache = Self::empty();

        // 1. Build bidirectional synonym groups from dictionary entries.
        //    First collect all headword→synonym pairs, then insert bidirectionally.
        let mut raw_edges: HashMap<String, HashSet<String>> = HashMap::new();
        let mut synonym_pairs: Vec<(String, String)> = Vec::new();
        for entry in dictionary.entries() {
            let headword = entry.headword.trim().to_string();
            if headword.is_empty() {
                continue;
            }
            for raw_syn in entry.synonyms.split('/') {
                let syn = raw_syn.trim().to_string();
                if syn.is_empty() || syn == headword {
                    continue;
                }
                synonym_pairs.push((headword.clone(), syn));
            }
        }
        for (headword, syn) in synonym_pairs {
            raw_edges
                .entry(headword.clone())
                .or_default()
                .insert(syn.clone());
            raw_edges.entry(syn).or_default().insert(headword);
        }

        // Collapse edges into sorted Vecs (deduplicate + stable order for determinism)
        for (word, set) in raw_edges {
            let mut associates: Vec<String> = set.into_iter().collect();
            associates.sort();
            cache.word_to_associates.insert(word, associates);
        }

        // 2. Build Gan-Mandarin pair maps.
        for entry in dictionary.entries() {
            let headword = entry.headword.trim().to_string();
            let mandarin_words = distinct_mandarin_words(&headword, &entry.mandarin_word);
            if mandarin_words.is_empty() {
                continue;
            }
            let gan_entry = cache.gan_to_mandarin.entry(headword.clone()).or_default();
            for mw in mandarin_words {
                if !gan_entry.contains(&mw) {
                    gan_entry.push(mw.clone());
                }
                cache
                    .mandarin_to_gan
                    .entry(mw)
                    .or_default()
                    .push(headword.clone());
            }
        }

        // Sort/unique vectors for determinism.
        for mandarins in cache.gan_to_mandarin.values_mut() {
            mandarins.sort();
            mandarins.dedup();
        }
        for gans in cache.mandarin_to_gan.values_mut() {
            gans.sort();
            gans.dedup();
        }

        cache
    }

    /// Get all words associated with `word` (synonyms + reverse synonyms).
    /// Returns empty slice if no associations.
    pub fn associates_of(&self, word: &str) -> &[String] {
        self.word_to_associates
            .get(word)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the Mandarin equivalent of a Gan word, if any.
    pub fn mandarins_of_gan(&self, gan_word: &str) -> &[String] {
        self.gan_to_mandarin
            .get(gan_word)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the Gan equivalents of a Mandarin word.
    pub fn gan_of_mandarin(&self, mandarin_word: &str) -> &[String] {
        self.mandarin_to_gan
            .get(mandarin_word)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check whether `word` has any Gan-Mandarin pair relationship.
    pub fn has_pair(&self, word: &str) -> bool {
        self.gan_to_mandarin.contains_key(word) || self.mandarin_to_gan.contains_key(word)
    }

    /// Returns true if no pair or association data exists.
    pub fn is_empty(&self) -> bool {
        self.gan_to_mandarin.is_empty()
            && self.mandarin_to_gan.is_empty()
            && self.word_to_associates.is_empty()
    }

    /// Iterator over Gan-to-Mandarin keys (for debugging).
    pub fn gan_to_mandarin_keys(&self) -> impl Iterator<Item = &String> {
        self.gan_to_mandarin.keys()
    }

    /// Get the full pair group for a word: Gan words + Mandarin word.
    /// Returns (gan_words, mandarin_word) tuple.
    /// If the input is a Gan word: returns (vec![word], its mandarin)
    /// If the input is a Mandarin word: returns (its gan equivalents, word)
    pub fn pair_group_of(&self, word: &str) -> (Vec<String>, Option<String>) {
        if let Some(mandarins) = self.gan_to_mandarin.get(word) {
            if let Some(first) = mandarins.first() {
                return (vec![word.to_string()], Some(first.clone()));
            }
        }
        if let Some(gans) = self.mandarin_to_gan.get(word) {
            if !gans.is_empty() {
                return (gans.clone(), Some(word.to_string()));
            }
        }
        (Vec::new(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_cache() {
        let cache = AssociationCache::empty();
        assert!(cache.associates_of("任何词").is_empty());
        assert!(cache.mandarins_of_gan("任何词").is_empty());
    }
}
