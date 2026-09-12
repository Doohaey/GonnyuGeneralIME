use crate::dictionary::{distinct_mandarin_words, Dictionary};
use crate::slang::SlangBook;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

type WordId = u32;

#[derive(Clone, Copy)]
pub struct AssociationWords<'a> {
    words: &'a [Arc<str>],
    ids: &'a [WordId],
}

impl<'a> AssociationWords<'a> {
    fn empty(words: &'a [Arc<str>]) -> Self {
        Self { words, ids: &[] }
    }

    pub fn is_empty(self) -> bool {
        self.ids.is_empty()
    }

    pub fn first(self) -> Option<&'a str> {
        self.ids.first().map(|id| self.words[*id as usize].as_ref())
    }

    pub fn iter(self) -> AssociationWordIter<'a> {
        AssociationWordIter {
            words: self.words,
            ids: self.ids.iter(),
        }
    }

    pub fn to_vec(self) -> Vec<String> {
        self.iter().map(str::to_owned).collect()
    }

    pub fn join(self, separator: &str) -> String {
        self.iter().collect::<Vec<_>>().join(separator)
    }
}

impl<'a> IntoIterator for AssociationWords<'a> {
    type Item = &'a str;
    type IntoIter = AssociationWordIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct AssociationWordIter<'a> {
    words: &'a [Arc<str>],
    ids: std::slice::Iter<'a, WordId>,
}

impl<'a> Iterator for AssociationWordIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| self.words[*id as usize].as_ref())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl ExactSizeIterator for AssociationWordIter<'_> {}

/// Pre-computed cache mapping words to their associated words and
/// Gan-Mandarin pairs. Built once at pipeline load time (打包时空间换时间).
#[derive(Debug, Default)]
pub struct AssociationCache {
    /// Every relationship word is allocated once and addressed by a compact ID.
    words: Vec<Arc<str>>,
    word_ids: HashMap<Arc<str>, WordId>,
    /// Word → all other words in its synonym association group.
    /// Bidirectional: if A lists B as synonym, both A→B and B→A edges exist.
    word_to_associates: HashMap<WordId, Vec<WordId>>,
    /// Gan headword → its Mandarin equivalent words.
    gan_to_mandarin: HashMap<WordId, Vec<WordId>>,
    /// Mandarin word → list of Gan headword equivalents.
    mandarin_to_gan: HashMap<WordId, Vec<WordId>>,
}

impl AssociationCache {
    pub fn empty() -> Self {
        Self::default()
    }

    fn intern(&mut self, word: &str) -> WordId {
        if let Some(id) = self.word_ids.get(word) {
            return *id;
        }
        let id = WordId::try_from(self.words.len()).expect("association word table exceeds u32");
        let word: Arc<str> = Arc::from(word);
        self.words.push(Arc::clone(&word));
        self.word_ids.insert(word, id);
        id
    }

    fn word(&self, id: WordId) -> &str {
        self.words[id as usize].as_ref()
    }

    fn related_words<'a>(
        &'a self,
        relationships: &'a HashMap<WordId, Vec<WordId>>,
        word: &str,
    ) -> AssociationWords<'a> {
        let Some(id) = self.word_ids.get(word) else {
            return AssociationWords::empty(&self.words);
        };
        AssociationWords {
            words: &self.words,
            ids: relationships.get(id).map(Vec::as_slice).unwrap_or(&[]),
        }
    }

    /// Build the cache from dictionary entries and optional slang book.
    pub fn build(dictionary: &Dictionary, _slang: Option<&SlangBook>) -> Self {
        let mut cache = Self::empty();

        // 1. Build bidirectional synonym groups from dictionary entries.
        //    First collect all headword→synonym pairs, then insert bidirectionally.
        let mut raw_edges: HashMap<WordId, HashSet<WordId>> = HashMap::new();
        for entry in dictionary.entries() {
            let headword = entry.headword.trim();
            if headword.is_empty() {
                continue;
            }
            let headword_id = cache.intern(headword);
            for raw_syn in entry.synonyms.split('/') {
                let syn = raw_syn.trim();
                if syn.is_empty() || syn == headword {
                    continue;
                }
                let syn_id = cache.intern(syn);
                raw_edges.entry(headword_id).or_default().insert(syn_id);
                raw_edges.entry(syn_id).or_default().insert(headword_id);
            }
        }

        // Collapse edges into sorted Vecs (deduplicate + stable order for determinism)
        for (word, set) in raw_edges {
            let mut associates: Vec<WordId> = set.into_iter().collect();
            associates.sort_unstable_by(|left, right| cache.word(*left).cmp(cache.word(*right)));
            cache.word_to_associates.insert(word, associates);
        }

        // 2. Build Gan-Mandarin pair maps.
        for entry in dictionary.entries() {
            let headword = entry.headword.trim();
            let mandarin_words = distinct_mandarin_words(headword, &entry.mandarin_word);
            if mandarin_words.is_empty() {
                continue;
            }
            let headword_id = cache.intern(headword);
            let mandarin_ids: Vec<WordId> = mandarin_words
                .iter()
                .map(|word| cache.intern(word))
                .collect();
            cache
                .gan_to_mandarin
                .entry(headword_id)
                .or_default()
                .extend(mandarin_ids.iter().copied());
            for mandarin_id in mandarin_ids {
                cache
                    .mandarin_to_gan
                    .entry(mandarin_id)
                    .or_default()
                    .push(headword_id);
            }
        }

        // Sort/unique vectors for determinism.
        let words = &cache.words;
        for mandarins in cache.gan_to_mandarin.values_mut() {
            mandarins.sort_unstable_by(|left, right| {
                words[*left as usize]
                    .as_ref()
                    .cmp(words[*right as usize].as_ref())
            });
            mandarins.dedup();
        }
        for gans in cache.mandarin_to_gan.values_mut() {
            gans.sort_unstable_by(|left, right| {
                words[*left as usize]
                    .as_ref()
                    .cmp(words[*right as usize].as_ref())
            });
            gans.dedup();
        }

        cache
    }

    /// Get all words associated with `word` (synonyms + reverse synonyms).
    /// Returns empty slice if no associations.
    pub fn associates_of(&self, word: &str) -> AssociationWords<'_> {
        self.related_words(&self.word_to_associates, word)
    }

    /// Get the Mandarin equivalent of a Gan word, if any.
    pub fn mandarins_of_gan(&self, gan_word: &str) -> AssociationWords<'_> {
        self.related_words(&self.gan_to_mandarin, gan_word)
    }

    /// Get the Gan equivalents of a Mandarin word.
    pub fn gan_of_mandarin(&self, mandarin_word: &str) -> AssociationWords<'_> {
        self.related_words(&self.mandarin_to_gan, mandarin_word)
    }

    /// Check whether `word` has any Gan-Mandarin pair relationship.
    pub fn has_pair(&self, word: &str) -> bool {
        self.word_ids.get(word).is_some_and(|id| {
            self.gan_to_mandarin.contains_key(id) || self.mandarin_to_gan.contains_key(id)
        })
    }

    /// Returns true if no pair or association data exists.
    pub fn is_empty(&self) -> bool {
        self.gan_to_mandarin.is_empty()
            && self.mandarin_to_gan.is_empty()
            && self.word_to_associates.is_empty()
    }

    /// Iterator over Gan-to-Mandarin keys (for debugging).
    pub fn gan_to_mandarin_keys(&self) -> impl Iterator<Item = &str> {
        self.gan_to_mandarin.keys().map(|id| self.word(*id))
    }

    /// Get the full pair group for a word: Gan words + Mandarin word.
    /// Returns (gan_words, mandarin_word) tuple.
    /// If the input is a Gan word: returns (vec![word], its mandarin)
    /// If the input is a Mandarin word: returns (its gan equivalents, word)
    pub fn pair_group_of(&self, word: &str) -> (Vec<String>, Option<String>) {
        let mandarins = self.mandarins_of_gan(word);
        if let Some(first) = mandarins.first() {
            return (vec![word.to_string()], Some(first.to_owned()));
        }
        let gans = self.gan_of_mandarin(word);
        if !gans.is_empty() {
            return (gans.to_vec(), Some(word.to_string()));
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
