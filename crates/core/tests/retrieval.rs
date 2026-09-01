use ahash::AHashMap as HashMap;
use gannyu_input_core::{
    format_preedit_display, retrieve, retrieve_sentence_input, retrieve_with_manual_segments,
    segment_boundaries, segment_sentence, Dictionary, FuzzyMap, RetrievalLayer,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_fixture(name: &str, content: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "gannyu-retrieval-{}-{}-{}",
        std::process::id(),
        unique,
        name
    ));
    fs::write(&path, content).expect("fixture write");
    path
}

const DICT_HEADER: &str =
    "本词\t国际音标\t方言拼音\t汉语拼音\t词汇属性\t对应官话词\t官话拼音\t词频\t同义词\t新旧标记";

fn sample_dictionary() -> Dictionary {
    let body = format!(
        "{DICT_HEADER}\n\
         渠\ttɕʰy21\tqu\tqu2\t赣\t他\tta1\t5000\n\
         佢\ttɕʰy21\tqu\tqu2\t赣\t他\tta1\t100\n\
         去\ttɕʰy42\tqu\tqu4\t赣\t去\tqu4\t3000\n\
         在\ttsai6\tcoi\tzai4\t赣\t在\tzai4\t2400\n\
         许只\the21 zat5\the'zat\txu3 zhi3\t赣\t那个\tna4 ge5\t1200\n\
         个\tgo5\tgo\tge4\t赣\t这\tzhe4\t2000\n\
         箇\tgo5\tgo\tge4\t赣\t这\tzhe4\t1500\n\
         个里\tgo5 li3\tgo li\tge4 li3\t赣\t这里\tzhe4 li3\t800\n\
         这\t\tze\tzhe4\t官\t这\tzhe4\t20\n\
         这里\t\tze li\tzhe4 li3\t官\t这里\tzhe4 li3\t19\n\
         哪\tna213\tna\tna3\t官\t哪\tna3\t20\n\
         脚\ttɕiɔk5\tgiok\tjiao3\t赣\t脚\tjiao3\t800\n"
    );
    let path = write_fixture("dict.tsv", &body);
    Dictionary::load_tsv(&path).expect("load dictionary")
}

fn sample_fuzzy() -> FuzzyMap {
    let body = "category\tgon_han\tgon_pin\tapplies\tbidirectional\tpriority_tier\n\
                onset\tgi\tji\tsyllable-initial\ttrue\tsecondary\n";
    let path = write_fixture("fuzzy.tsv", body);
    FuzzyMap::load_tsv(&path).expect("load fuzzy")
}

fn repo_fuzzy() -> FuzzyMap {
    let path = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/fuzzy_scheme.tsv"
    ));
    FuzzyMap::load_tsv(&path).expect("load fuzzy")
}

fn tone_values() -> HashMap<String, u8> {
    let mut map = HashMap::new();
    map.insert("42".to_string(), 1);
    map.insert("21".to_string(), 5);
    map.insert("5".to_string(), 6);
    map
}

#[test]
fn gan_reading_is_first_layer() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "qu");
    assert!(!result.is_empty());
    assert_eq!(result[0].layer, RetrievalLayer::GannyuExact);
    assert_eq!(result[0].text, "渠");
    // Higher frequency Gan word ranks before the rarer homophone.
    // Note: 他 (mandarin_word of 渠) now also appears directly via
    // the unified lookup (no category filtering).
    let qu_texts: Vec<&str> = result
        .iter()
        .filter(|item| item.layer == RetrievalLayer::GannyuExact)
        .filter(|item| item.text != "他") // 他 is cross-reference, not direct hit
        .map(|item| item.text.as_str())
        .collect();
    assert_eq!(qu_texts, vec!["渠", "去", "佢"]);
}

#[test]
fn gan_exact_weights_higher_than_mandarin_and_fuzzy() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "qu");
    let gan = result
        .iter()
        .find(|item| item.layer == RetrievalLayer::GannyuExact)
        .unwrap()
        .weight;
    // GannyuExact base = 5.0, MandarinExact = 4.5, Fuzzy = 4.0.
    assert!(gan >= RetrievalLayer::GannyuExact.base_weight());
    // Query "jiok" should trigger fuzzy match (gi↔ji onset rule).
    let fuzzy_result = retrieve(&dictionary, &fuzzy, &tones, "jiok");
    let fuzzy_hit = fuzzy_result
        .iter()
        .find(|item| item.layer == RetrievalLayer::Fuzzy);
    assert!(fuzzy_hit.is_some());
    assert!(fuzzy_hit.unwrap().weight < gan);
}

#[test]
fn gan_pinyin_annotation_carries_tone_class() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "qu");
    let qu = result.iter().find(|item| item.text == "渠").unwrap();
    // IPA tone value 21 maps to tone class 5. 渠 has mandarin_word=他 so annotation includes [义]他.
    assert_eq!(qu.annotation.as_deref(), Some("qu5 [义]他"));
    assert!(!qu.mandarin_only);
}

#[test]
fn mandarin_only_entry_is_tagged() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    // 哪 is a Mandarin-only entry with no native Gan equivalent; it gets the
    // [不习用] tag with no further reading or reverse annotation.
    let result = retrieve(&dictionary, &fuzzy, &tones, "na3");
    let mandarin = result.iter().find(|item| item.text == "哪").unwrap();
    assert!(mandarin.mandarin_only);
    assert_eq!(mandarin.annotation.as_deref(), Some("[不习用]"));
}

#[test]
fn mandarin_word_is_inserted_behind_gan_word() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "ta1");
    let qu_index = result.iter().position(|item| item.text == "渠").unwrap();
    let gi_index = result.iter().position(|item| item.text == "佢").unwrap();
    let mandarin_index = result.iter().position(|item| item.text == "他").unwrap();
    assert!(mandarin_index > qu_index);
    assert!(gi_index > mandarin_index);
    let mandarin = &result[mandarin_index];
    assert_eq!(
        mandarin.annotation.as_deref(),
        Some("[不习用] [习用]渠（qu5）/佢（qu5）")
    );
}

#[test]
fn annotation_shows_bidirectional_associated_words() {
    let body = format!(
        "{DICT_HEADER}\n\
         青菜\ttɕʰiaŋ1 tsʰai3\tqiang cai\tqing1 cai4\t赣\t蔬菜\tshu1 cai4\t90000\t白菜/菜蔬\n\
         白菜\tpai2 tsʰai3\tbai cai\tbai2 cai4\t赣\t白菜\tbai2 cai4\t80000\t\n\
         菜蔬\ttsʰai3 su1\tcai su\tcai4 shu1\t赣\t菜蔬\tcai4 shu1\t70000\t\n\
         小菜\tsiɛu3 tsʰai3\tsieu cai\txiao3 cai4\t赣\t小菜\txiao3 cai4\t60000\t青菜\n"
    );
    let path = write_fixture("assoc.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let result = retrieve(&dictionary, &fuzzy, &tones, "qiangcai");
    let qingcai = result.iter().find(|item| item.text == "青菜").unwrap();
    assert_eq!(
        qingcai.annotation.as_deref(),
        Some("qiang cai [义]蔬菜, [联]小菜/白菜/菜蔬")
    );

    let reverse = retrieve(&dictionary, &fuzzy, &tones, "sieu cai");
    let xiaocai = reverse.iter().find(|item| item.text == "小菜").unwrap();
    assert_eq!(xiaocai.annotation.as_deref(), Some("sieu cai, [联]青菜"));
}

#[test]
fn multi_mandarin_counterparts_expand_annotations_and_candidates() {
    let body = format!(
        "{DICT_HEADER}\n\
         青菜\ttɕʰiaŋ1 tsʰai3\tqiang cai\tqing1 cai4\t赣\t蔬菜/菜\tshu1 cai4/cai4\t90000\t\n"
    );
    let path = write_fixture("multi-mandarin-retrieval.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let gan = retrieve(&dictionary, &fuzzy, &tones, "qiangcai");
    let qingcai = gan.iter().find(|item| item.text == "青菜").unwrap();
    assert_eq!(qingcai.annotation.as_deref(), Some("qiang cai [义]蔬菜/菜"));
    assert!(gan.iter().any(|item| item.text == "蔬菜"));
    assert!(gan.iter().any(|item| item.text == "菜"));

    let reverse = retrieve(&dictionary, &fuzzy, &tones, "cai4");
    let qingcai_from_cai = reverse.iter().find(|item| item.text == "青菜").unwrap();
    assert_eq!(
        qingcai_from_cai.annotation.as_deref(),
        Some("qiang cai [义]蔬菜/菜")
    );
}

#[test]
fn fuzzy_layer_matches_alternate_spelling() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    // Input ji fuzzes to gi (onset rule), matching 脚 (dialect pinyin giok)?
    // The rule maps initial gi<->ji at syllable start; input "jiok" fuzzes to "giok".
    let result = retrieve(&dictionary, &fuzzy, &tones, "jiok");
    let jiao = result.iter().find(|item| item.text == "脚");
    assert!(jiao.is_some());
    assert_eq!(jiao.unwrap().layer, RetrievalLayer::Fuzzy);
}

#[test]
fn continuous_dialect_input_matches_multi_syllable_word() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "hezat");
    let hit = result.iter().find(|item| item.text == "许只");
    assert!(hit.is_some());
}

#[test]
fn mandarin_lookup_works_without_tone_digits() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let zhe = retrieve(&dictionary, &fuzzy, &tones, "zhe");
    assert!(zhe.iter().any(|item| item.text == "个"));
    assert!(zhe.iter().any(|item| item.text == "箇"));
    let zai = retrieve(&dictionary, &fuzzy, &tones, "zai");
    assert!(zai.iter().any(|item| item.text == "在"));
    let ta = retrieve(&dictionary, &fuzzy, &tones, "ta");
    assert!(ta.iter().any(|item| item.text == "渠"));
}

#[test]
fn mandarin_alias_appears_naturally_by_frequency() {
    let dictionary = sample_dictionary();
    let fuzzy = sample_fuzzy();
    let tones = tone_values();
    let result = retrieve(&dictionary, &fuzzy, &tones, "zhe");
    let ge_index = result.iter().position(|item| item.text == "个").unwrap();
    let go_index = result.iter().position(|item| item.text == "箇").unwrap();
    let zhe_index = result.iter().position(|item| item.text == "这").unwrap();
    // Higher-frequency Gan entries rank before lower-frequency Mandarin alias.
    assert!(zhe_index > ge_index);
    assert!(go_index > zhe_index);

    let zheli = retrieve(&dictionary, &fuzzy, &tones, "zheli");
    let gan_index = zheli.iter().position(|item| item.text == "个里").unwrap();
    let mandarin_index = zheli.iter().position(|item| item.text == "这里").unwrap();
    assert!(mandarin_index > gan_index);
}

#[test]
fn multi_reading_annotation_aggregates_register_labels() {
    let body = format!(
        "{DICT_HEADER}\n\
         明\tmin21\tming\tming2\t文\t\t\t5000\n\
         明\tmiaŋ21\tmiang\tming2\t白\t\t\t4000\n\
         明年\tmiaŋ21 ȵiɛn21\tmiang nien\tming2 nian2\t白\t\t\t1200\n"
    );
    let path = write_fixture("dict-multi.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let wen = retrieve(&dictionary, &fuzzy, &tones, "ming");
    let bai = retrieve(&dictionary, &fuzzy, &tones, "miang");
    let ming = wen.iter().find(|item| item.text == "明").unwrap();
    let miang = bai.iter().find(|item| item.text == "明").unwrap();
    let next_year = retrieve(&dictionary, &fuzzy, &tones, "miangnien");
    let ming_nien = next_year.iter().find(|item| item.text == "明年").unwrap();

    assert_eq!(ming.annotation.as_deref(), Some("[文]ming5 [白]miang5"));
    assert_eq!(miang.annotation.as_deref(), Some("[文]ming5 [白]miang5"));
    assert_eq!(ming_nien.annotation.as_deref(), Some("[白]miang5 nien5"));
}

#[test]
fn single_char_annotation_keeps_only_neutral_reading() {
    let body = format!(
        "{DICT_HEADER}\n\
         呣\t\tm\t\t赣\t\t\t5000\n"
    );
    let path = write_fixture("dict-neutral-only.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let result = retrieve(&dictionary, &fuzzy, &tones, "m");
    let item = result.iter().find(|item| item.text == "呣").unwrap();

    assert_eq!(item.annotation.as_deref(), Some("m"));
}

#[test]
fn single_char_annotation_hides_neutral_duplicate_base() {
    let body = format!(
        "{DICT_HEADER}\n\
         么\t\tma\t\t赣\t\t\t3000\n\
         么\t\tma3\t\t赣\t\t\t2000\n\
         嚜\t\tma\t\t赣\t\t\t1000\n"
    );
    let path = write_fixture("dict-neutral-duplicate.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let result = retrieve(&dictionary, &fuzzy, &tones, "ma");
    let item = result.iter().find(|item| item.text == "么").unwrap();
    let only_neutral = result.iter().find(|item| item.text == "嚜").unwrap();

    assert_eq!(item.annotation.as_deref(), Some("ma3"));
    assert_eq!(only_neutral.annotation.as_deref(), Some("ma"));
}

#[test]
fn multichar_annotation_keeps_neutral_duplicate_base() {
    let body = format!(
        "{DICT_HEADER}\n\
         么么\t\tma ma\t\t赣\t\t\t3000\n\
         么么\t\tma3 ma3\t\t赣\t\t\t2000\n"
    );
    let path = write_fixture("dict-multichar-neutral-duplicate.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let result = retrieve(&dictionary, &fuzzy, &tones, "mama");
    let item = result.iter().find(|item| item.text == "么么").unwrap();

    assert_eq!(item.annotation.as_deref(), Some("ma ma / ma3 ma3"));
}

#[test]
fn mixed_gan_mandarin_pinyin_hits_zhonghuarenmin() {
    let body = format!(
        "{DICT_HEADER}\n\
         中华人民\t\tzung1 fa4 nin2 min2\tzhong1 hua2 ren2 min2\t赣\t\t\t30000\n"
    );
    let path = write_fixture("dict-mixed.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    // All Mandarin pinyin.
    assert!(retrieve(&dictionary, &fuzzy, &tones, "zhong hua ren min")
        .iter()
        .any(|c| c.text == "中华人民"));
    // Mixed: 中=Mandarin, 华=Gan, others=Mandarin
    assert!(retrieve(&dictionary, &fuzzy, &tones, "zhong fa ren min")
        .iter()
        .any(|c| c.text == "中华人民"));
    // Mixed: 中=Gan, 华=Mandarin, others=Mandarin
    assert!(retrieve(&dictionary, &fuzzy, &tones, "zung hua ren min")
        .iter()
        .any(|c| c.text == "中华人民"));
    // All Gan pinyin: zung fa nin min
    assert!(retrieve(&dictionary, &fuzzy, &tones, "zung fa nin min")
        .iter()
        .any(|c| c.text == "中华人民"));
    // Continuous input (no spaces) — mixed pinyin
    assert!(retrieve(&dictionary, &fuzzy, &tones, "zunghuarenmin")
        .iter()
        .any(|c| c.text == "中华人民"));
}

#[test]
fn segment_sentence_finds_multi_word_input() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\tlan4 tsʰɔŋ1\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
        话\twa5\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-seg.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load dictionary");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let segments = segment_sentence(&dictionary, &fuzzy, &tones, "lancongwa");
    assert_eq!(segments.len(), 3);
    assert!(segments[0].iter().any(|c| c.text == "南昌"));
    assert!(segments[2].iter().any(|c| c.text == "话"));
}

#[test]
fn real_dict_segmentation_works() {
    let dict_dir = std::path::PathBuf::from("resources/regions/lancong/dictionaries");
    let dict_files = ["chars.tsv", "words.tsv", "gan_chars.tsv", "gan_words.tsv"];
    if !dict_files.iter().all(|name| dict_dir.join(name).exists()) {
        return;
    }
    let mut dictionary = Dictionary::empty();
    let mut loaded = false;
    for name in dict_files {
        let path = dict_dir.join(name);
        if loaded {
            dictionary.extend_from_tsv(&path).expect("extend real dict");
        } else {
            dictionary = Dictionary::load_tsv(&path).expect("load real dict");
            loaded = true;
        }
    }
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let segments = segment_sentence(&dictionary, &fuzzy, &tones, "lancongwa");
    // With real dict, we expect at least one segment (南昌 should be found)
    assert!(
        !segments.is_empty(),
        "segment_sentence returned empty for real dict"
    );
    eprintln!("Real dict segments: {}", segments.len());
    for (i, seg) in segments.iter().enumerate() {
        let texts: Vec<&str> = seg.iter().map(|c| c.text.as_str()).collect();
        eprintln!("  {}: {:?}", i, texts);
    }
}

#[test]
fn shortest_first_segmentation_prefers_single_syllable() {
    let body = format!(
        "{DICT_HEADER}\n\
         蓝\t\tlan2\t\t官\t\t\t50\n\
         葱\t\tcong1\t\t赣\t\t\t50\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         话\t\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-short-first.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let segments = segment_sentence(&dictionary, &fuzzy, &tones, "lancongwa");
    // With single-char 蓝 and 葱 available, the new shortest-first
    // strategy should prefer splitting into individual syllables
    // rather than the greedy 2-syllable 南昌 match.
    assert_eq!(
        segments.len(),
        3,
        "expected 3 segments (single-syllable splits), got {}: {:?}",
        segments.len(),
        segments
            .iter()
            .map(|s| s.iter().map(|c| c.text.as_str()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    assert!(segments[0].iter().any(|c| c.text == "蓝"));
    assert!(segments[1].iter().any(|c| c.text == "葱"));
    assert!(segments[2].iter().any(|c| c.text == "话"));
}

#[test]
fn sentence_input_retrieves_per_prefix_with_correct_consumed() {
    let body = format!(
        "{DICT_HEADER}\n\
         蓝\t\tlan2\t\t赣\t\t\t50\n\
         葱\t\tcong1\t\t赣\t\t\t50\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         话\t\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-sentence.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let candidates = retrieve_sentence_input(&dictionary, &fuzzy, &tones, "lancongwa", None);
    let texts: Vec<&str> = candidates.iter().map(|c| c.text.as_str()).collect();
    eprintln!("Candidates: {:?}", texts);
    for c in &candidates {
        eprintln!("  {} consumed_bytes={}", c.text, c.consumed_bytes);
    }

    // 南昌 from 2-syllable prefix "lancong", should consume 7 bytes
    let nanchang = candidates.iter().find(|c| c.text == "南昌");
    assert!(nanchang.is_some(), "expected 南昌 in results");
    assert_eq!(
        nanchang.unwrap().consumed_bytes,
        7,
        "南昌 should consume 7 bytes (lancong)"
    );

    // 蓝 from 1-syllable prefix "lan", should consume 3 bytes
    let lan = candidates.iter().find(|c| c.text == "蓝");
    assert!(lan.is_some(), "expected 蓝 in results");
    assert_eq!(
        lan.unwrap().consumed_bytes,
        3,
        "蓝 should consume 3 bytes (lan)"
    );

    // 话 should NOT appear since it's from "wa" which is a separate segment prefix
    let hua = candidates.iter().find(|c| c.text == "话");
    assert!(
        hua.is_none(),
        "话 should not appear since it's in a different segment"
    );
}

#[test]
fn preedit_display_matches_fcitx5_for_auto_manual_and_partial_segments() {
    let body = format!(
        "{DICT_HEADER}\n\
         蓝\t\tlan2\t\t赣\t\t\t50\n\
         葱\t\tcong1\t\t赣\t\t\t50\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         话\t\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-preedit-display.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    assert_eq!(
        format_preedit_display(&dictionary, &fuzzy, &tones, "lancongwa", 0),
        "lan cong wa"
    );
    let manual_apostrophe = format!("lan{}congwa", char::from(39));
    assert_eq!(
        format_preedit_display(&dictionary, &fuzzy, &tones, &manual_apostrophe, 0),
        format!("lan{}cong wa", char::from(39))
    );
    assert_eq!(
        format_preedit_display(&dictionary, &fuzzy, &tones, "lan congwa", 0),
        "lan cong wa"
    );
    assert_eq!(
        format_preedit_display(&dictionary, &fuzzy, &tones, "lancongwa", 7),
        "lancong wa"
    );
}

#[test]
fn sentence_input_first_candidate_uses_full_suffix_best_match() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         嗰\t\tgo3\tge3\t赣\t\t\t300\n\
         佳偶\t\tga1 ngieu3\tjia1 ou3\t赣\t\t\t500\n"
    );
    let path = write_fixture("dict-full-suffix-best.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let candidates = retrieve_sentence_input(&dictionary, &fuzzy, &tones, "lanconggo", None);
    assert!(!candidates.is_empty());
    assert_eq!(candidates[0].text, "南昌嗰");
    assert_ne!(candidates[0].text, "南昌佳偶");
}

#[test]
fn segment_boundaries_prefer_better_gan_path_over_mandarin_fallback() {
    let body = format!(
        "{DICT_HEADER}\n\
         深\t\tsen\tshen1\t赣\t\t\t500\n\
         色\t\tse\tse4\t赣\t\t\t200\n\
         嗯\t\tn\ten1\t赣\t\t\t200\n\
         嗰\t\tgo\tge4\t赣\t\t\t500\n\
         僧\t\t\tseng1\t官\t\t\t100\n\
         哦\t\t\to1\t官\t\t\t100\n"
    );
    let path = write_fixture("dict-seg-sengo.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = repo_fuzzy();
    let tones = tone_values();

    assert_eq!(
        segment_boundaries(&dictionary, &fuzzy, &tones, "sengo"),
        vec![3]
    );
}

#[test]
fn retrieve_prefers_sen_go_over_se_n_go() {
    let body = format!(
        "{DICT_HEADER}\n\
         深嗰\t\tsen go\tshen1 ge4\t赣\t\t\t500\n\
         色嗯嗰\t\tse n go\tse4 en1 ge4\t赣\t\t\t200\n\
         深\t\tsen\tshen1\t赣\t\t\t500\n\
         色\t\tse\tse4\t赣\t\t\t200\n\
         嗯\t\tn\ten1\t赣\t\t\t200\n\
         嗰\t\tgo\tge4\t赣\t\t\t500\n"
    );
    let path = write_fixture("dict-seg-se-n-go.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = repo_fuzzy();
    let tones = tone_values();

    let result = retrieve(&dictionary, &fuzzy, &tones, "sengo");
    assert_eq!(
        result.first().map(|candidate| candidate.text.as_str()),
        Some("深嗰")
    );
}

#[test]
fn segment_boundaries_avoid_mandarin_tail_when_gan_split_exists() {
    let body = format!(
        "{DICT_HEADER}\n\
         你\t\tni\tni3\t赣\t\t\t500\n\
         头\t\tteu\ttou2\t赣\t\t\t500\n\
         日\t\tnit\tri4\t赣\t\t\t100\n\
         诶\t\t\teu1\t官\t\t\t100\n"
    );
    let path = write_fixture("dict-seg-niteu.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = repo_fuzzy();
    let tones = tone_values();

    assert_eq!(
        segment_boundaries(&dictionary, &fuzzy, &tones, "niteu"),
        vec![2]
    );
}

#[test]
fn segment_boundaries_avoid_deprecated_ieu_tail() {
    let body = format!(
        "{DICT_HEADER}\n\
         连\t\tlen\tlian2\t赣\t\t\t500\n\
         脚\t\tgieu\tjiao3\t赣\t\t\t500\n\
         楞\t\t\tleng2\t官\t\t\t100\n\
         夭\t\tieu\tyao1\t赣\t\t\t100\n"
    );
    let path = write_fixture("dict-seg-lengieu.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = repo_fuzzy();
    let tones = tone_values();

    assert_eq!(
        segment_boundaries(&dictionary, &fuzzy, &tones, "lengieu"),
        vec![3]
    );
}

#[test]
fn sentence_input_middle_suffix_uses_normal_short_input_match() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         嗰\t\tgo3\tge3\t赣\t\t\t300\n\
         两岸\t\tliong3 ngon5\tliang3 an4\t赣\t\t\t200\n\
         佳偶\t\tga1 ngieu3\tjia1 ou3\t赣\t\t\t500\n"
    );
    let path = write_fixture("dict-middle-suffix-best.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let candidates =
        retrieve_sentence_input(&dictionary, &fuzzy, &tones, "lanconggoliongngon", None);
    assert!(!candidates.is_empty());
    assert_eq!(candidates[0].text, "南昌嗰两岸");
}

#[test]
fn user_dict_entry_found_after_extend() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         话\t\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-extend.tsv", &body);
    let mut dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let new_entry = gannyu_input_core::DictionaryEntry {
        headword: "南昌话".to_string(),
        ipa: String::new(),
        dialect_pinyin: "lan4 cong1 wa5".to_string(),
        mandarin_pinyin: "nan2 chang1 hua4".to_string(),
        category: "赣".to_string(),
        mandarin_word: String::new(),
        mandarin_word_pinyin: String::new(),
        frequency: Some(20000),
        synonyms: String::new(),
        entry_index: 0,
        new_old: String::new(),
    };
    dictionary.extend_from_entries(std::iter::once(new_entry));

    let candidates = retrieve(&dictionary, &fuzzy, &tones, "lancongwa");
    let found = candidates.iter().any(|c| c.text == "南昌话");
    assert!(
        found,
        "南昌话 should be found after extending dictionary, got: {:?}",
        candidates.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn user_dict_entry_has_user_tag() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         话\t\twa5\thua4\t赣\t\t\t200\n"
    );
    let path = write_fixture("dict-tag.tsv", &body);
    let mut dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let new_entry = gannyu_input_core::DictionaryEntry {
        headword: "南昌话".to_string(),
        ipa: String::new(),
        dialect_pinyin: "lan4 cong1 wa5".to_string(),
        mandarin_pinyin: "nan2 chang1 hua4".to_string(),
        category: "自".to_string(),
        mandarin_word: String::new(),
        mandarin_word_pinyin: String::new(),
        frequency: Some(20000),
        synonyms: String::new(),
        entry_index: 0,
        new_old: String::new(),
    };
    dictionary.extend_from_entries(std::iter::once(new_entry));

    let candidates = retrieve(&dictionary, &fuzzy, &tones, "lancongwa");
    let nanchanghua = candidates
        .iter()
        .find(|c| c.text == "南昌话")
        .expect("should find 南昌话");
    let ann = nanchanghua.annotation.as_deref().unwrap_or("");
    assert!(
        ann.contains("[用户]"),
        "annotation should contain [用户], got: {}",
        ann
    );
}

#[test]
fn user_dict_prune_removes_main_dict_duplicates() {
    let body = format!(
        "{DICT_HEADER}\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n"
    );
    let path = write_fixture("dict-prune.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");

    let mut user_entries: HashMap<String, gannyu_input_core::DictionaryEntry> = HashMap::new();
    user_entries.insert(
        "南昌".to_string(),
        gannyu_input_core::DictionaryEntry {
            headword: "南昌".to_string(),
            ipa: String::new(),
            dialect_pinyin: "lan4 cong1".to_string(),
            mandarin_pinyin: "nan2 chang1".to_string(),
            category: "自".to_string(),
            mandarin_word: String::new(),
            mandarin_word_pinyin: String::new(),
            frequency: Some(20000),
            synonyms: String::new(),
            entry_index: 0,
            new_old: String::new(),
        },
    );
    user_entries.insert(
        "南昌话".to_string(),
        gannyu_input_core::DictionaryEntry {
            headword: "南昌话".to_string(),
            ipa: String::new(),
            dialect_pinyin: "lan4 cong1 wa5".to_string(),
            mandarin_pinyin: "nan2 chang1 hua4".to_string(),
            category: "自".to_string(),
            mandarin_word: String::new(),
            mandarin_word_pinyin: String::new(),
            frequency: Some(20000),
            synonyms: String::new(),
            entry_index: 0,
            new_old: String::new(),
        },
    );

    // Simulate prune: remove entries already in main dict
    user_entries.retain(|hw, _| dictionary.by_headword(hw).is_empty());

    // 南昌 should be removed (in main dict), 南昌话 should remain
    assert!(!user_entries.contains_key("南昌"), "南昌 should be pruned");
    assert!(user_entries.contains_key("南昌话"), "南昌话 should remain");
}

#[test]
fn manual_segment_returns_candidates_at_all_prefix_lengths() {
    use gannyu_input_core::retrieve_with_manual_segments;

    // Build a dictionary that has a 2-syllable entry (go li / 个里) and a
    // 1-syllable entry (go / 个).  When the user types "go'li" the function
    // should return candidates for both the 2-syllable prefix (consumed=5,
    // i.e. all of "go'li") and the 1-syllable prefix (consumed=2, i.e. "go").
    let body = format!(
        "{DICT_HEADER}\n\
         个\tgo5\tgo\tge4\t赣\t这\tzhe4\t2000\n\
         个里\tgo5 li3\tgo li\tge4 li3\t赣\t这里\tzhe4 li3\t800\n"
    );
    let path = write_fixture("dict_manual.tsv", &body);
    let dict = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = FuzzyMap {
        entries: Vec::new(),
    };
    let tone_values = HashMap::new();

    let candidates = retrieve_with_manual_segments(&dict, &fuzzy, &tone_values, "go'li", None);
    assert!(
        !candidates.is_empty(),
        "should return at least one candidate for go'li"
    );

    // There must be at least one candidate consuming all of "go'li" (5 bytes)
    // for the 2-syllable match 个里, and at least one consuming only "go" (2 bytes).
    let has_full = candidates.iter().any(|c| c.consumed_bytes == 5);
    let has_partial = candidates.iter().any(|c| c.consumed_bytes == 2);
    assert!(
        has_full,
        "expected a 2-syllable candidate consuming 5 bytes; got: {:?}",
        candidates
            .iter()
            .map(|c| (&c.text, c.consumed_bytes))
            .collect::<Vec<_>>()
    );
    assert!(
        has_partial,
        "expected a 1-syllable candidate consuming 2 bytes; got: {:?}",
        candidates
            .iter()
            .map(|c| (&c.text, c.consumed_bytes))
            .collect::<Vec<_>>()
    );
}

#[test]
fn manual_segment_with_space_separator() {
    use gannyu_input_core::retrieve_with_manual_segments;

    let body = format!(
        "{DICT_HEADER}\n\
         个\tgo5\tgo\tge4\t赣\t这\tzhe4\t2000\n\
         个里\tgo5 li3\tgo li\tge4 li3\t赣\t这里\tzhe4 li3\t800\n"
    );
    let path = write_fixture("dict_space.tsv", &body);
    let dict = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = FuzzyMap {
        entries: Vec::new(),
    };
    let tone_values = HashMap::new();

    // Space-separated input "go li" should behave identically to "go'li".
    let candidates = retrieve_with_manual_segments(&dict, &fuzzy, &tone_values, "go li", None);
    assert!(
        !candidates.is_empty(),
        "should return candidates for space-separated go li"
    );
    let has_partial = candidates.iter().any(|c| c.consumed_bytes == 2);
    assert!(
        has_partial,
        "expected a 1-syllable candidate with consumed_bytes=2"
    );
}

#[test]
fn manual_segment_keeps_best_first_candidate_inside_apostrophe_chunks() {
    use gannyu_input_core::retrieve_with_manual_segments;

    let body = format!(
        "{DICT_HEADER}\n\
         蓝\t\tlan2\t\t赣\t\t\t50\n\
         葱\t\tcong1\t\t赣\t\t\t50\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         嗰\t\tgo3\tge3\t赣\t\t\t300\n\
         佳偶\t\tga1 ngieu3\tjia1 ou3\t赣\t\t\t500\n"
    );
    let path = write_fixture("dict-manual-apostrophe-chunks.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let candidates = retrieve_with_manual_segments(&dictionary, &fuzzy, &tones, "lancong'go", None);
    assert!(
        !candidates.is_empty(),
        "should return candidates for lancong'go"
    );
    assert_eq!(
        candidates.first().map(|candidate| candidate.text.as_str()),
        Some("南昌嗰")
    );

    let nanchang = candidates.iter().find(|candidate| candidate.text == "南昌");
    assert!(nanchang.is_some(), "expected 南昌 in results");
    assert_eq!(nanchang.unwrap().consumed_bytes, 7);

    let first = candidates.first().unwrap();
    assert_eq!(first.consumed_bytes, 10);
}

#[test]
fn manual_segment_space_path_matches_apostrophe_chunk_behavior() {
    use gannyu_input_core::retrieve_with_manual_segments;

    let body = format!(
        "{DICT_HEADER}\n\
         蓝\t\tlan2\t\t赣\t\t\t50\n\
         葱\t\tcong1\t\t赣\t\t\t50\n\
         南昌\t\tlan4 cong1\tnan2 chang1\t赣\t\t\t100\n\
         嗰\t\tgo3\tge3\t赣\t\t\t300\n\
         佳偶\t\tga1 ngieu3\tjia1 ou3\t赣\t\t\t500\n"
    );
    let path = write_fixture("dict-manual-space-chunks.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = sample_fuzzy();
    let tones = tone_values();

    let candidates = retrieve_with_manual_segments(&dictionary, &fuzzy, &tones, "lancong go", None);
    assert!(
        !candidates.is_empty(),
        "should return candidates for lancong go"
    );
    assert_eq!(
        candidates.first().map(|candidate| candidate.text.as_str()),
        Some("南昌嗰")
    );
}

#[test]
fn manual_segment_partial_combination_keeps_consumed_bytes_at_last_valid_chunk() {
    use gannyu_input_core::retrieve_with_manual_segments;

    let body = format!(
        "{DICT_HEADER}\n\
         个\tgo5\tgo\tge4\t赣\t这\tzhe4\t2000\n"
    );
    let path = write_fixture("dict-manual-partial-consumed.tsv", &body);
    let dictionary = Dictionary::load_tsv(&path).expect("load");
    let fuzzy = FuzzyMap {
        entries: Vec::new(),
    };
    let tone_values = HashMap::new();

    let candidates =
        retrieve_with_manual_segments(&dictionary, &fuzzy, &tone_values, "go'zzz", None);
    let first = candidates
        .first()
        .expect("should keep the valid first chunk candidate");
    assert_eq!(first.text, "个");
    assert_eq!(first.consumed_bytes, 2);
}

#[test]
fn entering_coda_k_fuzzes_to_t_with_manual_segments() {
    let body = format!("{DICT_HEADER}\n日头\t\tnit7 teu\tri4 tou5\t赣\t太阳\ttai4 yang2\t300000\n");
    let dict_path = write_fixture("dict_kcoda.tsv", &body);
    let dictionary = Dictionary::load_tsv(&dict_path).expect("load");
    let fuzzy = repo_fuzzy();
    let tone_values = HashMap::new();

    let canonical =
        retrieve_with_manual_segments(&dictionary, &fuzzy, &tone_values, "nit'teu", None);
    assert!(canonical.iter().any(|candidate| candidate.text == "日头"));

    let k_coda = retrieve_with_manual_segments(&dictionary, &fuzzy, &tone_values, "nik'teu", None);
    assert!(k_coda.iter().any(|candidate| candidate.text == "日头"));

    let h_coda = retrieve_with_manual_segments(&dictionary, &fuzzy, &tone_values, "nih'teu", None);
    assert!(h_coda.iter().all(|candidate| candidate.text != "日头"));
}

#[test]
fn sentence_input_maps_doubled_variant_consumed_bytes_back_to_original() {
    let body = format!(
        "{DICT_HEADER}\n\
         日头\t\tnit7 teu\tri4 tou5\t赣\t太阳\ttai4 yang2\t300000\n"
    );
    let dict_path = write_fixture("dict_sentence_doubled.tsv", &body);
    let dictionary = Dictionary::load_tsv(&dict_path).expect("load");
    let fuzzy = repo_fuzzy();
    let tone_values = HashMap::new();

    let result = retrieve_sentence_input(&dictionary, &fuzzy, &tone_values, "niteum", None);
    let hit = result
        .iter()
        .find(|candidate| candidate.text == "日头")
        .expect("日头 candidate");
    assert_eq!(hit.consumed_bytes, 5);
}

#[test]
fn retrieval_candidate_order_and_json_contract_stay_stable() {
    let result = retrieve(&sample_dictionary(), &sample_fuzzy(), &tone_values(), "qu");
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        r#"[{"text":"渠","annotation":"qu5 [义]他","ipa":"tɕʰy21","layer":"gannyu-exact","mandarin_only":false,"weight":5.0225,"reading":"qu","mandarin_reading":"qu2"},{"text":"他","annotation":"[不习用] [习用]渠（qu5）/佢（qu5）","ipa":null,"layer":"gannyu-exact","mandarin_only":false,"weight":5.0},{"text":"去","annotation":"qu1","ipa":"tɕʰy42","layer":"gannyu-exact","mandarin_only":false,"weight":5.0135,"reading":"qu","mandarin_reading":"qu4"},{"text":"佢","annotation":"qu5 [义]他","ipa":"tɕʰy21","layer":"gannyu-exact","mandarin_only":false,"weight":5.00045,"reading":"qu","mandarin_reading":"qu2"}]"#
    );
}
