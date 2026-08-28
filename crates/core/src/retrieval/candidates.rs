const FREQUENCY_CAP: f64 = 200_000.0;

fn frequency_factor(frequency: Option<u64>) -> f64 {
    match frequency {
        Some(value) if value > 0 => {
            let normalized = (value as f64).min(FREQUENCY_CAP) / FREQUENCY_CAP;
            0.9 * normalized
        }
        _ => 0.0,
    }
}

fn trailing_tone_value(syllable: &str) -> Option<String> {
    let digits: String = syllable
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn toned_gan_pinyin(
    dialect_pinyin: &str,
    ipa: &str,
    tone_values: &HashMap<String, u8>,
) -> Option<String> {
    let dialect = dialect_pinyin.trim();
    if dialect.is_empty() {
        return None;
    }
    let dialect_syllables: Vec<&str> = dialect.split_whitespace().collect();
    let ipa_syllables: Vec<&str> = ipa.split_whitespace().collect();
    if dialect_syllables.is_empty() {
        return None;
    }
    if dialect_syllables.len() != ipa_syllables.len() || tone_values.is_empty() {
        return Some(dialect.to_string());
    }
    let mut parts = Vec::with_capacity(dialect_syllables.len());
    for (dialect_syllable, ipa_syllable) in dialect_syllables.iter().zip(ipa_syllables.iter()) {
        if trailing_tone_value(dialect_syllable).is_some() {
            parts.push((*dialect_syllable).to_string());
            continue;
        }
        let tone =
            trailing_tone_value(ipa_syllable).and_then(|value| tone_values.get(&value).copied());
        match tone {
            Some(class) => parts.push(format!("{dialect_syllable}{class}")),
            None => parts.push((*dialect_syllable).to_string()),
        }
    }
    Some(parts.join(" "))
}

fn label_priority(display: &str) -> u8 {
    if display.starts_with("[文]") {
        0
    } else if display.starts_with("[白]") {
        1
    } else {
        2
    }
}

fn reading_without_prefix_labels(display: &str) -> &str {
    let mut rest = display;
    while let Some(stripped) = rest.strip_prefix('[') {
        let Some(end) = stripped.find(']') else {
            break;
        };
        rest = &stripped[end + 1..];
    }
    rest
}

fn is_neutral_display(display: &str) -> bool {
    let reading = reading_without_prefix_labels(display);
    match trailing_tone_value(reading) {
        Some(value) => value == "0",
        None => true,
    }
}

fn suppress_redundant_single_char_neutral_displays(
    entries: &[(String, Option<u64>)],
) -> Vec<(String, Option<u64>)> {
    if entries.len() <= 1 {
        return entries.to_vec();
    }
    let bases_with_tone: HashSet<String> = entries
        .iter()
        .filter_map(|(display, _)| {
            let reading = reading_without_prefix_labels(display);
            let tone = trailing_tone_value(reading)?;
            if tone == "0" {
                None
            } else {
                Some(strip_tone(reading).to_string())
            }
        })
        .collect();
    entries
        .iter()
        .filter(|(display, _)| {
            let reading = reading_without_prefix_labels(display);
            !is_neutral_display(display) || !bases_with_tone.contains(strip_tone(reading))
        })
        .cloned()
        .collect()
}

fn sanitize_quotes(s: &str) -> String {
    s.replace('「', "“")
        .replace('」', "”")
        .replace('『', "“")
        .replace('』', "”")
}

fn mandarin_word_suffix(headword: &str, related_entries: &[&DictionaryEntry]) -> Option<String> {
    let mut words: Vec<String> = Vec::new();
    for entry in related_entries {
        for mw in distinct_mandarin_words(headword, &entry.mandarin_word) {
            if !words.contains(&mw) {
                words.push(mw);
            }
        }
    }
    if words.is_empty() {
        None
    } else {
        Some(format!("[官]{}", words.join("/")))
    }
}

fn associated_word_suffix(dictionary: &Dictionary, headword: &str) -> Option<String> {
    // 关联缓存随词典单例共享(Dictionary::associations), 不再按线程重建。
    let words = dictionary.associations().associates_of(headword);
    if words.is_empty() {
        None
    } else {
        Some(format!("[联]{}", words.join("/")))
    }
}

fn append_annotation_suffix(annotation: Option<String>, suffix: Option<String>) -> Option<String> {
    match (annotation, suffix) {
        (Some(base), Some(extra)) if !base.is_empty() => Some(format!("{base}, {extra}")),
        (Some(base), _) => Some(base),
        (None, Some(extra)) => Some(extra),
        (None, None) => None,
    }
}

fn append_associated_suffix(
    dictionary: &Dictionary,
    headword: &str,
    annotation: Option<String>,
) -> Option<String> {
    append_annotation_suffix(annotation, associated_word_suffix(dictionary, headword))
}

fn gan_annotation_for_mandarin_entry(
    dictionary: &Dictionary,
    mandarin_headword: &str,
    tone_values: &HashMap<String, u8>,
) -> Option<String> {
    ANNOTATION_CACHE.with(|cache| {
        if let Some(cached) = cache.borrow().get(mandarin_headword) {
            return cached.clone();
        }
        let result = gan_annotation_impl(dictionary, mandarin_headword, tone_values);
        cache
            .borrow_mut()
            .insert(mandarin_headword.to_string(), result.clone());
        result
    })
}

fn gan_annotation_impl(
    dictionary: &Dictionary,
    mandarin_headword: &str,
    tone_values: &HashMap<String, u8>,
) -> Option<String> {
    let source_entries: Vec<&DictionaryEntry> = dictionary
        .by_mandarin_word_text(mandarin_headword)
        .into_iter()
        .filter(|e| !e.is_mandarin_only())
        .collect();
    if source_entries.is_empty() {
        return None;
    }
    let mut seen_headwords: Vec<&str> = Vec::new();
    for entry in &source_entries {
        if !seen_headwords.contains(&entry.headword.as_str()) {
            seen_headwords.push(entry.headword.as_str());
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for headword in seen_headwords {
        let all_for_headword = dictionary.by_headword(headword);
        if let Some(reading) =
            aggregate_annotation(
                &all_for_headword,
                tone_values,
                &dictionary.new_old_map,
                &dictionary.heteronym_chars,
                &dictionary.paired_readings,
            )
        {
            parts.push(format!("{}（{}）", headword, reading));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("[赣]{}", parts.join("/")))
    }
}

/// Build a `[官话词]` annotation: the Mandarin-word tag followed by the entry's
/// own Gan reading (if any) and then the native Gan equivalent(s) (if any).
fn guan_hua_ci_annotation(
    own_reading: Option<String>,
    gan_reverse: Option<String>,
) -> Option<String> {
    let parts: Vec<&str> = [
        Some("[官话词]"),
        own_reading.as_deref(),
        gan_reverse.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    Some(parts.join(" "))
}

fn annotation_for_entry(
    dictionary: &Dictionary,
    entry: &DictionaryEntry,
    tone_values: &HashMap<String, u8>,
) -> Option<String> {
    let mandarin_only = entry.is_mandarin_only();
    let related_entries = dictionary.by_headword(&entry.headword);
    let annotation =
        if mandarin_only || !dictionary.by_mandarin_word_text(&entry.headword).is_empty() {
            let own_reading = if mandarin_only {
                None
            } else {
                aggregate_annotation(
                    &related_entries,
                    tone_values,
                    &dictionary.new_old_map,
                    &dictionary.heteronym_chars,
                    &dictionary.paired_readings,
                )
            };
            let gan_reverse =
                gan_annotation_for_mandarin_entry(dictionary, &entry.headword, tone_values);
            guan_hua_ci_annotation(own_reading, gan_reverse)
        } else {
            let gan_reading =
                aggregate_annotation(
                    &related_entries,
                    tone_values,
                    &dictionary.new_old_map,
                    &dictionary.heteronym_chars,
                    &dictionary.paired_readings,
                );
            let suffix = mandarin_word_suffix(&entry.headword, &related_entries);
            let parts: Vec<&str> = [gan_reading.as_deref(), suffix.as_deref()]
                .into_iter()
                .flatten()
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(" "))
            }
        };
    let annotation = if entry.category == "自" {
        match annotation {
            Some(a) => Some(format!("{a} [用户]")),
            None => Some("[用户]".to_string()),
        }
    } else {
        annotation
    };
    append_associated_suffix(dictionary, &entry.headword, annotation)
}

/// Generate a `[官话词]` candidate for the Mandarin-equivalent of a Gan entry.
///
/// Returns `None` if the entry has no `mandarin_word` distinct from its own headword.
fn mandarin_word_candidates_for_gan_entry(
    dictionary: &Dictionary,
    gan_entry: &DictionaryEntry,
    tone_values: &HashMap<String, u8>,
) -> Vec<RankedCandidate> {
    let mut candidates = Vec::new();
    for mw in distinct_mandarin_words(&gan_entry.headword, &gan_entry.mandarin_word) {
        let mw_related = dictionary.by_headword(&mw);
        let own_reading = aggregate_annotation(
            &mw_related,
            tone_values,
            &dictionary.new_old_map,
            &dictionary.heteronym_chars,
            &dictionary.paired_readings,
        );
        let gan_reverse = gan_annotation_for_mandarin_entry(dictionary, &mw, tone_values);
        let annotation = append_associated_suffix(
            dictionary,
            &mw,
            guan_hua_ci_annotation(own_reading, gan_reverse),
        );
        let ipa = aggregate_ipa(&mw_related);
        candidates.push(RankedCandidate {
            text: sanitize_quotes(&mw),
            annotation,
            ipa,
            layer: RetrievalLayer::GannyuExact,
            mandarin_only: false,
            weight: RetrievalLayer::GannyuExact.base_weight()
                + frequency_factor(mw_related.first().and_then(|e| e.frequency)),
            reading: None,
            mandarin_reading: None,
            consumed_bytes: 0,
        });
    }
    candidates
}

fn annotated_gan_pinyin(
    entry: &DictionaryEntry,
    tone_values: &HashMap<String, u8>,
    new_old_map: &HashMap<char, (String, String)>,
    heteronym_chars: &HashSet<char>,
    paired_readings: &HashMap<char, Vec<PairedReading>>,
) -> Option<String> {
    let label = entry.register_label();
    let reading = toned_gan_pinyin(&entry.dialect_pinyin, &entry.ipa, tone_values)?;
    let annotated = if let Some(new_old_label) = new_old_label_for_entry(entry, new_old_map, heteronym_chars) {
        format!("{new_old_label}{reading}")
    } else if !label.is_empty() {
        format!("{label}{reading}")
    } else {
        reading.clone()
    };
    // For multi-char words, apply per-syllable new/old pairs
    if entry.headword.chars().count() > 1 {
        let result = apply_new_old_to_multichar(
            entry,
            &annotated,
            tone_values,
            paired_readings,
        );
        return result;
    }
    if let Some(ch) = entry.headword.chars().next() {
        let stored = entry.dialect_pinyin.trim();
        if let Some(pair) = matching_paired_reading(ch, stored, paired_readings) {
            let first = tone_syllable(&pair.first, entry, tone_values);
            let second = tone_syllable(&pair.second, entry, tone_values);
            return Some(format_paired_display(pair, &first, &second, false));
        }
    }
    if annotated.is_empty() {
        None
    } else {
        Some(annotated)
    }
}

fn new_old_label_for_entry(
    entry: &DictionaryEntry,
    new_old_map: &HashMap<char, (String, String)>,
    heteronym_chars: &HashSet<char>,
) -> Option<String> {
    if entry.headword.chars().count() != 1 {
        return None;
    }
    let ch = entry.headword.chars().next()?;
    if !new_old_map.contains_key(&ch) {
        return None;
    }
    let label = if entry.new_old.starts_with("新") {
        "[新]"
    } else if entry.new_old.starts_with("老") {
        "[老]"
    } else if heteronym_chars.contains(&ch) && entry.new_old.starts_with("又") {
        "[又]"
    } else {
        return None;
    };
    Some(label.to_string())
}

fn display_pair_priority(kind: PairKind) -> u8 {
    match kind {
        PairKind::NewOld => 0,
        PairKind::Heteronym => 1,
        PairKind::WenBai => 2,
    }
}

fn matching_paired_reading<'a>(
    ch: char,
    stored_syllable: &str,
    paired_readings: &'a HashMap<char, Vec<PairedReading>>,
) -> Option<&'a PairedReading> {
    let pairs = paired_readings.get(&ch)?;
    let neutral = trailing_tone_value(stored_syllable).as_deref() == Some("0");
    let stripped = strip_tone(stored_syllable);
    pairs
        .iter()
        .filter(|pair| pair.kind != PairKind::WenBai)
        .filter(|pair| {
            stored_syllable == pair.first
                || stored_syllable == pair.second
                || (neutral
                    && (strip_tone(&pair.first) == stripped || strip_tone(&pair.second) == stripped))
        })
        .min_by_key(|pair| display_pair_priority(pair.kind))
}

fn format_paired_display(pair: &PairedReading, first: &str, second: &str, wrap: bool) -> String {
    let display = match pair.kind {
        PairKind::Heteronym => format!("{first}/[又]{second}"),
        PairKind::NewOld => format!("[新]{first}/[老]{second}"),
        PairKind::WenBai => unreachable!("wenbai should not render as a paired subtitle"),
    };
    if wrap {
        format!("({display})")
    } else {
        display
    }
}

fn apply_new_old_to_multichar(
    entry: &DictionaryEntry,
    reading: &str,
    tone_values: &HashMap<String, u8>,
    paired_readings: &HashMap<char, Vec<PairedReading>>,
) -> Option<String> {
    let mut parts: Vec<String> = reading.split_whitespace().map(|s| s.to_string()).collect();
    let chars: Vec<char> = entry.headword.chars().collect();
    let stored: Vec<&str> = entry.dialect_pinyin.split_whitespace().collect();
    if chars.len() != parts.len() || chars.len() != stored.len() {
        return Some(reading.to_string());
    }
    for (pos, &ch) in chars.iter().enumerate() {
        if let Some(pair) = matching_paired_reading(ch, stored[pos], paired_readings) {
            if trailing_tone_value(stored[pos]).as_deref() == Some("0") {
                let first = format!("{}0", strip_tone(&pair.first));
                let second = format!("{}0", strip_tone(&pair.second));
                parts[pos] = format_paired_display(pair, &first, &second, true);
            } else {
                let first = tone_syllable(&pair.first, entry, tone_values);
                let second = tone_syllable(&pair.second, entry, tone_values);
                parts[pos] = format_paired_display(pair, &first, &second, true);
            }
        }
    }
    let result = parts.join(" ");
    if result == reading {
        Some(reading.to_string())
    } else {
        Some(result)
    }
}

fn tone_syllable(
    syllable: &str,
    entry: &DictionaryEntry,
    tone_values: &HashMap<String, u8>,
) -> String {
    if syllable.is_empty() {
        return syllable.to_string();
    }
    if trailing_tone_value(syllable).is_some() {
        return syllable.to_string();
    }
    let ipa_syllables: Vec<&str> = entry.ipa.split_whitespace().collect();
    // Find matching position by matching untuned bases
    let dialect_syllables: Vec<&str> = entry.dialect_pinyin.split_whitespace().collect();
    for (i, ds) in dialect_syllables.iter().enumerate() {
        if *ds == syllable {
            if let Some(ipa_syl) = ipa_syllables.get(i) {
                if let Some(tone) =
                    trailing_tone_value(ipa_syl).and_then(|v| tone_values.get(&v).copied())
                {
                    return format!("{}{}", syllable, tone);
                }
            }
            break;
        }
    }
    // Fallback: look up any IPA syllable at same position
    if let Some(pos) = dialect_syllables
        .iter()
        .position(|s| strip_tone(s) == strip_tone(syllable))
    {
        if let Some(ipa_syl) = ipa_syllables.get(pos) {
            if let Some(tone) =
                trailing_tone_value(ipa_syl).and_then(|v| tone_values.get(&v).copied())
            {
                return format!("{}{}", syllable, tone);
            }
        }
    }
    syllable.to_string()
}

fn aggregate_annotation(
    entries: &[&DictionaryEntry],
    tone_values: &HashMap<String, u8>,
    new_old_map: &HashMap<char, (String, String)>,
    heteronym_chars: &HashSet<char>,
    paired_readings: &HashMap<char, Vec<PairedReading>>,
) -> Option<String> {
    let mut displays: Vec<(String, Option<u64>)> = Vec::new();
    for entry in entries {
        if entry.is_mandarin_only() {
            continue;
        }
        let Some(display) = annotated_gan_pinyin(
            entry,
            tone_values,
            new_old_map,
            heteronym_chars,
            paired_readings,
        ) else {
            continue;
        };
        if let Some(existing) = displays.iter_mut().find(|(value, _)| value == &display) {
            existing.1 = existing.1.max(entry.frequency);
            continue;
        }
        displays.push((display, entry.frequency));
    }
    if displays.is_empty() {
        return None;
    }
    if entries
        .iter()
        .any(|entry| !entry.is_mandarin_only() && entry.headword.chars().count() == 1)
    {
        displays = suppress_redundant_single_char_neutral_displays(&displays);
        if displays.is_empty() {
            return None;
        }
    }
    displays.sort_by(|left, right| {
        label_priority(&left.0)
            .cmp(&label_priority(&right.0))
            .then_with(|| right.1.unwrap_or(0).cmp(&left.1.unwrap_or(0)))
            .then_with(|| left.0.cmp(&right.0))
    });
    let joiner = if displays.iter().all(|(display, _)| display.starts_with('[')) {
        " "
    } else {
        " / "
    };
    Some(
        displays
            .into_iter()
            .map(|(display, _)| display)
            .collect::<Vec<String>>()
            .join(joiner),
    )
}

fn aggregate_ipa(entries: &[&DictionaryEntry]) -> Option<String> {
    let mut displays: Vec<(String, Option<u64>)> = Vec::new();
    for entry in entries {
        if entry.ipa.is_empty() {
            continue;
        }
        if let Some(existing) = displays.iter_mut().find(|(value, _)| value == &entry.ipa) {
            existing.1 = existing.1.max(entry.frequency);
            continue;
        }
        displays.push((entry.ipa.clone(), entry.frequency));
    }
    if displays.is_empty() {
        return None;
    }
    displays.sort_by(|left, right| {
        right
            .1
            .unwrap_or(0)
            .cmp(&left.1.unwrap_or(0))
            .then_with(|| left.0.cmp(&right.0))
    });
    Some(
        displays
            .into_iter()
            .map(|(ipa, _)| ipa)
            .collect::<Vec<String>>()
            .join(" / "),
    )
}

pub(crate) fn gan_candidate(
    dictionary: &Dictionary,
    entry: &DictionaryEntry,
    layer: RetrievalLayer,
    tone_values: &HashMap<String, u8>,
) -> RankedCandidate {
    let mandarin_only = entry.is_mandarin_only();
    let related_entries = dictionary.by_headword(&entry.headword);
    let annotation = annotation_for_entry(dictionary, entry, tone_values);
    let ipa = aggregate_ipa(&related_entries);
    RankedCandidate {
        text: sanitize_quotes(&entry.headword),
        annotation,
        ipa,
        layer,
        mandarin_only,
        weight: layer.base_weight() + frequency_factor(entry.frequency),
        reading: Some(entry.dialect_pinyin.clone()),
        mandarin_reading: if entry.mandarin_pinyin.is_empty() {
            None
        } else {
            Some(entry.mandarin_pinyin.clone())
        },
        consumed_bytes: 0,
    }
}

fn sorted_by_frequency(mut entries: Vec<&DictionaryEntry>) -> Vec<&DictionaryEntry> {
    entries.sort_by(|left, right| {
        right
            .frequency
            .unwrap_or(0)
            .cmp(&left.frequency.unwrap_or(0))
            .then_with(|| left.headword.cmp(&right.headword))
    });
    entries
}

fn push_candidate(
    candidates: &mut Vec<RankedCandidate>,
    seen: &mut HashSet<String>,
    candidate: RankedCandidate,
) {
    if seen.contains(&candidate.text) {
        return;
    }
    seen.insert(candidate.text.clone());
    candidates.push(candidate);
}

fn insert_candidate_after(
    candidates: &mut Vec<RankedCandidate>,
    seen: &mut HashSet<String>,
    after_text: &str,
    candidate: RankedCandidate,
) {
    if seen.contains(&candidate.text) {
        let Some(existing_index) = candidates
            .iter()
            .position(|item| item.text == candidate.text)
        else {
            return;
        };
        let Some(after_index) = candidates.iter().position(|item| item.text == after_text) else {
            return;
        };
        if existing_index <= after_index + 1 {
            return;
        }
        let existing = candidates.remove(existing_index);
        let target_after = if existing_index < after_index {
            after_index - 1
        } else {
            after_index
        };
        candidates.insert(target_after + 1, existing);
        return;
    }
    seen.insert(candidate.text.clone());
    if let Some(index) = candidates.iter().position(|item| item.text == after_text) {
        candidates.insert(index + 1, candidate);
    } else {
        candidates.push(candidate);
    }
}

fn push_synonyms(
    candidates: &mut Vec<RankedCandidate>,
    seen: &mut HashSet<String>,
    dictionary: &Dictionary,
    entry: &DictionaryEntry,
    parent_layer: RetrievalLayer,
    tone_values: &HashMap<String, u8>,
) {
    if entry.synonyms.is_empty() {
        return;
    }
    let mut synonym_entries: Vec<&DictionaryEntry> = Vec::new();
    for raw_synonym in entry.synonyms.split('/') {
        let synonym = raw_synonym.trim();
        if synonym.is_empty() || synonym == entry.headword {
            continue;
        }
        synonym_entries.extend(dictionary.by_headword(synonym));
    }
    for syn_entry in sorted_by_frequency(synonym_entries) {
        if seen.contains(&syn_entry.headword) {
            continue;
        }
        let mut candidate =
            gan_candidate(dictionary, syn_entry, RetrievalLayer::Synonym, tone_values);
        candidate.weight =
            parent_layer.base_weight() + frequency_factor(syn_entry.frequency) - 0.01;
        push_candidate(candidates, seen, candidate);
        for mw_cand in mandarin_word_candidates_for_gan_entry(dictionary, syn_entry, tone_values) {
            push_candidate(candidates, seen, mw_cand);
        }
    }
}

/// Inject Gan-Mandarin pairs and association-group members after candidates
/// within the display limit.  For each candidate in the scanned range:
///   - Gan word with Mandarin equivalent → insert Mandarin right after.
///   - Mandarin word with Gan equivalents → replace with Gan→Mandarin pair (Gan first).
///   - Association-group members → insert right after.
///
/// Returns the number of extra candidates inserted, so the caller can extend
/// the per-group display limit.
fn inject_pairs_and_associations(
    candidates: &mut Vec<RankedCandidate>,
    seen: &mut HashSet<String>,
    dictionary: &Dictionary,
    cache: &AssociationCache,
    tone_values: &HashMap<String, u8>,
    start_idx: usize,
    base_limit: usize,
) -> usize {
    let mut extra = 0usize;
    let mut i = start_idx;
    let mut scanned = 0usize;
    while i < candidates.len() && scanned < base_limit {
        let headword = candidates[i].text.clone();

        // ── Gan-Mandarin pair: Gan word hit ──
        let mandarins = cache.mandarins_of_gan(&headword);
        if !mandarins.is_empty() {
            let mut insert_at = i + 1;
            for mw in mandarins {
                if !seen.contains(mw) {
                    if let Some(entry) = dictionary.by_headword(mw).first() {
                        let mut cand = gan_candidate(
                            dictionary,
                            entry,
                            RetrievalLayer::GannyuExact,
                            tone_values,
                        );
                        cand.weight = candidates[i].weight - 0.02;
                        seen.insert(mw.clone());
                        candidates.insert(insert_at, cand);
                        insert_at += 1;
                        extra += 1;
                    }
                }
            }
        }

        // ── Gan-Mandarin pair: Mandarin word hit → Gan first, then Mandarin ──
        if !cache.gan_of_mandarin(&headword).is_empty() {
            let gan_words = cache.gan_of_mandarin(&headword).to_vec();
            if let Some(first_gan) = gan_words.first() {
                if !seen.contains(first_gan) {
                    if let Some(gan_entry) = dictionary.by_headword(first_gan).first() {
                        let mut gan_cand = gan_candidate(
                            dictionary,
                            gan_entry,
                            RetrievalLayer::GannyuExact,
                            tone_values,
                        );
                        gan_cand.weight = candidates[i].weight + 0.01;
                        seen.insert(first_gan.clone());
                        let mut mandarin_cand =
                            if let Some(entry) = dictionary.by_headword(&headword).first() {
                                gan_candidate(
                                    dictionary,
                                    entry,
                                    RetrievalLayer::GannyuExact,
                                    tone_values,
                                )
                            } else {
                                RankedCandidate {
                                    text: headword.clone(),
                                    annotation: None,
                                    ipa: None,
                                    layer: RetrievalLayer::GannyuExact,
                                    mandarin_only: true,
                                    weight: RetrievalLayer::GannyuExact.base_weight(),
                                    reading: None,
                                    mandarin_reading: None,
                                    consumed_bytes: 0,
                                }
                            };
                        mandarin_cand.weight = candidates[i].weight - 0.01;
                        mandarin_cand.consumed_bytes = candidates[i].consumed_bytes;
                        candidates.remove(i);
                        candidates.insert(i, gan_cand);
                        candidates.insert(i + 1, mandarin_cand);
                        extra += 1;
                        i += 1;
                        scanned += 1;
                        continue;
                    }
                }
            }
        }

        // ── Association groups ──
        for assoc in cache.associates_of(&headword) {
            if seen.contains(assoc) {
                continue;
            }
            if let Some(entry) = dictionary.by_headword(assoc).first() {
                let mut cand =
                    gan_candidate(dictionary, entry, RetrievalLayer::Synonym, tone_values);
                cand.weight = candidates[i].weight - 0.03;
                seen.insert(assoc.clone());
                i += 1;
                candidates.insert(i, cand);
                extra += 1;
            }
        }

        i += 1;
        scanned += 1;
    }
    extra
}

fn push_reverse_gan<'a>(
    dictionary: &'a Dictionary,
    source_entries: &[&'a DictionaryEntry],
    candidates: &mut Vec<RankedCandidate>,
    seen: &mut HashSet<String>,
    mandarin_words: &mut HashSet<String>,
    tone_values: &HashMap<String, u8>,
    layer: RetrievalLayer,
) {
    let source_headwords: HashSet<&str> =
        source_entries.iter().map(|e| e.headword.as_str()).collect();
    let mut reverse_gan: Vec<&'a DictionaryEntry> = Vec::new();
    let mut reverse_seen: HashSet<(&str, &str)> =
        HashSet::new();
    for entry in source_entries {
        for gan_entry in dictionary.by_mandarin_word_text(&entry.headword) {
            if source_headwords.contains(gan_entry.headword.as_str()) {
                continue;
            }
            let key = (
                gan_entry.headword.as_str(),
                gan_entry.dialect_pinyin.as_str(),
            );
            if !reverse_seen.insert(key) {
                continue;
            }
            reverse_gan.push(gan_entry);
        }
    }

    for entry in sorted_by_frequency(reverse_gan) {
        if seen.contains(&entry.headword) {
            continue;
        }
        push_candidate(
            candidates,
            seen,
            gan_candidate(dictionary, entry, layer, tone_values),
        );
        push_synonyms(candidates, seen, dictionary, entry, layer, tone_values);

        for mw_text in distinct_mandarin_words(&entry.headword, &entry.mandarin_word) {
            let mw_related = dictionary.by_headword(&mw_text);
            let own_reading =
                aggregate_annotation(
                    &mw_related,
                    tone_values,
                    &dictionary.new_old_map,
                    &dictionary.heteronym_chars,
                    &dictionary.paired_readings,
                );
            let gan_reverse = gan_annotation_for_mandarin_entry(dictionary, &mw_text, tone_values);
            let annotation = append_associated_suffix(
                dictionary,
                &mw_text,
                guan_hua_ci_annotation(own_reading, gan_reverse),
            );
            let mw_candidate = RankedCandidate {
                text: mw_text.clone(),
                annotation,
                ipa: None,
                layer,
                mandarin_only: true,
                weight: layer.base_weight()
                    + frequency_factor(mw_related.first().and_then(|e| e.frequency)),
                reading: None,
                mandarin_reading: None,
                consumed_bytes: 0,
            };
            push_candidate(candidates, seen, mw_candidate);
            mandarin_words.insert(mw_text);
        }
    }
}

/// Collect hits by matching each spaced syllable against the per-position index.
fn collect_per_syllable_hits(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    query_syllables: &[&str],
    direct_entry_ids: &mut HashSet<usize>,
    match_kind: &mut HashMap<usize, u8>,
    exact_dialect_ids: &HashSet<usize>,
    exact_mandarin_ids: &HashSet<usize>,
) {
    let mut per_position: Vec<HashSet<usize>> = Vec::new();

    for (pos, syl) in query_syllables.iter().enumerate() {
        let mut ids: HashSet<usize> = HashSet::new();

        let compact = syl.trim();
        // single-letter tokens are treated as initial-letter (首字母) matches
        let is_initial_letter =
            compact.len() == 1 && compact.chars().all(|c| c.is_ascii_alphabetic());

        if is_initial_letter {
            let first_ch = compact.chars().next().unwrap();
            for &idx in dictionary.initial_match_ids(pos, first_ch) {
                ids.insert(idx as usize);
            }
            // also consider fuzzy variants? keep simple: initials match is enough
        } else {
            for entry in dictionary.by_syllable_at_position(pos, syl) {
                if let Some(id) = dictionary.entry_id(entry) {
                    ids.insert(id);
                }
            }

            let mut fuzzy_forms: Vec<String> = Vec::new();
            for scheme in [SyllableScheme::GonPin, SyllableScheme::GonHan] {
                for normalized_syl in fuzzy.normalize(syl, scheme) {
                    if normalized_syl.text != *syl && !fuzzy_forms.contains(&normalized_syl.text) {
                        fuzzy_forms.push(normalized_syl.text);
                    }
                }
            }
            for form in &fuzzy_forms {
                for entry in dictionary.by_syllable_at_position(pos, form) {
                    if let Some(id) = dictionary.entry_id(entry) {
                        ids.insert(id);
                    }
                }
            }
        }

        per_position.push(ids);
    }

    // Cap per-position sets to top-200 by frequency to avoid
    // quadratic blowup in intersection for common syllables.
    const PER_POSITION_CAP: usize = 200;
    for pos_ids in &mut per_position {
        if pos_ids.len() > PER_POSITION_CAP {
            let mut sorted: Vec<usize> = pos_ids.drain().collect();
            sorted.sort_by_cached_key(|&id| {
                -(dictionary
                    .entries()
                    .get(id)
                    .and_then(|e| e.frequency)
                    .unwrap_or(0) as i64)
            });
            sorted.truncate(PER_POSITION_CAP);
            pos_ids.extend(sorted);
        }
    }

    if per_position.iter().all(|s| !s.is_empty()) {
        let mut intersection: HashSet<usize> = per_position[0].clone();
        for pos_set in &per_position[1..] {
            intersection = intersection.intersection(pos_set).copied().collect();
        }
        for &id in &intersection {
            if !direct_entry_ids.contains(&id) {
                direct_entry_ids.insert(id);
                // Classify at collection time: check if this entry was found
                // via dialect_index or mandarin_index during full-pinyin lookup.
                match_kind.entry(id).or_insert_with(|| {
                    if exact_dialect_ids.contains(&id) {
                        2u8 // Gan
                    } else if exact_mandarin_ids.contains(&id) {
                        1u8 // Mandarin
                    } else {
                        0u8 // fuzzy/mixed
                    }
                });
            }
        }
    }
}
