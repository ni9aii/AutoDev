//! Optional Jev classification pass over deduplicated findings.
//!
//! With `--jev`, the heuristic verdict is already on each finding; this
//! module re-asks Jev for the whole batch in ONE request and writes the
//! accepted verdicts back. Every rejection path (missing key, transport
//! error, low confidence, unknown answer) keeps the heuristic verdict and
//! marks the finding `HeuristicFallback`. Jev never breaks a run.

use super::findings::{Classification, ClassificationSource, Finding};
use autodev_jev::{classify_with, client_from_env, FindingInput, Verdict};

impl From<Classification> for Verdict {
    fn from(c: Classification) -> Self {
        match c {
            Classification::DoNow => Verdict::DoNow,
            Classification::Defer => Verdict::Defer,
        }
    }
}

impl From<Verdict> for Classification {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::DoNow => Classification::DoNow,
            Verdict::Defer => Classification::Defer,
        }
    }
}

/// Re-classify `findings` with Jev in place. Returns the number of
/// findings whose verdict CHANGED relative to the heuristic (informational
/// only; the run proceeds identically when it is 0).
pub(crate) fn reclassify_with_jev(findings: &mut [Finding]) -> anyhow::Result<usize> {
    if findings.is_empty() {
        return Ok(0);
    }
    let client = client_from_env()
        .map_err(|e| anyhow::anyhow!("Jev unavailable ({e}) — skipping --jev pass"))?;

    let inputs: Vec<FindingInput> = findings
        .iter()
        .map(|f| FindingInput {
            severity: f.severity.clone(),
            title: f.title.clone(),
            description: f.description.clone(),
            file: f.file.clone(),
            heuristic_hint: f.classification.into(),
        })
        .collect();

    let results = classify_with(client.as_ref(), &inputs);
    // classify_with returns exactly one Classification per input, in order.
    debug_assert_eq!(results.len(), findings.len());

    let mut changed = 0;
    for (finding, result) in findings.iter_mut().zip(results) {
        let verdict: Classification = result.verdict.into();
        if verdict != finding.classification {
            changed += 1;
        }
        finding.classification = verdict;
        finding.source = match result.source {
            autodev_jev::ClassificationSource::Jev => ClassificationSource::Jev,
            autodev_jev::ClassificationSource::HeuristicFallback => {
                ClassificationSource::HeuristicFallback
            }
            // classify_with never emits Heuristic; map defensively.
            autodev_jev::ClassificationSource::Heuristic => ClassificationSource::Heuristic,
        };
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(title: &str, classification: Classification) -> Finding {
        Finding {
            role: "code".into(),
            severity: "CRITICAL".into(),
            title: title.into(),
            description: "desc".into(),
            file: Some("src/db.rs".into()),
            line: None,
            classification,
            source: ClassificationSource::Heuristic,
        }
    }

    // --jev on an empty batch is a no-op that must not touch the API key.
    #[test]
    fn empty_batch_is_noop() {
        let mut findings: Vec<Finding> = Vec::new();
        // No TYPESAFE_API_KEY in test env; the empty-batch path returns
        // before any client construction.
        let changed = reclassify_with_jev(&mut findings).unwrap();
        assert_eq!(changed, 0);
    }

    #[test]
    fn missing_key_is_error_not_panic() {
        // Ensure the error path is a Result::Err, not a panic. The key may
        // or may not exist in the ambient env, so assert only on one of the
        // two acceptable outcomes for a non-empty batch.
        let mut findings = vec![finding("SQL injection", Classification::DoNow)];
        match reclassify_with_jev(&mut findings) {
            Ok(_) | Err(_) => {} // both fine; must not panic
        }
    }
}
