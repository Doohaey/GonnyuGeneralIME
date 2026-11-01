use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyllableScheme {
    GonFuzzy,
    GonPin,
}

impl SyllableScheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyllableScheme::GonFuzzy => "gon-fuzzy",
            SyllableScheme::GonPin => "gon-pin",
        }
    }

    pub fn parse(value: &str) -> Option<SyllableScheme> {
        match value {
            "gon-fuzzy" | "gon_fuzzy" | "fuzzy" => Some(SyllableScheme::GonFuzzy),
            "gon-pin" | "gon_pin" | "pin" => Some(SyllableScheme::GonPin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum FuzzyApplies {
    SyllableInitial,
    SyllableFinal,
    #[default]
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuzzyCategory {
    Onset,
    Nucleus,
    Coda,
    Rime,
    Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PriorityTier {
    #[default]
    Primary,
    Secondary,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FuzzyConversionCondition {
    NotDerivedFromIr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyEntry {
    #[serde(default = "default_fuzzy_region")]
    pub region: String,
    pub gon_fuzzy: String,
    pub gon_pin: String,
    pub category: FuzzyCategory,
    #[serde(default)]
    pub applies: FuzzyApplies,
    #[serde(default = "default_true")]
    pub bidirectional: bool,
    #[serde(default = "default_true")]
    pub chainable: bool,
    #[serde(default)]
    pub priority_tier: PriorityTier,
    #[serde(default)]
    pub starts_with: Vec<String>,
    #[serde(default)]
    pub conversion_condition: Option<FuzzyConversionCondition>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_fuzzy_region() -> String {
    "common".to_string()
}

fn fuzzy_region_applies(entry_region: &str, region_id: Option<&str>) -> bool {
    entry_region == "common" || region_id == Some(entry_region)
}

#[derive(Debug, Clone)]
pub struct FuzzyMap {
    pub entries: Vec<FuzzyEntry>,
    gon_fuzzy_index: HashMap<char, Vec<usize>>,
    gon_pin_index: HashMap<char, Vec<usize>>,
    gon_fuzzy_empty_source: Vec<usize>,
    gon_pin_empty_source: Vec<usize>,
}

#[derive(Debug)]
pub enum SyllableError {
    Io(std::io::Error),
    Parse { line: usize, message: String },
}

impl Display for SyllableError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SyllableError::Io(error) => write!(formatter, "{error}"),
            SyllableError::Parse { line, message } => {
                write!(formatter, "fuzzy_map line {line}: {message}")
            }
        }
    }
}

impl Error for SyllableError {}

impl From<std::io::Error> for SyllableError {
    fn from(error: std::io::Error) -> Self {
        SyllableError::Io(error)
    }
}

impl FuzzyMap {
    pub fn with_optional_rules(&self, enabled: bool) -> FuzzyMap {
        Self::from_entries(
            self.entries
                .iter()
                .filter(|entry| {
                    enabled
                        || matches!(
                            (entry.gon_fuzzy.as_str(), entry.gon_pin.as_str()),
                            ("i", "ï") | ("ii", "ï") | ("e", "ê") | ("ee", "ê") | ("ir", "e")
                        )
                })
                .cloned()
                .collect(),
        )
    }

    fn from_entries(entries: Vec<FuzzyEntry>) -> FuzzyMap {
        let mut gon_fuzzy_index: HashMap<char, Vec<usize>> = HashMap::new();
        let mut gon_pin_index: HashMap<char, Vec<usize>> = HashMap::new();
        let mut gon_fuzzy_empty_source = Vec::new();
        let mut gon_pin_empty_source = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if let Some(first) = entry.gon_fuzzy.chars().next() {
                gon_fuzzy_index.entry(first).or_default().push(index);
            } else {
                gon_fuzzy_empty_source.push(index);
            }
            if entry.bidirectional {
                if let Some(first) = entry.gon_pin.chars().next() {
                    gon_pin_index.entry(first).or_default().push(index);
                } else {
                    gon_pin_empty_source.push(index);
                }
            }
        }
        FuzzyMap {
            entries,
            gon_fuzzy_index,
            gon_pin_index,
            gon_fuzzy_empty_source,
            gon_pin_empty_source,
        }
    }

    pub fn empty() -> FuzzyMap {
        Self::from_entries(Vec::new())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<FuzzyMap, SyllableError> {
        Self::load_scoped(path, None)
    }

    pub fn load_for_region(
        path: impl AsRef<Path>,
        region_id: &str,
    ) -> Result<FuzzyMap, SyllableError> {
        Self::load_scoped(path, Some(region_id))
    }

    fn load_scoped(
        path: impl AsRef<Path>,
        region_id: Option<&str>,
    ) -> Result<FuzzyMap, SyllableError> {
        let content = fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut entry: FuzzyEntry =
                serde_json::from_str(line).map_err(|error| SyllableError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            if entry.region.trim().is_empty() {
                entry.region = default_fuzzy_region();
            }
            if fuzzy_region_applies(&entry.region, region_id) {
                entries.push(entry);
            }
        }
        Ok(FuzzyMap::from_entries(entries))
    }

    pub fn load_tsv(path: impl AsRef<Path>) -> Result<FuzzyMap, SyllableError> {
        Self::load_tsv_scoped(path, None)
    }

    pub fn load_tsv_for_region(
        path: impl AsRef<Path>,
        region_id: &str,
    ) -> Result<FuzzyMap, SyllableError> {
        Self::load_tsv_scoped(path, Some(region_id))
    }

    fn load_tsv_scoped(
        path: impl AsRef<Path>,
        region_id: Option<&str>,
    ) -> Result<FuzzyMap, SyllableError> {
        let content = fs::read_to_string(path)?;
        let mut header: Option<Vec<String>> = None;
        let mut entries = Vec::new();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim_end_matches(['\r', '\n']);
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            if header.is_none() {
                header = Some(
                    columns
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect(),
                );
                continue;
            }
            let header_ref = header.as_ref().expect("header set");
            let value_for = |name: &str| -> Option<String> {
                header_ref
                    .iter()
                    .position(|column| column == name)
                    .and_then(|position| columns.get(position))
                    .map(|value| value.trim().to_string())
            };
            let parse_error = |message: String| SyllableError::Parse {
                line: index + 1,
                message,
            };
            let category = value_for("category").unwrap_or_default();
            let category = parse_category(&category)
                .ok_or_else(|| parse_error(format!("unknown category: {category}")))?;
            let applies = value_for("applies").unwrap_or_default();
            let applies = if applies.is_empty() {
                FuzzyApplies::default()
            } else {
                parse_applies(&applies)
                    .ok_or_else(|| parse_error(format!("unknown applies: {applies}")))?
            };
            let priority = value_for("priority_tier").unwrap_or_default();
            let priority_tier = if priority.is_empty() {
                PriorityTier::default()
            } else {
                parse_priority(&priority)
                    .ok_or_else(|| parse_error(format!("unknown priority_tier: {priority}")))?
            };
            let bidirectional = match value_for("bidirectional").unwrap_or_default().as_str() {
                "" => true,
                "true" => true,
                "false" => false,
                other => return Err(parse_error(format!("invalid bidirectional: {other}"))),
            };
            let chainable = match value_for("chainable").unwrap_or_default().as_str() {
                "" => true,
                "true" => true,
                "false" => false,
                other => return Err(parse_error(format!("invalid chainable: {other}"))),
            };
            let conversion_condition = match value_for("conversion_condition")
                .unwrap_or_default()
                .as_str()
            {
                "" | "none" => None,
                "not-derived-from-ir" => Some(FuzzyConversionCondition::NotDerivedFromIr),
                other => {
                    return Err(parse_error(format!(
                        "invalid conversion_condition: {other}"
                    )))
                }
            };
            if conversion_condition.is_some()
                && !matches!(
                    (
                        value_for("gon_fuzzy").as_deref(),
                        value_for("gon_pin").as_deref()
                    ),
                    (Some("e"), Some("ê")) | (Some("ee"), Some("ê"))
                )
            {
                return Err(parse_error(
                    "not-derived-from-ir only applies to e/ee→ê".to_string(),
                ));
            }
            let optional = |value: Option<String>| value.filter(|text| !text.is_empty());
            let region = value_for("region")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(default_fuzzy_region);
            let entry = FuzzyEntry {
                region,
                gon_fuzzy: value_for("gon_fuzzy").unwrap_or_default(),
                gon_pin: value_for("gon_pin").unwrap_or_default(),
                category,
                applies,
                bidirectional,
                chainable,
                priority_tier,
                starts_with: parse_csv_list(value_for("starts_with")),
                conversion_condition,
                example: optional(value_for("example")),
                note: optional(value_for("note")),
            };
            if fuzzy_region_applies(&entry.region, region_id) {
                entries.push(entry);
            }
        }
        Ok(FuzzyMap::from_entries(entries))
    }

    pub fn normalize(&self, input: &str, target: SyllableScheme) -> Vec<NormalizedSyllable> {
        let stripped = strip_tone(input);
        let mut outputs = vec![(
            NormalizedSyllable {
                text: stripped.body.clone(),
                tone: stripped.tone,
                tone_starred: stripped.tone_starred,
                tier: PriorityTier::Primary,
                applied: Vec::new(),
            },
            true,
            Vec::<usize>::new(),
        )];
        let mut produced: HashSet<(String, Vec<usize>)> =
            std::iter::once((outputs[0].0.text.clone(), Vec::new())).collect();
        let mut produced_texts: HashSet<String> =
            std::iter::once(outputs[0].0.text.clone()).collect();
        let mut cursor = 0;
        const MAX_OUTPUTS: usize = 64;
        while cursor < outputs.len() && produced_texts.len() < MAX_OUTPUTS {
            let (base, expandable, origins) = outputs[cursor].clone();
            if !expandable {
                cursor += 1;
                continue;
            }
            let (index, empty_source) = match target {
                SyllableScheme::GonPin => (&self.gon_fuzzy_index, &self.gon_fuzzy_empty_source),
                SyllableScheme::GonFuzzy => (&self.gon_pin_index, &self.gon_pin_empty_source),
            };
            let mut candidate_indices = empty_source.clone();
            for character in base.text.chars() {
                if let Some(indices) = index.get(&character) {
                    candidate_indices.extend(indices);
                }
            }
            candidate_indices.sort_unstable();
            candidate_indices.dedup();
            for index in candidate_indices {
                let entry = &self.entries[index];
                if !entry.starts_with.is_empty()
                    && !entry
                        .starts_with
                        .iter()
                        .any(|prefix| base.text.starts_with(prefix))
                {
                    continue;
                }
                let (from, to) = match target {
                    SyllableScheme::GonPin => (entry.gon_fuzzy.as_str(), entry.gon_pin.as_str()),
                    SyllableScheme::GonFuzzy => {
                        if !entry.bidirectional {
                            continue;
                        }
                        (entry.gon_pin.as_str(), entry.gon_fuzzy.as_str())
                    }
                };
                let substituted = match entry.applies {
                    FuzzyApplies::SyllableInitial => {
                        substitute_initial_matches(&base.text, from, to)
                    }
                    FuzzyApplies::SyllableFinal => substitute_final_matches(&base.text, from, to),
                    FuzzyApplies::Anywhere => {
                        substitute_any_matches(&base.text, from, to, &entry.starts_with)
                    }
                };
                for (candidate, start, source_length) in substituted {
                    if entry.conversion_condition
                        == Some(FuzzyConversionCondition::NotDerivedFromIr)
                        && origins
                            .iter()
                            .any(|position| start <= *position && *position < start + source_length)
                    {
                        continue;
                    }
                    let candidate_origins =
                        carry_ir_e_origins(&origins, start, source_length, to.len(), from, to);
                    let state = (candidate.clone(), candidate_origins.clone());
                    if produced.contains(&state)
                        || (!produced_texts.contains(&candidate)
                            && produced_texts.len() >= MAX_OUTPUTS)
                    {
                        continue;
                    }
                    let tier = lowest_tier(base.tier, entry.priority_tier);
                    let mut applied = base.applied.clone();
                    applied.push(format!("{}→{}", from, to));
                    produced.insert(state);
                    produced_texts.insert(candidate.clone());
                    outputs.push((
                        NormalizedSyllable {
                            text: candidate,
                            tone: stripped.tone,
                            tone_starred: stripped.tone_starred,
                            tier,
                            applied,
                        },
                        entry.chainable,
                        candidate_origins,
                    ));
                }
            }
            cursor += 1;
        }
        outputs.sort_by_key(|(item, _, _)| tier_rank(item.tier));
        let mut output_texts = HashSet::new();
        outputs
            .into_iter()
            .filter_map(|(item, _, _)| output_texts.insert(item.text.clone()).then_some(item))
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FuzzyEntry> {
        self.entries.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSyllable {
    pub text: String,
    pub tone: Option<u8>,
    pub tone_starred: bool,
    pub tier: PriorityTier,
    pub applied: Vec<String>,
}

struct StrippedSyllable {
    body: String,
    tone: Option<u8>,
    tone_starred: bool,
}

fn parse_csv_list(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn strip_tone(input: &str) -> StrippedSyllable {
    let trimmed = input.trim();
    let (unstarred, tone_starred) = match trimmed.strip_suffix('*') {
        Some(base) if base.ends_with(|character: char| character.is_ascii_digit()) => (base, true),
        _ => (trimmed, false),
    };
    if let Some(last) = unstarred.chars().last() {
        if let Some(digit) = last.to_digit(10) {
            if (0..=7).contains(&digit) {
                let body = unstarred[..unstarred.len() - last.len_utf8()].to_string();
                return StrippedSyllable {
                    body,
                    tone: Some(digit as u8),
                    tone_starred,
                };
            }
        }
    }
    StrippedSyllable {
        body: trimmed.to_string(),
        tone: None,
        tone_starred: false,
    }
}

fn substitute_any_matches(
    text: &str,
    from: &str,
    to: &str,
    initials: &[String],
) -> Vec<(String, usize, usize)> {
    if from.is_empty() || !text.contains(from) {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut start = 0;
    while let Some(position) = text[start..].find(from) {
        let absolute = start + position;
        if from == "u" && to == "yu" && !initials.iter().any(|initial| text[..absolute] == *initial)
        {
            start = absolute + from.len();
            continue;
        }
        let mut next = String::with_capacity(text.len() + to.len());
        next.push_str(&text[..absolute]);
        next.push_str(to);
        next.push_str(&text[absolute + from.len()..]);
        results.push((next, absolute, from.len()));
        start = absolute + from.len();
    }
    results
}

fn substitute_initial_matches(text: &str, from: &str, to: &str) -> Vec<(String, usize, usize)> {
    if !text.starts_with(from) || (to.len() > from.len() && text.starts_with(to)) {
        return Vec::new();
    }
    let mut next = String::with_capacity(text.len() + to.len());
    next.push_str(to);
    next.push_str(&text[from.len()..]);
    vec![(next, 0, from.len())]
}

fn substitute_final_matches(text: &str, from: &str, to: &str) -> Vec<(String, usize, usize)> {
    if from == "o" && to == "eo" && text != from {
        return Vec::new();
    }
    if from == "eu" && to == "ieu" && text.ends_with("ieu") {
        return Vec::new();
    }
    if from.is_empty() {
        // Append coda only if text doesn't already end with any stop coda
        if to.is_empty() || text.ends_with(['p', 't', 'k']) {
            return Vec::new();
        }
        return vec![(format!("{}{}", text, to), text.len(), 0)];
    }
    if !text.ends_with(from) {
        return Vec::new();
    }
    if from == "u" && to == "yu" {
        let stem = &text[..text.len() - from.len()];
        if stem.ends_with('y') || stem.ends_with('w') {
            return Vec::new();
        }
    }
    let mut next = String::with_capacity(text.len() + to.len());
    next.push_str(&text[..text.len() - from.len()]);
    next.push_str(to);
    vec![(next, text.len() - from.len(), from.len())]
}

fn carry_ir_e_origins(
    origins: &[usize],
    start: usize,
    source_length: usize,
    target_length: usize,
    from: &str,
    to: &str,
) -> Vec<usize> {
    let end = start + source_length;
    let shift = target_length as isize - source_length as isize;
    let mut carried = Vec::new();
    for position in origins {
        if *position < start {
            carried.push(*position);
        } else if *position >= end {
            carried.push((*position as isize + shift) as usize);
        } else {
            carried.extend(to.char_indices().map(|(offset, _)| start + offset));
        }
    }
    if from == "ir" && to == "e" {
        carried.push(start);
    }
    carried.sort_unstable();
    carried.dedup();
    carried
}

fn parse_category(value: &str) -> Option<FuzzyCategory> {
    match value {
        "onset" => Some(FuzzyCategory::Onset),
        "nucleus" => Some(FuzzyCategory::Nucleus),
        "coda" => Some(FuzzyCategory::Coda),
        "rime" => Some(FuzzyCategory::Rime),
        "tone" => Some(FuzzyCategory::Tone),
        _ => None,
    }
}

fn parse_applies(value: &str) -> Option<FuzzyApplies> {
    match value {
        "syllable-initial" => Some(FuzzyApplies::SyllableInitial),
        "syllable-final" => Some(FuzzyApplies::SyllableFinal),
        "anywhere" => Some(FuzzyApplies::Anywhere),
        _ => None,
    }
}

fn parse_priority(value: &str) -> Option<PriorityTier> {
    match value {
        "primary" => Some(PriorityTier::Primary),
        "secondary" => Some(PriorityTier::Secondary),
        "fallback" => Some(PriorityTier::Fallback),
        _ => None,
    }
}

fn tier_rank(tier: PriorityTier) -> u8 {
    match tier {
        PriorityTier::Primary => 0,
        PriorityTier::Secondary => 1,
        PriorityTier::Fallback => 2,
    }
}

fn lowest_tier(left: PriorityTier, right: PriorityTier) -> PriorityTier {
    if tier_rank(left) >= tier_rank(right) {
        left
    } else {
        right
    }
}
