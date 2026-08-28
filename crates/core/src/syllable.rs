use serde::Deserialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyllableScheme {
    GonHan,
    GonPin,
}

impl SyllableScheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyllableScheme::GonHan => "gon-han",
            SyllableScheme::GonPin => "gon-pin",
        }
    }

    pub fn parse(value: &str) -> Option<SyllableScheme> {
        match value {
            "gon-han" | "gon_han" | "han" => Some(SyllableScheme::GonHan),
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

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyEntry {
    pub gon_han: String,
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
    pub example: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct FuzzyMap {
    pub entries: Vec<FuzzyEntry>,
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
    pub fn load(path: impl AsRef<Path>) -> Result<FuzzyMap, SyllableError> {
        let content = fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: FuzzyEntry =
                serde_json::from_str(line).map_err(|error| SyllableError::Parse {
                    line: index + 1,
                    message: error.to_string(),
                })?;
            entries.push(entry);
        }
        Ok(FuzzyMap { entries })
    }

    pub fn load_tsv(path: impl AsRef<Path>) -> Result<FuzzyMap, SyllableError> {
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
            let optional = |value: Option<String>| value.filter(|text| !text.is_empty());
            entries.push(FuzzyEntry {
                gon_han: value_for("gon_han").unwrap_or_default(),
                gon_pin: value_for("gon_pin").unwrap_or_default(),
                category,
                applies,
                bidirectional,
                chainable,
                priority_tier,
                starts_with: parse_csv_list(value_for("starts_with")),
                example: optional(value_for("example")),
                note: optional(value_for("note")),
            });
        }
        Ok(FuzzyMap { entries })
    }

    pub fn normalize(&self, input: &str, target: SyllableScheme) -> Vec<NormalizedSyllable> {
        let stripped = strip_tone(input);
        let mut outputs = vec![(
            NormalizedSyllable {
                text: stripped.body.clone(),
                tone: stripped.tone,
                tier: PriorityTier::Primary,
                applied: Vec::new(),
            },
            true,
        )];
        let mut produced: HashSet<String> = std::iter::once(outputs[0].0.text.clone()).collect();
        let mut cursor = 0;
        const MAX_OUTPUTS: usize = 64;
        while cursor < outputs.len() && outputs.len() < MAX_OUTPUTS {
            let (base, expandable) = outputs[cursor].clone();
            if !expandable {
                cursor += 1;
                continue;
            }
            for entry in &self.entries {
                if !entry.starts_with.is_empty()
                    && !entry
                        .starts_with
                        .iter()
                        .any(|prefix| base.text.starts_with(prefix))
                {
                    continue;
                }
                let (from, to) = match target {
                    SyllableScheme::GonPin => (entry.gon_han.as_str(), entry.gon_pin.as_str()),
                    SyllableScheme::GonHan => {
                        if !entry.bidirectional {
                            continue;
                        }
                        (entry.gon_pin.as_str(), entry.gon_han.as_str())
                    }
                };
                let substituted = match entry.applies {
                    FuzzyApplies::SyllableInitial => substitute_initial(&base.text, from, to),
                    FuzzyApplies::SyllableFinal => substitute_final(&base.text, from, to),
                    FuzzyApplies::Anywhere => substitute_any(&base.text, from, to),
                };
                for candidate in substituted {
                    if produced.contains(&candidate) {
                        continue;
                    }
                    let tier = lowest_tier(base.tier, entry.priority_tier);
                    let mut applied = base.applied.clone();
                    applied.push(format!("{}→{}", from, to));
                    produced.insert(candidate.clone());
                    outputs.push((
                        NormalizedSyllable {
                            text: candidate,
                            tone: stripped.tone,
                            tier,
                            applied,
                        },
                        entry.chainable,
                    ));
                }
            }
            cursor += 1;
        }
        outputs.sort_by_key(|(item, _)| tier_rank(item.tier));
        outputs.into_iter().map(|(item, _)| item).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FuzzyEntry> {
        self.entries.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSyllable {
    pub text: String,
    pub tone: Option<u8>,
    pub tier: PriorityTier,
    pub applied: Vec<String>,
}

struct StrippedSyllable {
    body: String,
    tone: Option<u8>,
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
    if let Some(last) = trimmed.chars().last() {
        if let Some(digit) = last.to_digit(10) {
            if (1..=7).contains(&digit) {
                let body = trimmed[..trimmed.len() - last.len_utf8()].to_string();
                return StrippedSyllable {
                    body,
                    tone: Some(digit as u8),
                };
            }
        }
    }
    StrippedSyllable {
        body: trimmed.to_string(),
        tone: None,
    }
}

fn substitute_any(text: &str, from: &str, to: &str) -> Vec<String> {
    if from.is_empty() || !text.contains(from) {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut start = 0;
    while let Some(position) = text[start..].find(from) {
        let absolute = start + position;
        if from == "y" && to == "yu" {
            let suffix = &text[absolute + from.len()..];
            if suffix.starts_with('u') {
                start = absolute + from.len();
                continue;
            }
        }
        let mut next = String::with_capacity(text.len() + to.len());
        next.push_str(&text[..absolute]);
        next.push_str(to);
        next.push_str(&text[absolute + from.len()..]);
        results.push(next);
        start = absolute + from.len();
    }
    results
}

fn substitute_initial(text: &str, from: &str, to: &str) -> Vec<String> {
    if !text.starts_with(from) {
        return Vec::new();
    }
    let mut next = String::with_capacity(text.len() + to.len());
    next.push_str(to);
    next.push_str(&text[from.len()..]);
    vec![next]
}

fn substitute_final(text: &str, from: &str, to: &str) -> Vec<String> {
    if from.is_empty() {
        // Append coda only if text doesn't already end with any stop coda
        if to.is_empty() || text.ends_with('t') || text.ends_with('k') {
            return Vec::new();
        }
        return vec![format!("{}{}", text, to)];
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
    vec![next]
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
