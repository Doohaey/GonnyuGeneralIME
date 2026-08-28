/// Clears the per-thread retrieve cache.  Must be called after mutating the
/// dictionary (e.g. add_user_word) so stale entries are not returned.
pub(crate) fn clear_retrieve_cache() {
    INNER_RETRIEVE_CACHE.with(|cache| cache.borrow_mut().clear());
}

/// Build the layered candidate list for an input string.
///
/// Multi-character input supports mixed Gan / Mandarin pinyin:
/// each syllable position can match either scheme.
pub fn retrieve(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
) -> Vec<RankedCandidate> {
    retrieve_inner(dictionary, fuzzy, tone_values, input, false)
}

pub(crate) fn retrieve_top_with_boosts(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    max: usize,
    boosts: &HashMap<String, u64>,
) -> Vec<RankedCandidate> {
    let boosts = (!boosts.is_empty()).then_some(boosts);
    retrieve_inner_limited_with_boosts(dictionary, fuzzy, tone_values, input, false, max, boosts)
}

#[derive(Debug, Clone)]
struct ExplicitChunk {
    start: usize,
    text: String,
}

fn split_explicit_chunks(normalized: &str) -> Vec<ExplicitChunk> {
    let mut chunks = Vec::new();
    let bytes = normalized.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor < bytes.len()
            && (bytes[cursor] == b'\'' || bytes[cursor].is_ascii_whitespace())
        {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'\'' && !bytes[cursor].is_ascii_whitespace()
        {
            cursor += 1;
        }
        chunks.push(ExplicitChunk {
            start,
            text: normalized[start..cursor].to_string(),
        });
    }
    chunks
}

/// Retrieve candidates for an input that contains explicit separators (`'` or space).
///
/// Explicit separators act as hard chunk boundaries, but each chunk still
/// contributes its own best internal syllable segmentation.
pub fn retrieve_with_manual_segments(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    cache: Option<&AssociationCache>,
) -> Vec<RankedCandidate> {
    let normalized = normalize_pinyin(input);
    if normalized.is_empty() {
        return Vec::new();
    }
    let chunks = split_explicit_chunks(&normalized);
    if chunks.len() <= 1 {
        return retrieve_inner_limited(dictionary, fuzzy, tone_values, input, false, 100);
    }
    let mut segmentation_cache = SegmentationCache::default();
    let mut syllables: Vec<String> = Vec::new();
    let mut positions: Vec<usize> = Vec::new();
    for chunk in &chunks {
        if let Some(path) =
            complete_chunk_segmentation(dictionary, fuzzy, &chunk.text, &mut segmentation_cache)
        {
            let mut offset = chunk.start;
            for syllable in path {
                offset += syllable.text.len();
                positions.push(offset);
                syllables.push(syllable.text);
            }
        } else {
            positions.push(chunk.start + chunk.text.len());
            syllables.push(chunk.text.clone());
        }
    }

    let total = syllables.len();
    let mut candidates: Vec<RankedCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let compact_boundaries: Vec<usize> = syllables
        .iter()
        .scan(0usize, |state, syllable| {
            *state += syllable.len();
            Some(*state)
        })
        .collect();
    let compact = syllables.join("");

    {
        let mut combined_text = String::new();
        let mut combined_reading = String::new();
        let mut combined_weight = 0f64;
        let mut start_index = 0usize;
        let mut consumed_bytes = 0usize;
        while start_index < total {
            let compact_start = if start_index == 0 {
                0
            } else {
                compact_boundaries[start_index - 1]
            };
            let mut found = false;
            for end_index in ((start_index + 1)..=total).rev() {
                let compact_end = compact_boundaries[end_index - 1];
                let prefix = &compact[compact_start..compact_end];
                let Some(candidate) = full_input_best(dictionary, fuzzy, tone_values, prefix)
                else {
                    continue;
                };
                combined_text.push_str(&candidate.text);
                if let Some(ref reading) = candidate.reading {
                    if !combined_reading.is_empty() {
                        combined_reading.push(' ');
                    }
                    combined_reading.push_str(reading);
                }
                combined_weight += candidate.weight;
                start_index = end_index;
                consumed_bytes = positions[end_index - 1];
                found = true;
                break;
            }
            if !found {
                let prefix = &syllables[start_index];
                let Some(candidate) = full_input_best(dictionary, fuzzy, tone_values, prefix)
                else {
                    break;
                };
                combined_text.push_str(&candidate.text);
                if let Some(ref reading) = candidate.reading {
                    if !combined_reading.is_empty() {
                        combined_reading.push(' ');
                    }
                    combined_reading.push_str(reading);
                }
                combined_weight += candidate.weight;
                start_index += 1;
                consumed_bytes = positions[start_index - 1];
            }
        }
        if !combined_text.is_empty() {
            let combined_entries = dictionary.by_headword(&combined_text);
            let combined_annotation = combined_entries
                .first()
                .and_then(|entry| annotation_for_entry(dictionary, entry, tone_values));
            let combined_mandarin_only = combined_entries
                .first()
                .map(|entry| entry.is_mandarin_only())
                .unwrap_or(false);
            let first = RankedCandidate {
                text: combined_text.clone(),
                annotation: combined_annotation,
                ipa: None,
                layer: RetrievalLayer::GannyuExact,
                mandarin_only: combined_mandarin_only,
                weight: combined_weight + 1.0,
                reading: if combined_reading.is_empty() {
                    None
                } else {
                    Some(combined_reading)
                },
                mandarin_reading: None,
                consumed_bytes,
            };
            if !seen.contains(&first.text) {
                seen.insert(first.text.clone());
                candidates.push(first);
            }
        }
    }

    for n in (1..=total).rev() {
        let prefix = syllables[..n].join(" ");
        let consumed = positions[n - 1];
        let pre_count = candidates.len();
        let max_items = if n == 1 { 100 } else { 4 };
        let mut count = 0usize;
        let retrieve_max = if n == 1 { 100 } else { 5 };
        for mut c in
            retrieve_inner_limited(dictionary, fuzzy, tone_values, &prefix, true, retrieve_max)
        {
            if count >= retrieve_max {
                break;
            }
            if seen.contains(&c.text) {
                continue;
            }
            c.consumed_bytes = consumed;
            seen.insert(c.text.clone());
            candidates.push(c);
            count += 1;
        }
        if let Some(cache) = cache {
            if n > 1 {
                let limit = count.min(max_items);
                let _extra = inject_pairs_and_associations(
                    &mut candidates,
                    &mut seen,
                    dictionary,
                    cache,
                    tone_values,
                    pre_count,
                    limit,
                );
            }
        }
    }
    candidates
}

fn retrieve_inner(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    skip_step7: bool,
) -> Vec<RankedCandidate> {
    retrieve_inner_limited(
        dictionary,
        fuzzy,
        tone_values,
        input,
        skip_step7,
        usize::MAX,
    )
}

/// Same as retrieve_inner but stops early after collecting `max_candidates` (excluding
/// synonyms/mandarin-word candidates which are always appended regardless).
fn retrieve_inner_limited(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    skip_step7: bool,
    max_candidates: usize,
) -> Vec<RankedCandidate> {
    retrieve_inner_limited_with_boosts(
        dictionary,
        fuzzy,
        tone_values,
        input,
        skip_step7,
        max_candidates,
        None,
    )
}

fn retrieve_inner_limited_with_boosts(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    skip_step7: bool,
    max_candidates: usize,
    boosts: Option<&HashMap<String, u64>>,
) -> Vec<RankedCandidate> {
    let query = input.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let normalized = normalize_pinyin(query);

    // Check per-thread cache keyed by (dictionary_ptr, normalized, skip_step7, max_candidates).
    if boosts.is_none() && max_candidates != usize::MAX {
        let dictionary_id = dictionary.cache_id();
        let cache_key = (
            dictionary_id,
            normalized.clone(),
            skip_step7,
            max_candidates,
        );
        let hit = INNER_RETRIEVE_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned());
        if let Some(result) = hit {
            return result;
        }
    }

    let mut candidates: Vec<RankedCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut mandarin_words: HashSet<String> = HashSet::new();

    ANNOTATION_CACHE.with(|cache| cache.borrow_mut().clear());
    let query_syllables: Vec<&str> = pinyin_segments(&normalized);

    let mut direct_entry_ids: HashSet<usize> = HashSet::new();
    let mut match_kind: HashMap<usize, u8> = HashMap::new();
    let mut exact_dialect_ids: HashSet<usize> = HashSet::new();
    let mut exact_mandarin_ids: HashSet<usize> = HashSet::new();

    for entry in dictionary.by_dialect_pinyin_normalized(&normalized) {
        if let Some(id) = dictionary.entry_id(entry) {
            exact_dialect_ids.insert(id);
            direct_entry_ids.insert(id);
            match_kind.insert(id, 2u8);
        }
    }
    for entry in dictionary.by_mandarin_pinyin_normalized(&normalized) {
        if let Some(id) = dictionary.entry_id(entry) {
            exact_mandarin_ids.insert(id);
            direct_entry_ids.insert(id);
            match_kind.entry(id).or_insert(1u8);
        }
    }
    for entry in dictionary.by_mandarin_word_pinyin_normalized(&normalized) {
        if let Some(id) = dictionary.entry_id(entry) {
            direct_entry_ids.insert(id);
            match_kind.entry(id).or_insert(1u8);
        }
    }

    if query_syllables.len() >= 2 {
        collect_per_syllable_hits(
            dictionary,
            fuzzy,
            &query_syllables,
            &mut direct_entry_ids,
            &mut match_kind,
            &exact_dialect_ids,
            &exact_mandarin_ids,
        );
    } else if normalized.len() == 1 && normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        // Single-letter input: treat as initial-letter match across all
        // positions (e.g. "z" → entries starting with "z" at any position).
        let ch = normalized.chars().next().unwrap();
        let mut pos = 0usize;
        while !dictionary.initial_match_ids(pos, ch).is_empty() {
            for &idx in dictionary.initial_match_ids(pos, ch) {
                let id = idx as usize;
                if !direct_entry_ids.contains(&id) {
                    direct_entry_ids.insert(id);
                    match_kind.entry(id).or_insert(0u8);
                }
            }
            pos += 1;
        }
    } else {
        segment_and_collect_hits(
            dictionary,
            fuzzy,
            &normalized,
            &mut direct_entry_ids,
            &mut match_kind,
        );
        // Also try coda-doubled variants (entering-tone coda shared as
        // next onset).  E.g., "niteu" → "nitteu" → segment as nit+teu.
        for doubled in &coda_doubled_variants(&normalized) {
            segment_and_collect_hits(
                dictionary,
                fuzzy,
                doubled,
                &mut direct_entry_ids,
                &mut match_kind,
            );
        }
    }

    let mut fuzzy_forms: Vec<String> = Vec::new();
    for scheme in [SyllableScheme::GonPin, SyllableScheme::GonHan] {
        for normalized_fuzzy in fuzzy.normalize(&normalized, scheme) {
            if normalized_fuzzy.text != normalized && !fuzzy_forms.contains(&normalized_fuzzy.text)
            {
                fuzzy_forms.push(normalized_fuzzy.text);
            }
        }
    }
    for form in &fuzzy_forms {
        for entry in dictionary.by_dialect_pinyin_normalized(form) {
            if let Some(id) = dictionary.entry_id(entry) {
                if !direct_entry_ids.contains(&id) {
                    direct_entry_ids.insert(id);
                    match_kind.insert(id, 0u8);
                }
            }
        }
        for entry in dictionary.by_mandarin_pinyin_normalized(form) {
            if let Some(id) = dictionary.entry_id(entry) {
                if !direct_entry_ids.contains(&id) {
                    direct_entry_ids.insert(id);
                    match_kind.insert(id, 0u8);
                }
            }
        }
    }

    let mut direct_entries: Vec<(usize, u8)> = Vec::new();
    for &id in &direct_entry_ids {
        let kind = match_kind.get(&id).copied().unwrap_or(0u8);
        direct_entries.push((id, kind));
    }

    direct_entries.sort_by_cached_key(|(idx, _)| {
        let entry = dictionary.entries().get(*idx);
        (
            std::cmp::Reverse(
                entry
                    .and_then(|item| boosts.and_then(|map| map.get(&item.headword)))
                    .copied()
                    .unwrap_or(0),
            ),
            std::cmp::Reverse(entry.and_then(|item| item.frequency).unwrap_or(0)),
            *idx,
        )
    });

    let mut expanded_direct_entries: Vec<(usize, u8)> = Vec::new();
    if max_candidates == usize::MAX {
        expanded_direct_entries = direct_entries.clone();
        let (entry_list_entries, entry_kinds): (Vec<_>, Vec<_>) = direct_entries
            .iter()
            .filter_map(|(entry_index, kind)| {
                dictionary
                    .entries()
                    .get(*entry_index)
                    .map(|entry| (entry, *kind))
            })
            .unzip();
        // Uncapped path (public retrieve API): build everything, rayon-parallel, then merge.
        let built: Vec<Vec<RankedCandidate>> = if entry_list_entries.len() >= 3 {
            use rayon::prelude::*;
            entry_list_entries
                .par_iter()
                .zip(entry_kinds.par_iter())
                .map(|(entry, kind)| {
                    let layer = match kind {
                        2 => RetrievalLayer::GannyuExact,
                        1 => RetrievalLayer::MandarinExact,
                        _ => RetrievalLayer::Fuzzy,
                    };
                    let local: Vec<RankedCandidate> = vec![gan_candidate(dictionary, entry, layer, tone_values)];
                    // synonyms and mandarin word candidate handled serially later to preserve order
                    local
                })
                .collect()
        } else {
            entry_list_entries
                .iter()
                .zip(entry_kinds.iter())
                .map(|(entry, kind)| {
                    let layer = match kind {
                        2 => RetrievalLayer::GannyuExact,
                        1 => RetrievalLayer::MandarinExact,
                        _ => RetrievalLayer::Fuzzy,
                    };
                    vec![gan_candidate(dictionary, entry, layer, tone_values)]
                })
                .collect()
        };

        // Merge built candidates preserving frequency sort (direct_entries were sorted earlier)
        for built_group in built.into_iter() {
            for candidate in built_group {
                if seen.contains(&candidate.text) {
                    continue;
                }
                seen.insert(candidate.text.clone());
                candidates.push(candidate);
            }
        }
    } else {
        // Capped path (per-keystroke retrieval): build candidates lazily
        // inside the merge loop and stop at max_candidates.  The old code
        // built full annotated candidates for every hit (thousands for a
        // popular initial letter) and discarded all but the first ~100.
        // Output is identical: merge order and `seen` evolution match the
        // old loop, and skipped builds are pure (memo-cache side effects
        // only).  Once the cap is reached no further insertions can happen,
        // so checking the cap at the top of the loop is equivalent to the
        // old per-candidate check.
        let mut main_count = 0usize;
        for (entry_index, kind) in &direct_entries {
            if main_count >= max_candidates {
                break;
            }
            let Some(entry) = dictionary.entries().get(*entry_index) else {
                continue;
            };
            let layer = match kind {
                2 => RetrievalLayer::GannyuExact,
                1 => RetrievalLayer::MandarinExact,
                _ => RetrievalLayer::Fuzzy,
            };
            let candidate = gan_candidate(dictionary, entry, layer, tone_values);
            if seen.contains(&candidate.text) {
                continue;
            }
            expanded_direct_entries.push((*entry_index, *kind));
            seen.insert(candidate.text.clone());
            candidates.push(candidate);
            main_count += 1;
        }
    }

    // Now handle synonyms and mandarin-word candidates serially to avoid race on seen set
    for (entry_index, kind) in &expanded_direct_entries {
        let Some(entry) = dictionary.entries().get(*entry_index) else {
            continue;
        };
        let layer = match kind {
            2 => RetrievalLayer::GannyuExact,
            1 => RetrievalLayer::MandarinExact,
            _ => RetrievalLayer::Fuzzy,
        };
        push_synonyms(
            &mut candidates,
            &mut seen,
            dictionary,
            entry,
            layer,
            tone_values,
        );
        for mw_cand in mandarin_word_candidates_for_gan_entry(dictionary, entry, tone_values) {
            insert_candidate_after(&mut candidates, &mut seen, &entry.headword, mw_cand);
        }
    }

    let direct_slice: Vec<&DictionaryEntry> = expanded_direct_entries
        .iter()
        .filter_map(|(idx, _)| dictionary.entries().get(*idx))
        .collect();
    push_reverse_gan(
        dictionary,
        &direct_slice,
        &mut candidates,
        &mut seen,
        &mut mandarin_words,
        tone_values,
        RetrievalLayer::GannyuExact,
    );

    for text in &mandarin_words {
        let mw_related = dictionary.by_headword(text);
        let own_reading = aggregate_annotation(
            &mw_related,
            tone_values,
            &dictionary.new_old_map,
            &dictionary.heteronym_chars,
            &dictionary.paired_readings,
        );
        let gan_reverse = gan_annotation_for_mandarin_entry(dictionary, text, tone_values);
        let annotation = append_associated_suffix(
            dictionary,
            text,
            guan_hua_ci_annotation(own_reading, gan_reverse),
        );
        let freq = mw_related.first().and_then(|e| e.frequency);
        let mandarin_word = RankedCandidate {
            text: text.clone(),
            annotation,
            ipa: None,
            layer: RetrievalLayer::GannyuExact,
            mandarin_only: true,
            weight: RetrievalLayer::GannyuExact.base_weight() + frequency_factor(freq),
            reading: None,
            mandarin_reading: None,
            consumed_bytes: 0,
        };
        push_candidate(&mut candidates, &mut seen, mandarin_word);
    }

    if !skip_step7 && candidates.is_empty() && query_syllables.len() < 2 && normalized.len() >= 4 {
        let segments = segment_sentence_with_consumed(dictionary, fuzzy, tone_values, &normalized);
        if !segments.is_empty() {
            let (_, seg_candidates) = &segments[0];
            for c in seg_candidates.clone() {
                push_candidate(&mut candidates, &mut seen, c);
            }
        }
    }

    // Store in per-thread cache for future calls with different max_candidates.
    // Only cache when called with a finite max_candidates (skip the uncapped path).
    if boosts.is_none() && max_candidates != usize::MAX {
        let dictionary_id = dictionary.cache_id();
        INNER_RETRIEVE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() >= 512 {
                cache.clear();
            }
            cache.insert(
                (dictionary_id, normalized, skip_step7, max_candidates),
                candidates.clone(),
            );
        });
    }

    candidates
}

fn full_input_best(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
) -> Option<RankedCandidate> {
    retrieve_inner_limited(dictionary, fuzzy, tone_values, input, true, 1)
        .into_iter()
        .next()
}

fn candidate_matches_sentence_segments(
    dictionary: &Dictionary,
    candidate_text: &str,
    segments: &[SyllableCandidate],
) -> bool {
    dictionary.by_headword(candidate_text).iter().any(|entry| {
        let dialect_len = pinyin_segments(&entry.dialect_pinyin).len();
        let mandarin_len = pinyin_segments(&entry.mandarin_pinyin).len();
        (dialect_len == segments.len() || mandarin_len == segments.len())
            && segments
                .iter()
                .all(|segment| segment.ids.contains(&entry.entry_index))
    })
}

fn candidate_starts_with_sentence_segments(
    dictionary: &Dictionary,
    candidate_text: &str,
    segments: &[SyllableCandidate],
) -> bool {
    dictionary.by_headword(candidate_text).iter().any(|entry| {
        let candidate_len = pinyin_segments(&entry.dialect_pinyin)
            .len()
            .max(pinyin_segments(&entry.mandarin_pinyin).len());
        candidate_len > 0
            && segments
                .iter()
                .take(candidate_len)
                .all(|segment| segment.ids.contains(&entry.entry_index))
    })
}

fn sentence_candidate_content_weight(candidate: &RankedCandidate) -> f64 {
    candidate.weight - candidate.layer.base_weight()
}

fn sentence_candidate_frequency(dictionary: &Dictionary, candidate: &RankedCandidate) -> u64 {
    dictionary
        .by_headword(&candidate.text)
        .into_iter()
        .filter_map(|entry| entry.frequency)
        .max()
        .unwrap_or(0)
}

fn compact_sentence_form(pinyin: &str) -> String {
    pinyin_segments(pinyin)
        .iter()
        .map(|segment| strip_tone(segment))
        .collect::<Vec<_>>()
        .join("")
}

fn exact_prefix_candidates(
    dictionary: &Dictionary,
    tone_values: &HashMap<String, u8>,
    prefix: &str,
    limit: usize,
) -> Vec<RankedCandidate> {
    let compact = prefix.replace(' ', "").to_ascii_lowercase();
    if compact.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut ranked: Vec<(usize, u8)> = dictionary
        .lookup_prefix_ids(&compact)
        .into_iter()
        .filter_map(|id| {
            let entry = dictionary.entries().get(id as usize)?;
            let dialect_match = compact_sentence_form(&entry.dialect_pinyin).starts_with(&compact);
            let mandarin_match =
                compact_sentence_form(&entry.mandarin_pinyin).starts_with(&compact);
            let kind = if dialect_match {
                Some(2u8)
            } else if mandarin_match {
                Some(1u8)
            } else {
                None
            }?;
            Some((id as usize, kind))
        })
        .collect();
    ranked.sort_by_cached_key(|(idx, _)| {
        std::cmp::Reverse(
            dictionary
                .entries()
                .get(*idx)
                .and_then(|e| e.frequency)
                .unwrap_or(0),
        )
    });

    let mut candidates = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (entry_index, kind) in ranked.into_iter().take(limit) {
        let Some(entry) = dictionary.entries().get(entry_index) else {
            continue;
        };
        let layer = match kind {
            2 => RetrievalLayer::GannyuExact,
            1 => RetrievalLayer::MandarinExact,
            _ => RetrievalLayer::Fuzzy,
        };
        let candidate = gan_candidate(dictionary, entry, layer, tone_values);
        if seen.insert(candidate.text.clone()) {
            candidates.push(candidate);
        }
    }
    candidates
}

type ChunkPath = Option<(Vec<SyllableCandidate>, Vec<usize>)>;
type PathCache = HashMap<String, ChunkPath>;
type SentenceRetrievalCache = HashMap<(String, usize), Vec<RankedCandidate>>;

struct BoundaryCandidateRequest<'a> {
    input: &'a str,
    boundaries: &'a [usize],
    segments: &'a [SyllableCandidate],
    multi_boundary_max: usize,
    single_boundary_max: usize,
}

fn sentence_chunk_path(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    cache: &mut SegmentationCache,
) -> Option<(Vec<SyllableCandidate>, Vec<usize>)> {
    let path = best_chunk_segmentation(dictionary, fuzzy, input, cache)?;
    let mut boundaries = Vec::with_capacity(path.len());
    let mut cumulative = 0usize;
    for syllable in &path {
        cumulative += syllable.text.len();
        boundaries.push(cumulative);
    }
    Some((path, boundaries))
}

fn cached_sentence_chunk_path(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    segmentation_cache: &mut SegmentationCache,
    path_cache: &mut PathCache,
) -> Option<(Vec<SyllableCandidate>, Vec<usize>)> {
    if let Some(cached) = path_cache.get(input) {
        return cached.clone();
    }
    let path = sentence_chunk_path(dictionary, fuzzy, input, segmentation_cache);
    path_cache.insert(input.to_string(), path.clone());
    path
}

fn cached_sentence_prefix_candidates(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    prefix: &str,
    retrieve_max: usize,
    retrieval_cache: &mut SentenceRetrievalCache,
) -> Vec<RankedCandidate> {
    let key = (prefix.to_string(), retrieve_max);
    if let Some(cached) = retrieval_cache.get(&key) {
        return cached.clone();
    }

    let exact_limit = retrieve_max.min(16);
    let mut candidates = exact_prefix_candidates(dictionary, tone_values, prefix, exact_limit);
    let exact_satisfied = retrieve_max != usize::MAX && candidates.len() >= retrieve_max;
    if !exact_satisfied {
        let mut seen: HashSet<String> = candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect();
        for candidate in
            retrieve_inner_limited(dictionary, fuzzy, tone_values, prefix, true, retrieve_max)
        {
            if seen.insert(candidate.text.clone()) {
                candidates.push(candidate);
            }
        }
    }

    retrieval_cache.insert(key, candidates.clone());
    candidates
}

fn best_filtered_boundary_candidate(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    segments: &[SyllableCandidate],
    boundaries: &[usize],
    retrieval_cache: &mut SentenceRetrievalCache,
) -> Option<RankedCandidate> {
    for n in (1..=boundaries.len()).rev() {
        let consumed = boundaries[n - 1];
        let prefix = &input[..consumed];
        let mut best: Option<RankedCandidate> = None;
        for mut candidate in cached_sentence_prefix_candidates(
            dictionary,
            fuzzy,
            tone_values,
            prefix,
            5,
            retrieval_cache,
        ) {
            if !candidate_matches_sentence_segments(dictionary, &candidate.text, &segments[..n]) {
                continue;
            }
            candidate.consumed_bytes = consumed;
            let replace = match &best {
                None => true,
                Some(current) => {
                    let candidate_content = sentence_candidate_content_weight(&candidate);
                    let current_content = sentence_candidate_content_weight(current);
                    candidate_content > current_content
                        || (candidate_content == current_content
                            && (candidate.weight > current.weight
                                || (candidate.weight == current.weight
                                    && sentence_candidate_frequency(dictionary, &candidate)
                                        > sentence_candidate_frequency(dictionary, current))))
                }
            };
            if replace {
                best = Some(candidate);
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

fn no_tail_entering_alternative_path(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    path: &[SyllableCandidate],
    segmentation_cache: &mut SegmentationCache,
    path_cache: &mut PathCache,
) -> Option<(Vec<SyllableCandidate>, Vec<usize>)> {
    let first = path.first()?;
    if input.starts_with(&first.text) {
        return None;
    }
    if !matches!(first.text.chars().last(), Some('k' | 't')) {
        return None;
    }
    let shortened = &first.text[..first.text.len() - 1];
    if shortened.is_empty() || !input.starts_with(shortened) {
        return None;
    }
    let alt_first =
        classify_syllable_candidate_any_position(dictionary, fuzzy, shortened, segmentation_cache)?;
    let mut alt_path = vec![alt_first];
    let mut boundaries = vec![shortened.len()];
    if shortened.len() < input.len() {
        let suffix = &input[shortened.len()..];
        let (suffix_path, suffix_boundaries) =
            cached_sentence_chunk_path(dictionary, fuzzy, suffix, segmentation_cache, path_cache)?;
        boundaries.extend(
            suffix_boundaries
                .into_iter()
                .map(|boundary| boundary + shortened.len()),
        );
        alt_path.extend(suffix_path);
    }
    Some((alt_path, boundaries))
}

fn collect_boundary_aligned_candidates(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    request: BoundaryCandidateRequest<'_>,
    retrieval_cache: &mut SentenceRetrievalCache,
) -> Vec<RankedCandidate> {
    const SENTENCE_MULTI_BOUNDARY_SCAN: usize = 5;
    const SENTENCE_SINGLE_BOUNDARY_SCAN: usize = 100;
    let BoundaryCandidateRequest {
        input,
        boundaries,
        segments,
        multi_boundary_max,
        single_boundary_max,
    } = request;
    let mut candidates = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for n in (1..=boundaries.len()).rev() {
        let consumed = boundaries[n - 1];
        let prefix = &input[..consumed];
        let keep_max = if n == 1 {
            single_boundary_max
        } else {
            multi_boundary_max
        };
        let retrieve_max = if n == 1 {
            SENTENCE_SINGLE_BOUNDARY_SCAN
        } else {
            SENTENCE_MULTI_BOUNDARY_SCAN
        };
        let mut count = 0usize;
        for mut candidate in cached_sentence_prefix_candidates(
            dictionary,
            fuzzy,
            tone_values,
            prefix,
            retrieve_max,
            retrieval_cache,
        ) {
            if count >= keep_max {
                break;
            }
            if seen.contains(&candidate.text) {
                continue;
            }
            // 过滤「前缀匹配但音节不匹配」的候选：候选词的每个音节必须
            // 在 segments 对应位置命中，否则（如"中华民族"第 4 音节 cuk6
            // 不匹配输入的 wai）不收集。
            // 输入缓存只有单独音节（boundaries.len()==1）时不过滤：
            // 单音节可能是更长词的前缀（如 "zung" 是 "zungguet" 中国的前缀），
            // 应保留。
            if boundaries.len() > 1
                && !candidate_starts_with_sentence_segments(dictionary, &candidate.text, segments)
            {
                continue;
            }
            candidate.consumed_bytes = consumed;
            seen.insert(candidate.text.clone());
            candidates.push(candidate);
            count += 1;
        }
    }
    candidates
}

pub(crate) fn retrieve_sentence_input_cached(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    cache: Option<&AssociationCache>,
    retrieval_cache: &mut SentenceRetrievalCache,
) -> Vec<RankedCandidate> {
    let normalized = normalize_pinyin(input);
    let mut candidates: Vec<RankedCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut saw_sentence_segmentation = false;
    let mut segmentation_cache = SegmentationCache::default();
    let mut path_cache: PathCache =
        HashMap::new();
    if let Some((_first_path, first_boundaries)) = cached_sentence_chunk_path(
        dictionary,
        fuzzy,
        &normalized,
        &mut segmentation_cache,
        &mut path_cache,
    ) {
        saw_sentence_segmentation = true;

        let mut combined_text = String::new();
        let mut combined_reading = String::new();
        let mut combined_weight = 0f64;
        let mut start = 0usize;
        while start < normalized.len() {
            let remaining = &normalized[start..];
            let Some((path, boundaries)) = cached_sentence_chunk_path(
                dictionary,
                fuzzy,
                remaining,
                &mut segmentation_cache,
                &mut path_cache,
            ) else {
                break;
            };
            let Some(mut candidate) = best_filtered_boundary_candidate(
                dictionary,
                fuzzy,
                tone_values,
                remaining,
                &path,
                &boundaries,
                retrieval_cache,
            ) else {
                break;
            };
            if let Some((alt_path, alt_boundaries)) = no_tail_entering_alternative_path(
                dictionary,
                fuzzy,
                remaining,
                &path,
                &mut segmentation_cache,
                &mut path_cache,
            ) {
                if let Some(alt_candidate) = best_filtered_boundary_candidate(
                    dictionary,
                    fuzzy,
                    tone_values,
                    remaining,
                    &alt_path,
                    &alt_boundaries,
                    retrieval_cache,
                ) {
                    let candidate_content = sentence_candidate_content_weight(&candidate);
                    let alt_content = sentence_candidate_content_weight(&alt_candidate);
                    if alt_content > candidate_content
                        || (alt_content == candidate_content
                            && alt_candidate.weight > candidate.weight)
                    {
                        candidate = alt_candidate;
                    }
                }
            }
            combined_text.push_str(&candidate.text);
            if let Some(ref reading) = candidate.reading {
                if !combined_reading.is_empty() {
                    combined_reading.push(' ');
                }
                combined_reading.push_str(reading);
            }
            combined_weight += candidate.weight;
            start += candidate.consumed_bytes;
        }
        if !combined_text.is_empty() {
            let combined_entries = dictionary.by_headword(&combined_text);
            let combined_annotation = combined_entries
                .first()
                .and_then(|entry| annotation_for_entry(dictionary, entry, tone_values));
            let combined_mandarin_only = combined_entries
                .first()
                .map(|entry| entry.is_mandarin_only())
                .unwrap_or(false);
            let first = RankedCandidate {
                text: combined_text.clone(),
                annotation: combined_annotation,
                ipa: None,
                layer: RetrievalLayer::GannyuExact,
                mandarin_only: combined_mandarin_only,
                weight: combined_weight + 1.0,
                reading: if combined_reading.is_empty() {
                    None
                } else {
                    Some(combined_reading)
                },
                mandarin_reading: None,
                consumed_bytes: start,
            };
            if !seen.contains(&first.text) {
                seen.insert(first.text.clone());
                candidates.insert(0, first);
            }
        }

        let first_candidates = collect_boundary_aligned_candidates(
            dictionary,
            fuzzy,
            tone_values,
            BoundaryCandidateRequest {
                input: &normalized,
                boundaries: &first_boundaries,
                segments: &_first_path,
                multi_boundary_max: 5,
                single_boundary_max: usize::MAX,
            },
            retrieval_cache,
        );
        for candidate in first_candidates {
            if seen.contains(&candidate.text) {
                continue;
            }
            seen.insert(candidate.text.clone());
            candidates.push(candidate);
        }
        if let Some(_cache) = cache {
            // Sentence-path retrieval currently doesn't use pair injection in the
            // runtime pipeline; this branch stays intentionally inert until a
            // filtered-group version of association injection is needed.
        }
    }

    if candidates.len() < 8 {
        for doubled in &coda_doubled_variants(&normalized) {
            let Some((_path, boundaries)) = cached_sentence_chunk_path(
                dictionary,
                fuzzy,
                doubled,
                &mut segmentation_cache,
                &mut path_cache,
            ) else {
                continue;
            };
            saw_sentence_segmentation = true;
            let chunk_candidates = collect_boundary_aligned_candidates(
                dictionary,
                fuzzy,
                tone_values,
                BoundaryCandidateRequest {
                    input: doubled,
                    boundaries: &boundaries,
                    segments: &_path,
                    multi_boundary_max: 5,
                    single_boundary_max: usize::MAX,
                },
                retrieval_cache,
            );
            for candidate in chunk_candidates {
                if seen.contains(&candidate.text) {
                    continue;
                }
                let mut mapped = candidate.clone();
                mapped.consumed_bytes =
                    map_doubled_offset_to_original(&normalized, doubled, candidate.consumed_bytes);
                seen.insert(mapped.text.clone());
                candidates.push(mapped);
            }
            if let Some(_cache) = cache {
                // See note above for the non-doubled sentence path.
            }
        }
    }

    if candidates.is_empty() {
        if saw_sentence_segmentation {
            return Vec::new();
        }
        return retrieve_inner(dictionary, fuzzy, tone_values, input, false);
    }
    candidates
}

/// for multi-syllable prefixes, no limit for the single-syllable group.
/// Every candidate carries the exact `consumed_bytes` for that prefix.
pub fn retrieve_sentence_input(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    cache: Option<&AssociationCache>,
) -> Vec<RankedCandidate> {
    let mut retrieval_cache: HashMap<(String, usize), Vec<RankedCandidate>> = HashMap::new();
    retrieve_sentence_input_cached(
        dictionary,
        fuzzy,
        tone_values,
        input,
        cache,
        &mut retrieval_cache,
    )
}

/// Segment a continuous Latin input into word boundaries using greedy
/// longest-match-first. Returns one `Vec<RankedCandidate>` per segment;
/// users commit the first segment, then the next set of candidates appears.
pub fn segment_sentence(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
) -> Vec<Vec<RankedCandidate>> {
    segment_sentence_with_consumed(dictionary, fuzzy, tone_values, input)
        .into_iter()
        .map(|(_, candidates)| candidates)
        .collect()
}

/// Find segmentation boundaries for a continuous input string.
/// Returns a list of byte positions where word boundaries occur,
/// suitable for inserting `'` delimiters in the input display.
pub fn segment_boundaries(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
) -> Vec<usize> {
    let segments = segment_sentence_with_consumed(dictionary, fuzzy, tone_values, input);
    if segments.is_empty() {
        return Vec::new();
    }
    let mut boundaries = Vec::with_capacity(segments.len() - 1);
    let mut cumulative = 0usize;
    let last_idx = segments.len() - 1;
    for (i, (consumed, _)) in segments.iter().enumerate() {
        cumulative += consumed;
        if i < last_idx {
            boundaries.push(cumulative);
        }
    }
    boundaries
}

/// Format preedit text using the Fcitx5 display rule without changing the
/// input that retrieval receives. Manual spaces and apostrophes remain
/// visible; each continuous run is auto-segmented with display-only spaces.
/// A partial active candidate takes precedence, matching Fcitx5 preedit.
pub fn format_preedit_display(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
    consumed_bytes: usize,
) -> String {
    if consumed_bytes > 0 && consumed_bytes < input.len() && input.is_char_boundary(consumed_bytes)
    {
        let active = &input[..consumed_bytes];
        let mut rest = &input[consumed_bytes..];
        if rest.as_bytes().first() == Some(&32) || rest.as_bytes().first() == Some(&39) {
            rest = &rest[1..];
        }
        return if rest.is_empty() {
            active.to_owned()
        } else {
            format!("{active} {rest}")
        };
    }

    if input.len() < 4 {
        return input.to_owned();
    }

    let mut display = String::with_capacity(input.len());
    for run in input.split_inclusive(|ch| ch as u32 == 32 || ch as u32 == 39) {
        let (continuous, separator) = match run.chars().last() {
            Some(ch) if ch as u32 == 32 || ch as u32 == 39 => {
                (&run[..run.len() - ch.len_utf8()], Some(ch))
            }
            _ => (run, None),
        };
        if continuous.len() < 4 {
            display.push_str(continuous);
        } else {
            let boundaries = segment_boundaries(dictionary, fuzzy, tone_values, continuous);
            let mut cursor = 0;
            for boundary in boundaries {
                if boundary <= continuous.len() && continuous.is_char_boundary(boundary) {
                    display.push_str(&continuous[cursor..boundary]);
                    display.push(char::from(32));
                    cursor = boundary;
                }
            }
            display.push_str(&continuous[cursor..]);
        }
        if let Some(separator) = separator {
            display.push(separator);
        }
    }
    display
}

/// Same as segment_sentence but returns (consumed_chars, candidates) per segment.
pub fn segment_sentence_with_consumed(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    tone_values: &HashMap<String, u8>,
    input: &str,
) -> Vec<(usize, Vec<RankedCandidate>)> {
    let normalized = normalize_pinyin(input);
    let mut result = Vec::new();
    let mut cursor = 0usize;
    let mut cache = SegmentationCache::default();
    let mut retrieval_cache: HashMap<(String, usize), Vec<RankedCandidate>> = HashMap::new();
    while cursor < normalized.len() {
        let remaining = &normalized[cursor..];
        let Some(path) = best_chunk_segmentation(dictionary, fuzzy, remaining, &mut cache) else {
            cursor += 1;
            continue;
        };
        let Some(first) = path.first() else {
            cursor += 1;
            continue;
        };
        let mut boundaries = Vec::with_capacity(path.len());
        let mut cumulative = 0usize;
        for syllable in &path {
            cumulative += syllable.text.len();
            boundaries.push(cumulative);
        }
        result.push((
            first.text.len(),
            collect_boundary_aligned_candidates(
                dictionary,
                fuzzy,
                tone_values,
                BoundaryCandidateRequest {
                    input: remaining,
                    boundaries: &boundaries,
                    segments: &path,
                    multi_boundary_max: 5,
                    single_boundary_max: 100,
                },
                &mut retrieval_cache,
            ),
        ));
        cursor += first.text.len();
    }
    result
}
