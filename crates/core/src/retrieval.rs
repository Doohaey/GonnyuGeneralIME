use crate::association_cache::AssociationCache;
use crate::candidate::CandidateView;
use crate::dictionary::{
    distinct_mandarin_words, normalize_pinyin, paired_alternates_for_stored, pinyin_segments,
    strip_tone, Dictionary, DictionaryEntry, PairKind, PairedReading,
};
use crate::syllable::{FuzzyMap, SyllableScheme};
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;

type RetrieveCache = HashMap<(u64, String, bool, usize), Vec<RankedCandidate>>;

thread_local! {
    /// Per-thread cache for gan_annotation_for_mandarin_entry results,
    /// cleared at the start of each retrieve_inner call.
    static ANNOTATION_CACHE: RefCell<HashMap<String, Option<String>>> = RefCell::new(HashMap::new());
    /// Per-thread cache for retrieve_inner_limited results, keyed by
    /// (dictionary_id, normalized_input, skip_step7, max_candidates).  Capped at 512
    /// entries to bound memory.
    static INNER_RETRIEVE_CACHE: RefCell<RetrieveCache> = RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetrievalLayer {
    /// Exact hit via dialect (Gan) pinyin — including tone-stripped and
    /// checked-tone variants.
    GannyuExact,
    /// Exact hit via Mandarin pinyin — including tone-stripped variants.
    MandarinExact,
    /// Fuzzy hit via onset/nucleus/coda/tone substitution rules.
    Fuzzy,
    /// Synonym cross-reference.
    Synonym,
}

impl RetrievalLayer {
    pub fn base_weight(self) -> f64 {
        match self {
            RetrievalLayer::GannyuExact => 5.0,
            RetrievalLayer::MandarinExact => 4.5,
            RetrievalLayer::Fuzzy => 4.0,
            RetrievalLayer::Synonym => 4.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RankedCandidate {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation: Option<String>,
    pub ipa: Option<String>,
    pub layer: RetrievalLayer,
    pub mandarin_only: bool,
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mandarin_reading: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    pub consumed_bytes: usize,
}

impl CandidateView for RankedCandidate {
    fn text(&self) -> &str {
        &self.text
    }

    fn reading(&self) -> Option<&str> {
        self.reading.as_deref()
    }
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

include!("retrieval/candidates.rs");
include!("retrieval/segmentation.rs");
include!("retrieval/query.rs");

#[cfg(test)]
mod tests {
    use super::*;

    fn gan_entry(headword: &str, dialect_pinyin: &str, frequency: u64) -> DictionaryEntry {
        DictionaryEntry {
            headword: headword.to_string(),
            ipa: String::new(),
            dialect_pinyin: dialect_pinyin.to_string(),
            mandarin_pinyin: String::new(),
            category: "赣".to_string(),
            mandarin_word: String::new(),
            mandarin_word_pinyin: String::new(),
            frequency: Some(frequency),
            synonyms: String::new(),
            entry_index: 0,
            new_old: String::new(),
        }
    }

    /// 语 = nyu3(老) / yu3(新), 言 = nien4(老) / yen4(新);
    /// 语言 stores only the old readings `nyu3 nien4`.
    fn multi_reading_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut yu_old = gan_entry("语", "nyu3", 80000);
        yu_old.new_old = "老1".to_string();
        let mut yu_new = gan_entry("语", "yu3", 60000);
        yu_new.new_old = "新1".to_string();
        let mut yan_old = gan_entry("言", "nien4", 70000);
        yan_old.new_old = "老1".to_string();
        let mut yan_new = gan_entry("言", "yen4", 50000);
        yan_new.new_old = "新1".to_string();
        dictionary.extend_from_entries([
            yu_old,
            yu_new,
            yan_old,
            yan_new,
            gan_entry("语言", "nyu3 nien4", 90000),
        ]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn heteronym_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut base = gan_entry("手", "shou1", 80000);
        base.new_old = "本1".to_string();
        let mut variant = gan_entry("手", "sou1", 60000);
        variant.new_old = "又1".to_string();
        dictionary.extend_from_entries([base, variant, gan_entry("手心", "shou1 xin1", 90000)]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn multi_heteronym_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut hat = gan_entry("还", "hat6", 90000);
        hat.new_old = "本1".to_string();
        let mut hai = gan_entry("还", "hai6", 80000);
        hai.new_old = "又1".to_string();
        let mut wan = gan_entry("还", "wan6", 70000);
        wan.new_old = "本2".to_string();
        let mut fan = gan_entry("还", "fan6", 60000);
        fan.new_old = "又2".to_string();
        dictionary.extend_from_entries([hat, hai, wan, fan, gan_entry("还有", "hat6 yiu3", 95000)]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn mixed_pair_priority_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut new = gan_entry("横", "vang2", 90000);
        new.new_old = "新1".to_string();
        let mut old = gan_entry("横", "wang2", 80000);
        old.new_old = "老1".to_string();
        let mut base = gan_entry("横", "vang2", 70000);
        base.new_old = "本2".to_string();
        let mut variant = gan_entry("横", "fang2", 60000);
        variant.new_old = "又2".to_string();
        let mut wen = gan_entry("明", "ming5", 90000);
        wen.category = "文".to_string();
        let mut bai = gan_entry("明", "miang5", 80000);
        bai.category = "白".to_string();
        dictionary.extend_from_entries([
            new,
            old,
            base,
            variant,
            gan_entry("横额", "vang2 ngak8", 85000),
            wen,
            bai,
            gan_entry("明年", "ming5 nien4", 83000),
        ]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn heteronym_neutral_word_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut base = gan_entry("辑", "qit6", 80000);
        base.new_old = "本1".to_string();
        let mut variant = gan_entry("辑", "jit6", 60000);
        variant.new_old = "又1".to_string();
        dictionary.extend_from_entries([base, variant, gan_entry("逻辑", "lo5 qit0", 90000)]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn heteronym_neutral_word_with_nonmatching_first_pair_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        let mut base1 = gan_entry("辑", "lap6", 90000);
        base1.new_old = "本1".to_string();
        let mut variant1 = gan_entry("辑", "nap6", 70000);
        variant1.new_old = "又1".to_string();
        let mut base2 = gan_entry("辑", "qit6", 80000);
        base2.new_old = "本2".to_string();
        let mut variant2 = gan_entry("辑", "jit6", 60000);
        variant2.new_old = "又2".to_string();
        dictionary.extend_from_entries([
            base1,
            variant1,
            base2,
            variant2,
            gan_entry("逻辑", "lo5 qit0", 90000),
        ]);
        dictionary.rebuild_new_old_map();
        dictionary.rebuild_multi_reading_augmentation();
        dictionary
    }

    fn candidate_view(candidates: &[RankedCandidate], text: &str) -> Option<(RetrievalLayer, f64)> {
        candidates
            .iter()
            .find(|candidate| candidate.text == text)
            .map(|candidate| (candidate.layer, candidate.weight))
    }

    fn sentence_suffix_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries([
            gan_entry("南昌", "lan4 cong1", 90000),
            gan_entry("南昌话", "lan4 cong1 wa5", 120000),
            gan_entry("方言", "fong1 nien4", 80000),
            gan_entry("方言里边", "fong1 nien4 li3 bien1", 140000),
        ]);
        dictionary
    }

    fn sentence_prefix_groups_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries([
            gan_entry("蓝", "lan2", 70000),
            gan_entry("南昌", "lan4 cong1", 90000),
            gan_entry("南昌市", "lan4 cong1 si5", 85000),
            gan_entry("南昌话", "lan4 cong1 wa5", 120000),
        ]);
        dictionary
    }

    fn ngomengo_dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries([
            gan_entry("我们", "ngo3 men4", 298740),
            gan_entry("嗰", "go0", 442749),
            gan_entry("咯", "gok6", 241302),
            gan_entry("箇只", "go3 zat6", 300000),
        ]);
        dictionary
    }

    fn sentence_mixed_suffix_priority_dictionary(exact_freq: u64, fuzzy_freq: u64) -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries([
            gan_entry("佢", "jie3", 120000),
            gan_entry("们", "men4", 110000),
            gan_entry("嗰", "go0", exact_freq),
            gan_entry("咯", "gok6", fuzzy_freq),
        ]);
        dictionary
    }

    fn sentence_entering_tone_ambiguity_dictionary(exact_freq: u64, fuzzy_freq: u64) -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.extend_from_entries([
            gan_entry("佢", "jie3", 120000),
            gan_entry("们", "men4", 110000),
            gan_entry("嗰", "go0", exact_freq),
            gan_entry("咯", "gok6", fuzzy_freq),
            gan_entry("外面", "wai5 mien5", 4049),
        ]);
        dictionary
    }

    /// Multi-reading 等权: a word typed with an alternate reading of one of its
    /// characters gets the same GannyuExact layer and weight as when typed
    /// with the stored reading — only frequency_factor shapes the weight.
    #[test]
    fn finite_retrieval_keeps_relationships_for_selected_bases() {
        let mut dictionary = Dictionary::empty();
        let mut base = gan_entry("本词", "ben", 100);
        base.synonyms = "关联甲/关联乙".to_string();
        dictionary.extend_from_entries([
            base,
            gan_entry("关联甲", "ga", 10),
            gan_entry("关联乙", "yi", 9),
            gan_entry("次词", "ben", 50),
        ]);
        let result = retrieve_inner_limited(
            &dictionary,
            &FuzzyMap {
                entries: Vec::new(),
            },
            &HashMap::new(),
            "ben",
            false,
            1,
        );
        let texts: Vec<&_> = result
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect();
        assert_eq!(texts, vec!["本词", "关联甲", "关联乙"]);
    }

    #[test]
    fn dictionary_cache_identity_isolated_between_instances() {
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tones = HashMap::new();
        let mut first = Dictionary::empty();
        first.extend_from_entries([gan_entry("甲词", "ga", 100)]);
        let first_result = retrieve_inner_limited(&first, &fuzzy, &tones, "ga", false, 1);
        assert_eq!(first_result[0].text, "甲词");

        let mut second = Dictionary::empty();
        second.extend_from_entries([gan_entry("乙词", "ga", 100)]);
        let second_result = retrieve_inner_limited(&second, &fuzzy, &tones, "ga", false, 1);
        assert_eq!(second_result[0].text, "乙词");
    }

    #[test]
    fn equal_frequency_results_have_stable_dictionary_order() {
        let mut dictionary = Dictionary::empty();
        dictionary
            .extend_from_entries([gan_entry("甲词", "ga", 100), gan_entry("乙词", "ga", 100)]);
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tones = HashMap::new();
        let result = retrieve_inner_limited(&dictionary, &fuzzy, &tones, "ga", false, 2);
        assert_eq!(
            result
                .iter()
                .map(|candidate| candidate.text.as_str())
                .collect::<Vec<_>>(),
            vec!["甲词", "乙词"]
        );
    }

    #[test]
    fn alternate_reading_spelled_out_input_is_gannyu_exact() {
        let dictionary = multi_reading_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let stored = retrieve(&dictionary, &fuzzy, &tone_values, "nyu nien");
        let alternate = retrieve(&dictionary, &fuzzy, &tone_values, "yu yen");

        let (stored_layer, stored_weight) =
            candidate_view(&stored, "语言").expect("语言 via stored reading");
        let (alternate_layer, alternate_weight) =
            candidate_view(&alternate, "语言").expect("语言 via alternate reading");
        assert_eq!(stored_layer, RetrievalLayer::GannyuExact);
        assert_eq!(alternate_layer, RetrievalLayer::GannyuExact);
        assert_eq!(stored_weight, alternate_weight);
    }

    #[test]
    fn alternate_reading_continuous_input_is_gannyu_exact() {
        let dictionary = multi_reading_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let stored = retrieve(&dictionary, &fuzzy, &tone_values, "nyunien");
        let alternate = retrieve(&dictionary, &fuzzy, &tone_values, "yuyen");

        let (stored_layer, stored_weight) =
            candidate_view(&stored, "语言").expect("语言 via stored reading");
        let (alternate_layer, alternate_weight) =
            candidate_view(&alternate, "语言").expect("语言 via alternate reading");
        assert_eq!(stored_layer, RetrievalLayer::GannyuExact);
        assert_eq!(alternate_layer, RetrievalLayer::GannyuExact);
        assert_eq!(stored_weight, alternate_weight);
    }

    #[test]
    fn alternate_reading_sentence_input_is_gannyu_exact() {
        let dictionary = multi_reading_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let stored = retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "nyunien", None);
        let alternate = retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "yuyen", None);

        let (stored_layer, stored_weight) =
            candidate_view(&stored, "语言").expect("语言 via stored reading");
        let (alternate_layer, alternate_weight) =
            candidate_view(&alternate, "语言").expect("语言 via alternate reading");
        assert_eq!(stored_layer, RetrievalLayer::GannyuExact);
        assert_eq!(alternate_layer, RetrievalLayer::GannyuExact);
        assert_eq!(stored_weight, alternate_weight);
    }

    #[test]
    fn heteronym_word_annotation_marks_only_the_second_reading() {
        let dictionary = heteronym_dictionary();
        let entry = dictionary.by_headword("手心")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, entry, &HashMap::new()).as_deref(),
            Some("(shou1/[又]sou1) xin1"),
        );

        let new_old = multi_reading_dictionary();
        let entry = new_old.by_headword("语言")[0];
        assert_eq!(
            annotation_for_entry(&new_old, entry, &HashMap::new()).as_deref(),
            Some("([新]yu3/[老]nyu3) ([新]yen4/[老]nien4)"),
        );
    }

    #[test]
    fn single_char_heteronym_annotation_shows_full_pair() {
        let dictionary = heteronym_dictionary();
        let base = dictionary
            .by_headword("手")
            .into_iter()
            .find(|entry| entry.dialect_pinyin == "shou1")
            .unwrap();
        let variant = dictionary
            .by_headword("手")
            .into_iter()
            .find(|entry| entry.dialect_pinyin == "sou1")
            .unwrap();
        assert_eq!(
            annotation_for_entry(&dictionary, base, &HashMap::new()).as_deref(),
            Some("shou1/[又]sou1"),
        );
        assert_eq!(
            annotation_for_entry(&dictionary, variant, &HashMap::new()).as_deref(),
            Some("shou1/[又]sou1"),
        );
    }

    #[test]
    fn neutral_word_annotation_uses_matching_pair_but_single_char_neutral_stays_suppressed() {
        let dictionary = heteronym_neutral_word_dictionary();
        let word = dictionary.by_headword("逻辑")[0];
        let char_entry = dictionary
            .by_headword("辑")
            .into_iter()
            .find(|entry| entry.dialect_pinyin == "qit6")
            .unwrap();
        assert_eq!(
            annotation_for_entry(&dictionary, word, &HashMap::new()).as_deref(),
            Some("lo5 (qit0/[又]jit0)"),
        );
        assert_eq!(
            annotation_for_entry(&dictionary, char_entry, &HashMap::new()).as_deref(),
            Some("qit6/[又]jit6"),
        );
    }

    #[test]
    fn neutral_word_annotation_skips_nonmatching_pairs_until_it_finds_a_matching_one() {
        let dictionary = heteronym_neutral_word_with_nonmatching_first_pair_dictionary();
        let entry = dictionary.by_headword("逻辑")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, entry, &HashMap::new()).as_deref(),
            Some("lo5 (qit0/[又]jit0)"),
        );
    }

    #[test]
    fn heteronym_retrieval_stays_within_matched_pair_only() {
        let dictionary = multi_heteronym_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();
        assert!(retrieve(&dictionary, &fuzzy, &tone_values, "hatyiu")
            .iter()
            .any(|candidate| candidate.text == "还有"));
        assert!(retrieve(&dictionary, &fuzzy, &tone_values, "haiyiu")
            .iter()
            .any(|candidate| candidate.text == "还有"));
        assert!(retrieve(&dictionary, &fuzzy, &tone_values, "wanyiu")
            .iter()
            .all(|candidate| candidate.text != "还有"));
        assert!(retrieve(&dictionary, &fuzzy, &tone_values, "fanyiu")
            .iter()
            .all(|candidate| candidate.text != "还有"));
        let entry = dictionary.by_headword("还有")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, entry, &HashMap::new()).as_deref(),
            Some("(hat6/[又]hai6) yiu3"),
        );
    }

    #[test]
    fn subtitle_prefers_newold_over_heteronym_and_keeps_wenbai_nonpaired() {
        let dictionary = mixed_pair_priority_dictionary();
        let heng = dictionary.by_headword("横额")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, heng, &HashMap::new()).as_deref(),
            Some("([新]vang2/[老]wang2) ngak8"),
        );
        let ming = dictionary.by_headword("明年")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, ming, &HashMap::new()).as_deref(),
            Some("ming5 nien4"),
        );
    }

    #[test]
    fn unpaired_word_reading_is_not_replaced_by_pair() {
        let mut dictionary = multi_reading_dictionary();
        dictionary.extend_from_entries([gan_entry("谜语", "mi5 xi5", 50000)]);
        let entry = dictionary.by_headword("谜语")[0];
        assert_eq!(
            annotation_for_entry(&dictionary, entry, &HashMap::new()).as_deref(),
            Some("mi5 xi5"),
        );
    }

    #[test]
    fn sentence_input_prunes_only_overcombined_sentence_candidates() {
        let dictionary = sentence_suffix_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let candidates = retrieve_sentence_input(
            &dictionary,
            &fuzzy,
            &tone_values,
            "lancongfongnienlibien",
            None,
        );

        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.text != "南昌话方言里边"),
            "suffix-inconsistent combined sentence candidates should be pruned"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.text != "南昌话"),
            "南昌话's 3rd syllable wa does not match input's fong, so it should be pruned"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌" && candidate.consumed_bytes == 7),
            "decremental lancong prefix candidates should still remain visible"
        );
    }

    #[test]
    fn sentence_input_keeps_candidates_when_suffix_completes_them() {
        let dictionary = sentence_suffix_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let candidates = retrieve_sentence_input(
            &dictionary,
            &fuzzy,
            &tone_values,
            "lancongwafongnienlibien",
            None,
        );

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌话方言里边"),
            "suffix-consistent combined candidate should remain available"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌话" && candidate.consumed_bytes == 9),
            "real prefix candidates should retain their matched boundary"
        );
    }

    #[test]
    fn sentence_input_keeps_standalone_completion_for_shorter_prefix() {
        let dictionary = sentence_suffix_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "lancong", None);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌话"),
            "shorter standalone prefixes should keep extendable completions"
        );
    }

    #[test]
    fn sentence_input_preserves_decremental_prefix_groups_at_first_position() {
        let dictionary = sentence_prefix_groups_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let tone_values = HashMap::new();

        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "lancongwa", None);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌话" && candidate.consumed_bytes == 9),
            "full-prefix group should keep 南昌话"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "南昌" && candidate.consumed_bytes == 7),
            "decremental lancong group should keep 南昌"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.text != "南昌市"),
            "南昌市's 3rd syllable si5 does not match input's wa, so it should be pruned"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == "蓝" && candidate.consumed_bytes == 3),
            "decremental lan group should keep single-syllable matches"
        );
    }

    #[test]
    fn sentence_input_prefers_higher_weight_exact_suffix() {
        let dictionary = sentence_mixed_suffix_priority_dictionary(100000, 0);
        let fuzzy = FuzzyMap::load_tsv(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/fuzzy_scheme.tsv"
        ))
        .expect("load fuzzy");
        let tone_values = HashMap::new();

        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "jiemengo", None);

        assert!(
            !candidates.is_empty(),
            "expected sentence candidates for jiemengo"
        );
        assert_eq!(
            candidates[0].text, "佢们嗰",
            "higher content weight should keep 嗰 as the combined sentence candidate",
        );
    }

    #[test]
    fn ngomengo_prefers_open_go_suffix() {
        let dictionary = ngomengo_dictionary();
        let fuzzy = FuzzyMap {
            entries: Vec::new(),
        };
        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &HashMap::new(), "ngomengo", None);
        assert_eq!(
            candidates.first().map(|candidate| candidate.text.as_str()),
            Some("我们嗰")
        );
        assert_eq!(
            candidates
                .first()
                .map(|candidate| candidate.reading.as_deref()),
            Some(Some("ngo3 men4 go0"))
        );
    }

    #[test]
    fn longer_completion_does_not_cross_sentence_boundary() {
        let dictionary = ngomengo_dictionary();
        let entry_index = dictionary.by_headword("箇只")[0].entry_index;
        let segment = SyllableCandidate {
            text: "go".to_string(),
            ids: HashSet::from([entry_index]),
            tier: SegmentTier::GanExact,
            profile: 0,
        };
        assert!(!candidate_matches_sentence_segments(
            &dictionary,
            "箇只",
            &[segment],
        ));
    }

    #[test]
    fn h_coda_does_not_create_open_syllable_alternative() {
        let path = vec![SyllableCandidate {
            text: "pah".to_string(),
            ids: HashSet::new(),
            tier: SegmentTier::GanCompatible,
            profile: 0,
        }];
        let mut segmentation_cache = SegmentationCache::default();
        let mut path_cache = HashMap::new();
        assert!(no_tail_entering_alternative_path(
            &Dictionary::empty(),
            &FuzzyMap {
                entries: Vec::new()
            },
            "pa",
            &path,
            &mut segmentation_cache,
            &mut path_cache,
        )
        .is_none());
    }

    #[test]
    fn sentence_input_prefers_higher_weight_fuzzy_suffix() {
        let dictionary = sentence_mixed_suffix_priority_dictionary(0, 100000);
        let fuzzy = FuzzyMap::load_tsv(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/fuzzy_scheme.tsv"
        ))
        .expect("load fuzzy");
        let tone_values = HashMap::new();

        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "jiemengo", None);

        assert!(
            !candidates.is_empty(),
            "expected sentence candidates for jiemengo"
        );
        assert_eq!(
            candidates[0].text, "佢们咯",
            "higher content weight should let entering-tone fuzzy 咯 win the combined sentence candidate",
        );
    }

    #[test]
    fn sentence_input_keeps_alternative_open_syllable_path_for_entering_tone_ambiguity() {
        let dictionary = sentence_entering_tone_ambiguity_dictionary(100000, 0);
        let fuzzy = FuzzyMap::load_tsv(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/fuzzy_scheme.tsv"
        ))
        .expect("load fuzzy");
        let tone_values = HashMap::new();

        let candidates =
            retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "jiemengowaimien", None);

        assert!(
            !candidates.is_empty(),
            "expected sentence candidates for jiemengowaimien"
        );
        assert_eq!(
            candidates[0].text, "佢们嗰外面",
            "no-tail entering-tone ambiguity should retain the open-syllable path when it yields the stronger candidate",
        );
    }

    #[test]
    fn test_coda_doubled_variants() {
        // niteu → nitteu so it segments as nit+teu
        let vars = coda_doubled_variants("niteu");
        assert_eq!(vars, vec!["nitteu"]);

        // nikteu → nikkteu tries nik+kteu (falls back to nik+teu via fuzzy);
        // t at pos 3 also doubles
        let vars = coda_doubled_variants("nikteu");
        assert_eq!(vars, vec!["nikkteu", "niktteu"]);

        // Multiple t/k: one variant per coda position
        let vars = coda_doubled_variants("takat");
        // t at 0, k at 2, t at 4 → 3 positions
        assert_eq!(vars, vec!["ttakat", "takkat", "takatt"]);

        // h/p are not checked codas: no doubling
        let vars = coda_doubled_variants("pahong");
        assert!(vars.is_empty());

        // No coda consonants
        let vars = coda_doubled_variants("abc");
        assert!(vars.is_empty());
    }
}
