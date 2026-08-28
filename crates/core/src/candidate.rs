use std::collections::HashSet;

/// The common, behavior-neutral part of every candidate stage.
///
/// Stage-specific metadata deliberately stays on `ComposedCandidate` and
/// `RankedCandidate`; this view only supports operations whose semantics are
/// shared by both pipelines.
pub(crate) trait CandidateView {
    fn text(&self) -> &str;
    fn reading(&self) -> Option<&str>;
}

pub(crate) fn text_set<T: CandidateView>(candidates: &[T]) -> HashSet<&str> {
    candidates.iter().map(CandidateView::text).collect()
}

pub(crate) fn owned_text_set<T: CandidateView>(candidates: &[T]) -> HashSet<String> {
    candidates
        .iter()
        .map(|candidate| candidate.text().to_owned())
        .collect()
}

/// Keep the first candidate for each composed-candidate identity.
///
/// A reading is part of the identity here because two readings of the same
/// text are distinct composition results. Retrieval intentionally continues
/// to deduplicate by text at its existing insertion points.
pub(crate) fn retain_unique_text_and_reading<T: CandidateView>(candidates: &mut Vec<T>) {
    let mut seen = HashSet::<(String, Option<String>)>::new();
    candidates.retain(|candidate| {
        seen.insert((
            candidate.text().to_owned(),
            candidate.reading().map(str::to_owned),
        ))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Candidate {
        text: &'static str,
        reading: Option<&'static str>,
    }

    impl CandidateView for Candidate {
        fn text(&self) -> &str {
            self.text
        }

        fn reading(&self) -> Option<&str> {
            self.reading
        }
    }

    #[test]
    fn composed_identity_keeps_distinct_readings_and_first_duplicate() {
        let mut candidates = vec![
            Candidate {
                text: "公",
                reading: Some("gung1"),
            },
            Candidate {
                text: "公",
                reading: Some("gung1"),
            },
            Candidate {
                text: "公",
                reading: Some("gung0"),
            },
        ];

        retain_unique_text_and_reading(&mut candidates);

        assert_eq!(
            candidates,
            vec![
                Candidate {
                    text: "公",
                    reading: Some("gung1"),
                },
                Candidate {
                    text: "公",
                    reading: Some("gung0"),
                },
            ]
        );
    }
}
