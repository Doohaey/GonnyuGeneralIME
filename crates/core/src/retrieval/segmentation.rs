/// Generate variants of the input where an entering-tone coda consonant
/// (t or k) is doubled so it can serve as both the coda of one
/// syllable and the onset of the next.  E.g., "niteu" → "nitteu"
/// segments as nit+teu, "nikteu" → "nikkteu" tries nik+kteu (usually
/// falls back to nik+teu via fuzzy cross-matching).
fn coda_doubled_variants(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut variants: Vec<String> = Vec::new();
    for i in 0..n {
        let c = chars[i];
        if c == 't' || c == 'k' {
            let mut variant = String::with_capacity(n + 1);
            variant.push_str(&input[..=i]);
            variant.push(c);
            variant.push_str(&input[i + 1..]);
            if !variants.contains(&variant) {
                variants.push(variant);
            }
        }
    }
    variants
}

fn map_doubled_offset_to_original(original: &str, doubled: &str, doubled_offset: usize) -> usize {
    let original_bytes = original.as_bytes();
    let doubled_bytes = doubled.as_bytes();
    let mut original_offset = 0usize;
    for &byte in doubled_bytes
        .iter()
        .take(doubled_offset.min(doubled_bytes.len()))
    {
        if original_offset < original_bytes.len() && byte == original_bytes[original_offset] {
            original_offset += 1;
        }
    }
    original_offset
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentTier {
    GanExact,
    GanCompatible,
    Mandarin,
    Initial,
    GanDeprecated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SegmentScore {
    bad_count: usize,
    mandarin_count: usize,
    syllable_count: usize,
    compatible_count: usize,
    exact_count: usize,
    profile_sum: i64,
}

impl SegmentScore {
    fn extend(&self, tier: SegmentTier, profile: i64) -> Self {
        let mut next = self.clone();
        next.syllable_count += 1;
        match tier {
            SegmentTier::GanExact => next.exact_count += 1,
            SegmentTier::GanCompatible => next.compatible_count += 1,
            SegmentTier::Mandarin => next.mandarin_count += 1,
            SegmentTier::Initial | SegmentTier::GanDeprecated => next.bad_count += 1,
        }
        next.profile_sum += profile;
        next
    }

    fn better_than(&self, other: &Self) -> bool {
        if self.bad_count != other.bad_count {
            return self.bad_count < other.bad_count;
        }
        if self.mandarin_count != other.mandarin_count {
            return self.mandarin_count < other.mandarin_count;
        }
        if self.syllable_count != other.syllable_count {
            return self.syllable_count < other.syllable_count;
        }
        if self.compatible_count != other.compatible_count {
            return self.compatible_count < other.compatible_count;
        }
        if self.exact_count != other.exact_count {
            return self.exact_count > other.exact_count;
        }
        self.profile_sum > other.profile_sum
    }
}

#[derive(Debug, Clone)]
struct SyllableCandidate {
    text: String,
    ids: HashSet<usize>,
    tier: SegmentTier,
    profile: i64,
}

#[derive(Debug, Clone)]
struct WordSegmentation {
    consumed: usize,
    ids: HashSet<usize>,
    segments: Vec<SyllableCandidate>,
    score: SegmentScore,
}

#[derive(Debug, Clone)]
struct ChunkSegmentation {
    consumed: usize,
    score: SegmentScore,
    path: Vec<SyllableCandidate>,
}

type SourceMatches = (Rc<HashSet<usize>>, Rc<HashSet<usize>>);

#[derive(Default)]
struct SegmentationCache {
    fuzzy_forms: HashMap<String, Vec<String>>,
    ids_by_source: HashMap<(usize, String), SourceMatches>,
    classify_by_position: HashMap<(usize, String), Option<SyllableCandidate>>,
    classify_any_position: HashMap<String, Option<SyllableCandidate>>,
}

fn entry_matches_syllable_at_position(
    entry: &DictionaryEntry,
    position: usize,
    form: &str,
    dialect: bool,
) -> bool {
    let pinyin = if dialect {
        &entry.dialect_pinyin
    } else {
        &entry.mandarin_pinyin
    };
    pinyin_segments(pinyin)
        .get(position)
        .map(|segment| strip_tone(segment) == form)
        .unwrap_or(false)
}

/// Dialect position match with multi-reading support (等权): a form matches
/// when it is the entry's own stored syllable at `position` OR any known
/// reading of the character at that position, so word candidates typed with
/// an alternate reading keep the GannyuExact layer instead of dropping to
/// Fuzzy.
fn entry_matches_dialect_at_position(
    dictionary: &Dictionary,
    entry: &DictionaryEntry,
    position: usize,
    form: &str,
) -> bool {
    if entry_matches_syllable_at_position(entry, position, form, true) {
        return true;
    }
    let Some(character) = entry.headword.chars().nth(position) else {
        return false;
    };
    let syllables = pinyin_segments(&entry.dialect_pinyin);
    let Some(stored) = syllables.get(position) else {
        return false;
    };
    paired_alternates_for_stored(
        &character.to_string(),
        strip_tone(stored),
        &dictionary.paired_readings,
    )
    .map(|readings| readings.iter().any(|reading| reading == form))
    .unwrap_or(false)
}

fn ids_for_form_by_source(
    dictionary: &Dictionary,
    position: usize,
    form: &str,
    cache: &mut SegmentationCache,
) -> (Rc<HashSet<usize>>, Rc<HashSet<usize>>) {
    let key = (position, form.to_string());
    if let Some(cached) = cache.ids_by_source.get(&key) {
        return cached.clone();
    }
    let mut dialect_ids = HashSet::new();
    let mut mandarin_ids = HashSet::new();
    for entry in dictionary.by_syllable_at_position(position, form) {
        if entry_matches_dialect_at_position(dictionary, entry, position, form) {
            dialect_ids.insert(entry.entry_index);
        }
        if entry_matches_syllable_at_position(entry, position, form, false) {
            mandarin_ids.insert(entry.entry_index);
        }
    }
    let dialect_rc = Rc::new(dialect_ids);
    let mandarin_rc = Rc::new(mandarin_ids);
    cache
        .ids_by_source
        .insert(key, (Rc::clone(&dialect_rc), Rc::clone(&mandarin_rc)));
    (dialect_rc, mandarin_rc)
}

fn is_deprecated_gan_input(fuzzy: &FuzzyMap, syllable: &str) -> bool {
    let compact = syllable.trim().to_ascii_lowercase();
    if compact.is_empty() {
        return false;
    }
    if compact == "ieu" {
        return true;
    }
    let Some(first) = compact.chars().next() else {
        return false;
    };
    if !matches!(first, 'i' | 'o' | 'u') {
        return false;
    }
    fuzzy
        .normalize(&compact, SyllableScheme::GonPin)
        .into_iter()
        .any(|normalized| normalized.text != compact && !normalized.text.is_empty())
}

fn classify_syllable_candidate(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    position: usize,
    candidate: &str,
    cache: &mut SegmentationCache,
) -> Option<SyllableCandidate> {
    let compact = candidate.trim().to_ascii_lowercase();
    if compact.is_empty() {
        return None;
    }
    let cache_key = (position, compact.clone());
    if let Some(cached) = cache.classify_by_position.get(&cache_key) {
        return cached.clone();
    }

    let deprecated = is_deprecated_gan_input(fuzzy, &compact);
    let (exact_dialect_ids, exact_mandarin_ids) =
        ids_for_form_by_source(dictionary, position, &compact, cache);
    let mut fuzzy_dialect_ids = HashSet::new();
    let mut fuzzy_mandarin_ids = HashSet::new();
    let mut profile = *dictionary.syllable_profile.get(&compact).unwrap_or(&0);

    for form in fuzzy_forms_for(fuzzy, &compact, cache) {
        let (dialect_ids, mandarin_ids) =
            ids_for_form_by_source(dictionary, position, &form, cache);
        if !dialect_ids.is_empty() {
            fuzzy_dialect_ids.extend(dialect_ids.iter().copied());
            profile = profile.max(*dictionary.syllable_profile.get(&form).unwrap_or(&0));
        }
        if !mandarin_ids.is_empty() {
            fuzzy_mandarin_ids.extend(mandarin_ids.iter().copied());
            profile = profile.max(*dictionary.syllable_profile.get(&form).unwrap_or(&0));
        }
    }

    let mut initial_ids = HashSet::new();
    if compact.len() == 1 && compact.chars().all(|c| c.is_ascii_alphabetic()) {
        let initial = compact.chars().next().unwrap();
        for &id in dictionary.initial_match_ids(position, initial) {
            initial_ids.insert(id as usize);
        }
    }

    let tier = if !exact_dialect_ids.is_empty() {
        if deprecated {
            SegmentTier::GanDeprecated
        } else {
            SegmentTier::GanExact
        }
    } else if !fuzzy_dialect_ids.is_empty() {
        if deprecated {
            SegmentTier::GanDeprecated
        } else {
            SegmentTier::GanCompatible
        }
    } else if !exact_mandarin_ids.is_empty() || !fuzzy_mandarin_ids.is_empty() {
        SegmentTier::Mandarin
    } else if !initial_ids.is_empty() {
        SegmentTier::Initial
    } else {
        return None;
    };

    let mut ids = HashSet::new();
    ids.extend(exact_dialect_ids.iter().copied());
    ids.extend(exact_mandarin_ids.iter().copied());
    ids.extend(fuzzy_dialect_ids);
    ids.extend(fuzzy_mandarin_ids);
    ids.extend(initial_ids);
    if ids.is_empty() {
        cache.classify_by_position.insert(cache_key, None);
        return None;
    }

    let result = Some(SyllableCandidate {
        text: compact,
        ids,
        tier,
        profile,
    });
    cache.classify_by_position.insert(cache_key, result.clone());
    result
}

fn classify_syllable_candidate_any_position(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    candidate: &str,
    cache: &mut SegmentationCache,
) -> Option<SyllableCandidate> {
    let key = candidate.trim().to_ascii_lowercase();
    if let Some(cached) = cache.classify_any_position.get(&key) {
        return cached.clone();
    }
    let mut best: Option<SyllableCandidate> = None;
    let mut position = 0usize;
    while dictionary.syllable_map_at_position(position).is_some() {
        if let Some(found) =
            classify_syllable_candidate(dictionary, fuzzy, position, candidate, cache)
        {
            let replace = match &best {
                None => true,
                Some(current) => SegmentScore::default()
                    .extend(found.tier, found.profile)
                    .better_than(&SegmentScore::default().extend(current.tier, current.profile)),
            };
            if replace {
                best = Some(found);
            } else if let Some(current) = best.as_mut() {
                current.ids.extend(found.ids);
                current.profile = current.profile.max(found.profile);
            }
        }
        position += 1;
    }
    cache.classify_any_position.insert(key, best.clone());
    best
}

fn initial_matches_entry(entry: &DictionaryEntry, position: usize, initial: char) -> bool {
    let dialect_segments = pinyin_segments(&entry.dialect_pinyin);
    let mandarin_segments = pinyin_segments(&entry.mandarin_pinyin);
    dialect_segments
        .get(position)
        .or_else(|| mandarin_segments.get(position))
        .and_then(|segment| {
            strip_tone(segment)
                .chars()
                .find(|c| c.is_ascii_alphabetic())
        })
        .map(|ch| ch.eq_ignore_ascii_case(&initial))
        .unwrap_or(false)
}

fn score_segments_for_ids(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    segments: &[SyllableCandidate],
    ids: &HashSet<usize>,
    cache: &mut SegmentationCache,
) -> SegmentScore {
    let mut score = SegmentScore::default();
    for (position, segment) in segments.iter().enumerate() {
        let deprecated = is_deprecated_gan_input(fuzzy, &segment.text);
        let mut exact_dialect = false;
        let mut fuzzy_dialect = false;
        let mut mandarin = false;
        let mut initial = false;
        let mut profile = *dictionary.syllable_profile.get(&segment.text).unwrap_or(&0);
        let fuzzy_forms = fuzzy_forms_for(fuzzy, &segment.text, cache);
        for &id in ids {
            let Some(entry) = dictionary.entries().get(id) else {
                continue;
            };
            if entry_matches_dialect_at_position(dictionary, entry, position, &segment.text) {
                exact_dialect = true;
                profile =
                    profile.max(*dictionary.syllable_profile.get(&segment.text).unwrap_or(&0));
            }
            if entry_matches_syllable_at_position(entry, position, &segment.text, false) {
                mandarin = true;
                profile =
                    profile.max(*dictionary.syllable_profile.get(&segment.text).unwrap_or(&0));
            }
            for form in &fuzzy_forms {
                if entry_matches_dialect_at_position(dictionary, entry, position, form) {
                    fuzzy_dialect = true;
                    profile = profile.max(*dictionary.syllable_profile.get(form).unwrap_or(&0));
                }
                if entry_matches_syllable_at_position(entry, position, form, false) {
                    mandarin = true;
                    profile = profile.max(*dictionary.syllable_profile.get(form).unwrap_or(&0));
                }
            }
            if segment.text.len() == 1
                && segment.text.chars().all(|c| c.is_ascii_alphabetic())
                && initial_matches_entry(entry, position, segment.text.chars().next().unwrap())
            {
                initial = true;
            }
        }
        let tier = if exact_dialect {
            if deprecated {
                SegmentTier::GanDeprecated
            } else {
                SegmentTier::GanExact
            }
        } else if fuzzy_dialect {
            if deprecated {
                SegmentTier::GanDeprecated
            } else {
                SegmentTier::GanCompatible
            }
        } else if mandarin {
            SegmentTier::Mandarin
        } else if initial {
            SegmentTier::Initial
        } else {
            segment.tier
        };
        score = score.extend(tier, profile);
    }
    score
}

fn update_best_word_segmentation(best: &mut Option<WordSegmentation>, candidate: WordSegmentation) {
    let replace = match best {
        None => true,
        Some(current) => {
            candidate.score.better_than(&current.score)
                || (candidate.score == current.score && candidate.consumed > current.consumed)
        }
    };
    if replace {
        *best = Some(candidate);
    }
}

struct WordSegmentationSearch<'a> {
    dictionary: &'a Dictionary,
    fuzzy: &'a FuzzyMap,
    input: &'a str,
    target_syllables: usize,
    require_full_consumption: bool,
    cache: &'a mut SegmentationCache,
    best: Option<WordSegmentation>,
}

impl WordSegmentationSearch<'_> {
    fn recurse(
        &mut self,
        cursor: usize,
        syllable_pos: usize,
        current_ids: Option<HashSet<usize>>,
        current_segments: &mut Vec<SyllableCandidate>,
        current_score: &SegmentScore,
    ) {
        if syllable_pos == self.target_syllables {
            if cursor == 0 || current_ids.as_ref().is_none_or(|ids| ids.is_empty()) {
                return;
            }
            if self.require_full_consumption && cursor != self.input.len() {
                return;
            }
            let ids = current_ids.unwrap_or_default();
            update_best_word_segmentation(
                &mut self.best,
                WordSegmentation {
                    consumed: cursor,
                    score: score_segments_for_ids(
                        self.dictionary,
                        self.fuzzy,
                        current_segments,
                        &ids,
                        self.cache,
                    ),
                    ids,
                    segments: current_segments.clone(),
                },
            );
            return;
        }

        let remaining = self.input.len().saturating_sub(cursor);
        let min_needed = self.target_syllables - syllable_pos;
        if remaining < min_needed {
            return;
        }
        let max_try = (remaining - (min_needed - 1)).min(6);
        for syl_len in 1..=max_try {
            let candidate = &self.input[cursor..cursor + syl_len];
            let Some(syllable) = classify_syllable_candidate(
                self.dictionary,
                self.fuzzy,
                syllable_pos,
                candidate,
                self.cache,
            ) else {
                continue;
            };
            let next_ids = match &current_ids {
                Some(ids) => ids.intersection(&syllable.ids).copied().collect(),
                None => syllable.ids.clone(),
            };
            if next_ids.is_empty() {
                continue;
            }
            current_segments.push(syllable.clone());
            let next_score = current_score.extend(syllable.tier, syllable.profile);
            self.recurse(
                cursor + syl_len,
                syllable_pos + 1,
                Some(next_ids),
                current_segments,
                &next_score,
            );
            current_segments.pop();
        }
    }
}

fn search_word_segmentation(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    target_syllables: usize,
    require_full_consumption: bool,
    cache: &mut SegmentationCache,
) -> Option<WordSegmentation> {
    let mut search = WordSegmentationSearch {
        dictionary,
        fuzzy,
        input,
        target_syllables,
        require_full_consumption,
        cache,
        best: None,
    };
    let mut segments = Vec::with_capacity(target_syllables);
    search.recurse(
        0,
        0,
        None,
        &mut segments,
        &SegmentScore::default(),
    );
    search.best
}

fn update_best_chunk_path(best: &mut Option<ChunkSegmentation>, candidate: ChunkSegmentation) {
    let replace = match best {
        None => true,
        Some(current) => {
            candidate.consumed > current.consumed
                || (candidate.consumed == current.consumed
                    && (candidate.score.better_than(&current.score)
                        || (candidate.score == current.score
                            && candidate.path.len() < current.path.len())))
        }
    };
    if replace {
        *best = Some(candidate);
    }
}

fn best_chunk_segmentation(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    cache: &mut SegmentationCache,
) -> Option<Vec<SyllableCandidate>> {
    let mut memo: HashMap<usize, Option<ChunkSegmentation>> = HashMap::new();

    fn recurse(
        dictionary: &Dictionary,
        fuzzy: &FuzzyMap,
        input: &str,
        cursor: usize,
        memo: &mut HashMap<usize, Option<ChunkSegmentation>>,
        cache: &mut SegmentationCache,
    ) -> Option<ChunkSegmentation> {
        if cursor == input.len() {
            return Some(ChunkSegmentation {
                consumed: cursor,
                score: SegmentScore::default(),
                path: Vec::new(),
            });
        }
        if let Some(cached) = memo.get(&cursor) {
            return cached.clone();
        }

        let remaining = input.len() - cursor;
        let max_try = remaining.min(6);
        let mut best: Option<ChunkSegmentation> = None;
        for syl_len in 1..=max_try {
            let candidate = &input[cursor..cursor + syl_len];
            let Some(syllable) =
                classify_syllable_candidate_any_position(dictionary, fuzzy, candidate, cache)
            else {
                continue;
            };
            let suffix = recurse(dictionary, fuzzy, input, cursor + syl_len, memo, cache);
            let (suffix_consumed, suffix_score, suffix_path) = match suffix {
                Some(found) => (found.consumed, found.score, found.path),
                None => (cursor + syl_len, SegmentScore::default(), Vec::new()),
            };
            let score = suffix_score.extend(syllable.tier, syllable.profile);
            let mut path = Vec::with_capacity(1 + suffix_path.len());
            path.push(syllable);
            path.extend(suffix_path);
            update_best_chunk_path(
                &mut best,
                ChunkSegmentation {
                    consumed: suffix_consumed,
                    score,
                    path,
                },
            );
        }
        memo.insert(cursor, best.clone());
        best
    }

    recurse(dictionary, fuzzy, input, 0, &mut memo, cache).map(|result| result.path)
}

fn complete_chunk_segmentation(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    cache: &mut SegmentationCache,
) -> Option<Vec<SyllableCandidate>> {
    best_chunk_segmentation(dictionary, fuzzy, input, cache).filter(|path| {
        path.iter()
            .map(|syllable| syllable.text.len())
            .sum::<usize>()
            == input.len()
    })
}

/// Try to segment a continuous (no-space) input into syllables, then
/// match via the per-position index. Uses greedy longest-match first
/// for each possible syllable count to avoid exponential blowup.
fn segment_and_collect_hits(
    dictionary: &Dictionary,
    fuzzy: &FuzzyMap,
    input: &str,
    direct_entry_ids: &mut HashSet<usize>,
    match_kind: &mut HashMap<usize, u8>,
) {
    let len = input.len();
    if len < 3 {
        return;
    }

    let mut cache = SegmentationCache::default();
    let mut best: Option<WordSegmentation> = None;
    // Primary search: normal syllables average >=2 chars, so bounding the
    // syllable count at (len+1)/2 keeps the search cheap for pinyin input.
    let primary_max = len.div_ceil(2).min(8);
    for n_syllables in 2..=primary_max {
        let Some(candidate) =
            search_word_segmentation(dictionary, fuzzy, input, n_syllables, true, &mut cache)
        else {
            continue;
        };
        update_best_word_segmentation(&mut best, candidate);
    }

    // 首字母联想兜底: initial-letter chunks are a single char each, so an
    // all-initials input like "nhk" needs up to `len` syllables to segment.
    // Initials are the last resort: only when every syllable route (Gan
    // exact / fuzzy / Mandarin) has failed do we deepen the search. Complete
    // Gan syllables such as "ng"(五)/"ngo"(我) stay intact because chunk
    // classification ranks GanExact/GanCompatible/Mandarin above Initial and
    // update_best_word_segmentation keeps the best-scored segmentation.
    if best.is_none() {
        for n_syllables in (primary_max + 1)..=8.min(len) {
            let Some(candidate) =
                search_word_segmentation(dictionary, fuzzy, input, n_syllables, true, &mut cache)
            else {
                continue;
            };
            update_best_word_segmentation(&mut best, candidate);
        }
    }

    if let Some(best) = best {
        for &id in &best.ids {
            if !direct_entry_ids.contains(&id) {
                direct_entry_ids.insert(id);
                // Multi-reading 等权: when every segmented syllable is a known
                // reading of the entry's character at that position, the hit
                // is Gan-exact, not fuzzy.
                let kind = match dictionary.entries().get(id) {
                    Some(entry)
                        if best.segments.iter().enumerate().all(|(position, segment)| {
                            entry_matches_dialect_at_position(
                                dictionary,
                                entry,
                                position,
                                &segment.text,
                            )
                        }) =>
                    {
                        2u8
                    }
                    _ => 0u8,
                };
                match_kind.entry(id).or_insert(kind);
            }
        }
    }
}

/// Generate fuzzy normalised forms for a syllable.
fn fuzzy_forms_for(fuzzy: &FuzzyMap, syl: &str, cache: &mut SegmentationCache) -> Vec<String> {
    if let Some(cached) = cache.fuzzy_forms.get(syl) {
        return cached.clone();
    }
    let mut forms: Vec<String> = Vec::new();
    for scheme in [SyllableScheme::GonPin, SyllableScheme::GonHan] {
        for normalized_syl in fuzzy.normalize(syl, scheme) {
            if normalized_syl.text != syl && !forms.contains(&normalized_syl.text) {
                forms.push(normalized_syl.text);
            }
        }
    }
    cache.fuzzy_forms.insert(syl.to_string(), forms.clone());
    forms
}
