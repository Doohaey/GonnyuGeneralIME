use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrieNode {
    pub children: HashMap<char, TrieNode>,
    /// entry indices that end at this node (full-key entries)
    pub entries: Vec<usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Trie {
    pub root: TrieNode,
}

impl Trie {
    pub fn new() -> Trie {
        Trie {
            root: TrieNode {
                children: HashMap::new(),
                entries: Vec::new(),
            },
        }
    }

    /// Insert a key (already-normalized, e.g., concatenated syllables) with an entry index.
    pub fn insert(&mut self, key: &str, entry_index: usize) {
        let mut node = &mut self.root;
        for ch in key.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.entries.contains(&entry_index) {
            node.entries.push(entry_index);
        }
    }

    /// Return true if there exists any key that has `prefix` as its prefix.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            if let Some(next) = node.children.get(&ch) {
                node = next;
            } else {
                return false;
            }
        }
        true
    }

    /// Return the maximum traversed length (in chars) up to `limit` for which
    /// the trie still has a matching prefix starting at the beginning of `s`.
    /// If no prefix matches the first char, returns 0.
    pub fn max_prefix_len(&self, s: &str, limit: usize) -> usize {
        let mut node = &self.root;
        let mut count = 0usize;
        for ch in s.chars().take(limit) {
            if let Some(next) = node.children.get(&ch) {
                node = next;
                count += ch.len_utf8();
            } else {
                break;
            }
        }
        count
    }

    /// Get entries for the exact key (if present).
    pub fn entries_for_key(&self, key: &str) -> Option<&Vec<usize>> {
        let mut node = &self.root;
        for ch in key.chars() {
            node = node.children.get(&ch)?;
        }
        if node.entries.is_empty() {
            None
        } else {
            Some(&node.entries)
        }
    }
}
