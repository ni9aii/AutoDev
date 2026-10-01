//! Optional Jev (TypeSafe System One) classification layer for AutoDev.
//!
//! Wraps the `kunobi-jev` blocking client behind a small typed API:
//! [`classify_batch`] sends all findings in ONE System One request (the
//! state is charged once regardless of question count) and maps answers
//! back by name. Every failure mode — network error, timeout, missing
//! answer, low confidence — degrades to
//! [`ClassificationSource::HeuristicFallback`] instead of propagating:
//! Jev must never break a pipeline run.

use kunobi_jev::blocking::Client;
use kunobi_jev::{choice, Questions, SystemOneRequest};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Triage decision for a review finding. Mirrors review-aggregator's
/// `Classification` enum (serde names identical for plan.json output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Fix immediately.
    #[serde(rename = "do_now")]
    DoNow,
    /// Defer to a later pass.
    #[serde(rename = "defer")]
    Defer,
}

/// Who produced the final classification for a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationSource {
    /// Heuristic decided (no --jev, or Jev was not consulted).
    #[serde(rename = "heuristic")]
    Heuristic,
    /// Jev verdict accepted (confidence at or above the gate).
    #[serde(rename = "jev")]
    Jev,
    /// Jev was consulted but its answer was rejected (error, timeout,
    /// confidence below gate) — heuristic verdict kept.
    #[serde(rename = "heuristic_fallback")]
    HeuristicFallback,
}

/// One finding as seen by the classifier.
#[derive(Debug, Clone, Serialize)]
pub struct FindingInput {
    pub severity: String,
    pub title: String,
    pub description: String,
    pub file: Option<String>,
    /// The heuristic's verdict, fed to Jev as a hint it may overturn.
    pub heuristic_hint: Verdict,
}

/// Result for one finding.
#[derive(Debug, Clone)]
pub struct Classification {
    pub verdict: Verdict,
    pub source: ClassificationSource,
    /// Jev confidence for the winning label; `None` when Jev was not
    /// consulted or the answer was discarded.
    pub confidence: Option<f64>,
}

/// Minimum Choice confidence for a Jev verdict to be accepted.
pub const CONFIDENCE_GATE: f64 = 0.6;

const CHOICE_ID: &str = "triage";

/// Any `SystemOne` implementation: the real blocking client in production,
/// `FakeSystemOne` in tests. Erased so [`classify_with`] has one shape.
pub trait AskJev: Send + Sync {
    fn ask(&self, request: SystemOneRequest) -> Result<kunobi_jev::SystemOneResult, String>;
}

impl AskJev for Client {
    fn ask(&self, request: SystemOneRequest) -> Result<kunobi_jev::SystemOneResult, String> {
        self.system_one(request).map_err(|e| e.to_string())
    }
}

/// Build a production client from the environment. `TYPESAFE_API_KEY` is
/// required; `JEV_BASE_URL` (optional) overrides the API endpoint — set it
/// to `https://openrouter.ai/api` to use the OpenRouter-hosted SystemOne
/// endpoint with an OpenRouter key. Returns `Err` when the key is missing —
/// callers should treat that as "run without --jev", not as a fatal error.
pub fn client_from_env() -> Result<Arc<dyn AskJev>, String> {
    let builder = Client::builder();
    let client = match std::env::var("JEV_BASE_URL") {
        Ok(url) if !url.is_empty() => builder
            .configure(|b| b.base_url(url))
            .build()
            .map_err(|e| e.to_string())?,
        _ => builder.build().map_err(|e| e.to_string())?,
    };
    Ok(Arc::new(client))
}

/// Classify a batch of findings with Jev, falling back to the heuristic
/// hint for any finding Jev failed to answer confidently. `client` comes
/// from [`client_from_env`] (production) or a `FakeSystemOne` wrapper
/// (tests).
pub fn classify_with(client: &dyn AskJev, findings: &[FindingInput]) -> Vec<Classification> {
    if findings.is_empty() {
        return Vec::new();
    }

    match ask_batch(client, findings) {
        Ok(results) => results,
        // Whole-request failure: every finding falls back to the heuristic.
        Err(e) => {
            eprintln!("autodev-jev: batch request failed ({e}); using heuristic classifications");
            findings
                .iter()
                .map(|f| Classification {
                    verdict: f.heuristic_hint,
                    source: ClassificationSource::HeuristicFallback,
                    confidence: None,
                })
                .collect()
        }
    }
}

fn truncate_desc(desc: &str, max: usize) -> String {
    let compact = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max {
        compact
    } else {
        let cut: String = compact.chars().take(max).collect();
        format!("{cut}...")
    }
}

fn ask_batch(
    client: &dyn AskJev,
    findings: &[FindingInput],
) -> Result<Vec<Classification>, String> {
    // State: one JSON array with every finding and its heuristic hint.
    let state = kunobi_jev::Entry::from_serialize(&findings)
        .map_err(|e| format!("could not serialize findings: {e}"))?;

    // One Choice question per finding, all riding the same request: the
    // state is sent once no matter how many questions ride along. Each
    // question is SELF-CONTAINED (finding data quoted in the text): asking
    // the model to map "finding #N" to an array position in the state
    // misaligns answers in multi-question requests (observed live:
    // answers returned for the wrong findings). The heuristic_hint stays
    // in the question so Jev can disagree with it explicitly.
    let mut questions = Questions::new();
    for (i, f) in findings.iter().enumerate() {
        let file_part = f
            .file
            .as_deref()
            .map(|f| format!(" File: {f}."))
            .unwrap_or_default();
        let question = choice(
            format!(
                "Review finding: [{}] {}.{} {} Should this be fixed now (do_now) or deferred (defer)? \
                 do_now = a real defect that matters: ignoring it causes incorrect behavior, crashes, \\
                 data loss or a security hole. \
                 defer = cosmetic or trivial changes (typos, wording, naming) REGARDLESS of the stated \\
                 severity, refactoring/architecture suggestions, speculative or invasive work. \\
                 Severity labels in reviews are often miscalibrated - judge by the consequence \\
                 described, not by the label. \
                 The heuristic previously classified it as {:?}; disagree when the description warrants it.",
                f.severity,
                f.title,
                file_part,
                truncate_desc(&f.description, 300),
                match f.heuristic_hint {
                    Verdict::DoNow => "do_now",
                    Verdict::Defer => "defer",
                }
            ),
            [
                ("do_now", "real defect that matters, fix now"),
                ("defer", "cosmetic/trivial or invasive, defer"),
            ],
        );
        questions.add(format!("{CHOICE_ID}_{i}"), question);
    }

    let result = client.ask(SystemOneRequest::new(state, questions))?;

    let mut out = Vec::with_capacity(findings.len());
    for (i, finding) in findings.iter().enumerate() {
        let answer = result.answers.get(&format!("{CHOICE_ID}_{i}"));
        let classification = match answer {
            Some(kunobi_jev::Answer::Choice(c)) => {
                let verdict = match c.choice.as_str() {
                    "do_now" => Verdict::DoNow,
                    _ => Verdict::Defer,
                };
                if c.confidence >= CONFIDENCE_GATE {
                    Classification {
                        verdict,
                        source: ClassificationSource::Jev,
                        confidence: Some(c.confidence),
                    }
                } else {
                    Classification {
                        verdict: finding.heuristic_hint,
                        source: ClassificationSource::HeuristicFallback,
                        confidence: Some(c.confidence),
                    }
                }
            }
            // Wrong answer type or missing answer: fall back per finding.
            _ => Classification {
                verdict: finding.heuristic_hint,
                source: ClassificationSource::HeuristicFallback,
                confidence: None,
            },
        };
        out.push(classification);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kunobi_jev::testing::FakeSystemOne;
    use std::sync::Mutex;

    struct FakeAsk {
        inner: FakeSystemOne,
    }

    impl AskJev for FakeAsk {
        fn ask(&self, request: SystemOneRequest) -> Result<kunobi_jev::SystemOneResult, String> {
            // FakeSystemOne implements the async SystemOne trait; drive the
            // future on a tiny runtime.
            tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(kunobi_jev::SystemOne::ask(&self.inner, request))
                .map_err(|e| e.to_string())
        }
    }

    fn scripted(labels: Vec<(String, String, f64)>) -> FakeAsk {
        let mut f = FakeSystemOne::new();
        for (name, label, confidence) in labels {
            f = f.choice(name, label, confidence);
        }
        FakeAsk { inner: f }
    }

    fn finding(title: &str, hint: Verdict) -> FindingInput {
        FindingInput {
            severity: "CRITICAL".into(),
            title: title.into(),
            description: "SQL injection in query".into(),
            file: Some("src/db.rs".into()),
            heuristic_hint: hint,
        }
    }

    #[test]
    fn jev_agreement_is_accepted() {
        let fake = scripted(vec![("triage_0".into(), "do_now".into(), 0.9)]);
        let findings = vec![finding("SQL injection", Verdict::DoNow)];
        let out = classify_with(&fake, &findings);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].verdict, Verdict::DoNow);
        assert_eq!(out[0].source, ClassificationSource::Jev);
        assert_eq!(out[0].confidence, Some(0.9));
    }

    #[test]
    fn jev_disagreement_is_accepted_when_confident() {
        let fake = scripted(vec![("triage_0".into(), "defer".into(), 0.85)]);
        let findings = vec![finding("needs refactor", Verdict::DoNow)];
        let out = classify_with(&fake, &findings);
        assert_eq!(out[0].verdict, Verdict::Defer);
        assert_eq!(out[0].source, ClassificationSource::Jev);
    }

    #[test]
    fn low_confidence_falls_back_to_heuristic() {
        let fake = scripted(vec![("triage_0".into(), "defer".into(), 0.4)]);
        let findings = vec![finding("SQL injection", Verdict::DoNow)];
        let out = classify_with(&fake, &findings);
        assert_eq!(out[0].verdict, Verdict::DoNow);
        assert_eq!(out[0].source, ClassificationSource::HeuristicFallback);
        assert_eq!(out[0].confidence, Some(0.4));
    }

    #[test]
    fn missing_answer_falls_back_per_finding() {
        // A real server may omit answers; FakeSystemOne refuses the whole
        // request instead, so this scenario is covered by the partial-
        // answer path in ask_batch (wrong-type branch) — exercised here via
        // a raw unknown answer for triage_1.
        use kunobi_jev::Answer;
        let base = FakeSystemOne::new()
            .choice("triage_0", "do_now", 0.9)
            .answer(
                "triage_1",
                Answer::Unknown(serde_json::json!({"type": "mystery"})),
            );
        let fake = FakeAsk { inner: base };
        let findings = vec![finding("a", Verdict::Defer), finding("b", Verdict::DoNow)];
        let out = classify_with(&fake, &findings);
        assert_eq!(out[0].source, ClassificationSource::Jev);
        assert_eq!(out[1].source, ClassificationSource::HeuristicFallback);
        assert_eq!(out[1].verdict, Verdict::DoNow);
        assert_eq!(out[1].confidence, None);
    }

    #[test]
    fn request_error_falls_back_for_all() {
        struct Broken;
        impl AskJev for Broken {
            fn ask(
                &self,
                _request: SystemOneRequest,
            ) -> Result<kunobi_jev::SystemOneResult, String> {
                Err("connection refused".into())
            }
        }
        let findings = vec![finding("a", Verdict::DoNow), finding("b", Verdict::Defer)];
        let out = classify_with(&Broken, &findings);
        assert_eq!(out.len(), 2);
        assert!(out
            .iter()
            .all(|c| c.source == ClassificationSource::HeuristicFallback));
        assert_eq!(out[0].verdict, Verdict::DoNow);
        assert_eq!(out[1].verdict, Verdict::Defer);
    }

    #[test]
    fn empty_batch_is_empty() {
        let fake = scripted(vec![]);
        let out = classify_with(&fake, &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn all_findings_ride_one_request() {
        let fake = scripted(vec![
            ("triage_0".into(), "do_now".into(), 0.9),
            ("triage_1".into(), "defer".into(), 0.9),
            ("triage_2".into(), "do_now".into(), 0.9),
        ]);
        let findings: Vec<_> = (0..3)
            .map(|i| finding(&format!("f{i}"), Verdict::Defer))
            .collect();
        let out = classify_with(&fake, &findings);
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].verdict, Verdict::Defer);
        assert_eq!(out[2].verdict, Verdict::DoNow);
    }

    // Keep Mutex import used if the fake wrapper grows shared state.
    #[allow(dead_code)]
    type Unused = Mutex<()>;
}
