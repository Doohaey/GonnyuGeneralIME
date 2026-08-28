mod association_cache;
mod cache_obfuscation;
mod candidate;
mod dictionary;
mod mandarin_hints;
mod pipeline;
mod pronunciation;
mod resources;
mod retrieval;
mod slang;
mod syllable;
mod trie;
pub use trie::Trie;
mod user_dict;

pub use dictionary::{Dictionary, DictionaryEntry, DictionaryError};
pub use mandarin_hints::{MandarinHintBook, MandarinHintEntry, MandarinHintError};
pub use pipeline::{
    CandidateSource, CandidateTier, ComposedCandidate, InputPipeline, PipelineError,
};
pub use pronunciation::{
    PronunciationBook, PronunciationEntry, PronunciationError, Reading, Register,
    RegisterAlternate, RegisterCorrection,
};
pub use resources::{
    default_region_entry, list_region_entries, load_manifest, load_region_from_manifest,
    DictionaryFiles, LanguageFiles, Manifest, PhonologyFiles, RegionConfig, RegionEntry,
    RegionMetadata, RegionResource, ResourceError, ToneClass,
};
pub use retrieval::{
    format_preedit_display, retrieve, retrieve_sentence_input, retrieve_with_manual_segments,
    segment_boundaries, segment_sentence, RankedCandidate, RetrievalLayer,
};
pub use slang::{
    AssociationEntry, AssociationHit, AssociationSuggestion, ReverseHit, SlangBook, SlangEntry,
    SlangError, SlangHit, SlangTrigger, TriggerKind,
};
pub use syllable::{
    FuzzyApplies, FuzzyCategory, FuzzyEntry, FuzzyMap, NormalizedSyllable, PriorityTier,
    SyllableError, SyllableScheme,
};
