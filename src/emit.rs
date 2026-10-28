//! Candidate emission: run the two-tier matcher + verification gate over the
//! fetched listings and serialize a candidates JSON for human/agent review.
//!
//! The output is a WORK QUEUE, not a result: every entry is a title match the
//! automated gate could NOT fully verify, paired with the exact checklist
//! items (see CHECKLIST.md) that passed or failed, so a human or agent can
//! close the gap with live evidence.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::fetch::{KalshiMarketLite, PmMarket};
use crate::matching;
use crate::normalize::normalize_title;
use crate::verify::{classify_pair, extract_resolution_source, PairFacts};

/// One candidate pair emitted for review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidatePair {
    /// "exact_unverified" (normalized-title equality, gate not fully passed)
    /// or "fuzzy" (advisory overlap match; NEVER verifiable by construction).
    pub match_kind: String,
    /// Exact tier only: the shared normalized title.
    pub normalized_title: Option<String>,
    pub pm_question: String,
    pub pm_market_id: String,
    pub kalshi_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kalshi_ticker: Option<String>,
    pub pm_cutoff: Option<String>,
    pub kalshi_cutoff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pm_resolution_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kalshi_resolution_source: Option<String>,
    /// Fuzzy tier only: token-set Jaccard of the two normalized titles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jaccard: Option<f64>,
    /// Human-readable gate rationale (which checks failed, and why).
    pub rationale: String,
}

/// The full candidates document written to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidatesDoc {
    pub generated_unix_ts: i64,
    pub pm_markets_fetched: usize,
    pub kalshi_markets_fetched: usize,
    pub exact_title_matched: usize,
    pub fuzzy_matched: usize,
    /// Always 0 in this crate's output: nothing in this document is verified.
    /// Any pair passing the FULL gate belongs in a human-audited registry,
    /// not in an automated JSON dump. (In the methodology's recorded run:
    /// 16 candidates processed, 0 verified.)
    pub verified_pairs: usize,
    pub candidates: Vec<CandidatePair>,
}

/// A venue fetch that honestly failed. Outages are data: the doc records the
/// status and the reason rather than fabricating an empty market list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueStatus {
    pub status: String, // "ok" | "unavailable"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Run discovery over already-fetched listings. Pure — no I/O — so it is
/// fully testable with synthetic fixtures.
pub fn run_matching(
    now_unix: i64,
    pm: &[PmMarket],
    kalshi: &[KalshiMarketLite],
) -> CandidatesDoc {
    // Index Kalshi by normalized title (first quote wins on collisions).
    let mut kalshi_by_norm: HashMap<String, &KalshiMarketLite> = HashMap::new();
    for k in kalshi {
        let norm = normalize_title(&k.title);
        if !norm.is_empty() {
            kalshi_by_norm.entry(norm).or_insert(k);
        }
    }
    let kalshi_norms: Vec<String> = kalshi_by_norm.keys().cloned().collect();

    let mut candidates: Vec<CandidatePair> = Vec::new();
    let mut seen_norms: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut exact_matched = 0usize;
    let mut fuzzy_matched = 0usize;

    // ---- Exact tier: normalized-title equality -> verification gate ----
    for pm_market in pm {
        let norm = normalize_title(&pm_market.question);
        if norm.is_empty() || seen_norms.contains(&norm) {
            continue;
        }
        let Some(k) = kalshi_by_norm.get(&norm) else {
            continue;
        };
        seen_norms.insert(norm.clone());
        exact_matched += 1;

        let facts = PairFacts {
            pm_cutoff_raw: pm_market.end_date.clone(),
            kalshi_cutoff_raw: k.close_time.clone(),
            pm_source_text: pm_market.description.clone(),
            kalshi_source_text: k.rules_text.clone(),
        };
        let verdict = classify_pair(&facts);
        // NOTE: by construction this crate never emits `verified: true` —
        // a pair that passes the gate is escalated to human audit instead.
        candidates.push(CandidatePair {
            match_kind: "exact_unverified".to_string(),
            normalized_title: Some(norm),
            pm_question: pm_market.question.clone(),
            pm_market_id: pm_market.id.clone(),
            kalshi_title: k.title.clone(),
            kalshi_ticker: k.ticker.clone(),
            pm_cutoff: facts.pm_cutoff_raw,
            kalshi_cutoff: facts.kalshi_cutoff_raw,
            pm_resolution_source: verdict.pm_source,
            kalshi_resolution_source: verdict.kalshi_source,
            jaccard: None,
            rationale: if verdict.verified {
                "PASSED automated gate — requires HUMAN verification before use \
                 (live source-divergence checks, CHECKLIST.md). Not counted as verified here."
                    .to_string()
            } else {
                verdict.rationale
            },
        });
    }

    // ---- Fuzzy tier: advisory overlap candidates only ----
    for pm_market in pm {
        let pm_norm = normalize_title(&pm_market.question);
        if pm_norm.is_empty() || seen_norms.contains(&pm_norm) {
            continue;
        }
        let Some((k_norm, j)) = matching::best_fuzzy_match(&pm_norm, kalshi_norms.iter().map(|s| s.as_str()))
        else {
            continue;
        };
        let k = kalshi_by_norm
            .get(k_norm)
            .expect("index contains every returned norm");
        seen_norms.insert(pm_norm.clone());
        fuzzy_matched += 1;
        candidates.push(CandidatePair {
            match_kind: "fuzzy".to_string(),
            normalized_title: None,
            pm_question: pm_market.question.clone(),
            pm_market_id: pm_market.id.clone(),
            kalshi_title: k.title.clone(),
            kalshi_ticker: k.ticker.clone(),
            pm_cutoff: pm_market.end_date.clone(),
            kalshi_cutoff: k.close_time.clone(),
            // Honest None: no PM description fetch for fuzzy pairs (request
            // budget), so no PM source claim is recorded.
            pm_resolution_source: None,
            kalshi_resolution_source: k
                .rules_text
                .as_deref()
                .and_then(extract_resolution_source)
                .map(str::to_string),
            jaccard: Some((j * 100.0).round() / 100.0),
            rationale: format!(
                "fuzzy token-overlap match (jaccard={j:.2}); NEVER verifiable by \
                 construction — fuzzy matches only seed human review"
            ),
        });
    }

    CandidatesDoc {
        generated_unix_ts: now_unix,
        pm_markets_fetched: pm.len(),
        kalshi_markets_fetched: kalshi.len(),
        exact_title_matched: exact_matched,
        fuzzy_matched,
        verified_pairs: 0,
        candidates,
    }
}

#[cfg(test)]
mod tests {
    // FIXTURE NOTE: the entire input below is a labeled SYNTHETIC fixture.
    // All names ("FAKE-EVENT-A", "FAKE-PERSON-A", "FIXTURE-*") are invented
    // placeholders; no real market title, ticker, id, price, or rules text
    // appears in any test in this crate. See DATA.md.

    use super::*;

    fn fixture_pm(id: &str, question: &str) -> PmMarket {
        PmMarket {
            id: id.to_string(),
            question: question.to_string(),
            end_date: Some("2030-01-15T12:00:00Z".to_string()),
            description: Some("Resolves per Reuters. FIXTURE text only.".to_string()),
        }
    }

    fn fixture_kalshi(ticker: &str, title: &str) -> KalshiMarketLite {
        KalshiMarketLite {
            ticker: Some(ticker.to_string()),
            title: title.to_string(),
            close_time: Some("2030-01-15T09:30:00Z".to_string()),
            rules_text: Some("Resolves per official FAKE-AGENCY announcements; secondary: reuters.".to_string()),
        }
    }

    #[test]
    fn exact_fixture_pair_passes_automated_gate_but_is_not_verified() {
        let pm = vec![fixture_pm(
            "FIXTURE-PM-1",
            "Will FAKE-EVENT-A be approved?",
        )];
        let kalshi = vec![fixture_kalshi(
            "FIXTURE-K-1",
            "will fake event a be approved?",
        )];
        let doc = run_matching(1_700_000_000, &pm, &kalshi);
        assert_eq!(doc.exact_title_matched, 1);
        assert_eq!(doc.verified_pairs, 0, "this crate never certifies verification");
        let c = &doc.candidates[0];
        assert_eq!(c.match_kind, "exact_unverified");
        assert!(c.rationale.contains("HUMAN verification"));
        assert_eq!(c.pm_resolution_source.as_deref(), Some("Reuters"));
    }

    #[test]
    fn fixture_gate_failure_becomes_rationale() {
        let mut pm = vec![fixture_pm(
            "FIXTURE-PM-2",
            "Will FAKE-EVENT-B be approved?",
        )];
        pm[0].end_date = Some("2030-02-20T12:00:00Z".to_string()); // different day
        let kalshi = vec![fixture_kalshi(
            "FIXTURE-K-2",
            "will fake event b be approved?",
        )];
        let doc = run_matching(1_700_000_000, &pm, &kalshi);
        let c = &doc.candidates[0];
        assert_eq!(c.match_kind, "exact_unverified");
        assert!(c.rationale.contains("cutoff days differ"), "{}", c.rationale);
    }

    #[test]
    fn fuzzy_fixture_overlap_is_advisory_only() {
        let pm = vec![fixture_pm(
            "FIXTURE-PM-3",
            "Will FAKE-PERSON-A win the FAKE-TROPHY-1 contest?",
        )];
        let kalshi = vec![fixture_kalshi(
            "FIXTURE-K-3",
            "Will FAKE-PERSON-A win the FAKE-TROPHY-1 contest this season?",
        )];
        let doc = run_matching(1_700_000_000, &pm, &kalshi);
        assert_eq!(doc.exact_title_matched, 0);
        assert_eq!(doc.fuzzy_matched, 1);
        let c = &doc.candidates[0];
        assert_eq!(c.match_kind, "fuzzy");
        assert!(c.jaccard.unwrap_or(0.0) > 0.7);
        assert!(c.rationale.contains("NEVER verifiable"));
    }

    #[test]
    fn serializes_to_json_round_trip() {
        let pm = vec![fixture_pm("FIXTURE-PM-4", "Will FAKE-EVENT-C happen?")];
        let kalshi = vec![fixture_kalshi("FIXTURE-K-4", "will fake event c happen?")];
        let doc = run_matching(42, &pm, &kalshi);
        let json = serde_json::to_string(&doc).expect("serialize");
        let back: CandidatesDoc = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, doc);
        assert!(json.contains("FAKE-EVENT-C"));
    }

    #[test]
    fn disjoint_fixture_titles_produce_no_candidates() {
        let pm = vec![fixture_pm("FIXTURE-PM-5", "Will FAKE-THING-X happen?")];
        let kalshi = vec![fixture_kalshi("FIXTURE-K-5", "totally unrelated fixture title")];
        let doc = run_matching(0, &pm, &kalshi);
        assert!(doc.candidates.is_empty());
        assert_eq!(doc.verified_pairs, 0);
    }
}
