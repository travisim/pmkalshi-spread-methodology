//! Two-tier candidate matching (METHODOLOGY.md §4):
//!
//! - **Exact tier**: normalized-title equality. The ONLY tier that may
//!   proceed to the verification gate.
//! - **Fuzzy tier**: token-set Jaccard >= `FUZZY_JACCARD_MIN` with at least
//!   `FUZZY_SHARED_MIN` shared normalized words. Advisory only — a fuzzy
//!   match can NEVER verify by construction.

use std::collections::HashSet;

/// Fuzzy-tier admission: token-Jaccard threshold on normalized titles.
pub const FUZZY_JACCARD_MIN: f64 = 0.70;
/// Fuzzy-tier admission: minimum number of shared normalized words.
pub const FUZZY_SHARED_MIN: usize = 4;

fn token_set(norm: &str) -> HashSet<&str> {
    norm.split_whitespace().collect()
}

/// Token-set Jaccard similarity of two normalized titles.
pub fn token_jaccard(a: &str, b: &str) -> f64 {
    let sa = token_set(a);
    let sb = token_set(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count();
    let union = sa.len() + sb.len() - inter;
    inter as f64 / union as f64
}

/// Number of shared normalized words (guard against tiny-title Jaccard
/// inflation, where 2/3 shared words of a 3-word title would pass).
pub fn shared_words(a: &str, b: &str) -> usize {
    token_set(a).intersection(&token_set(b)).count()
}

/// Is `(pm_norm, kalshi_norm)` admissible as a fuzzy candidate?
pub fn fuzzy_admissible(pm_norm: &str, kalshi_norm: &str) -> bool {
    let j = token_jaccard(pm_norm, kalshi_norm);
    j >= FUZZY_JACCARD_MIN && shared_words(pm_norm, kalshi_norm) >= FUZZY_SHARED_MIN
}

/// Fuzzy candidates for one normalized PM title: the best-scoring (highest
/// Jaccard) admissible Kalshi title, if any. The caller supplies the index of
/// Kalshi normalized titles; ties resolve to the first-seen title.
pub fn best_fuzzy_match<'a>(
    pm_norm: &str,
    kalshi_norms: impl Iterator<Item = &'a str>,
) -> Option<(&'a str, f64)> {
    let mut best: Option<(f64, &str)> = None;
    for k in kalshi_norms {
        if k == pm_norm {
            continue; // exact tier handles these
        }
        if !fuzzy_admissible(pm_norm, k) {
            continue;
        }
        let j = token_jaccard(pm_norm, k);
        if best.map_or(true, |(bj, _)| j > bj) {
            best = Some((j, k));
        }
    }
    best.map(|(j, k)| (k, j))
}

#[cfg(test)]
mod tests {
    // FIXTURE NOTE: all titles below are synthetic placeholders
    // ("FAKE-..."). No real market data. See DATA.md.

    use super::*;

    #[test]
    fn jaccard_identical_and_disjoint() {
        assert_eq!(token_jaccard("a b c", "a b c"), 1.0);
        assert_eq!(token_jaccard("a b c", "x y z"), 0.0);
        assert_eq!(token_jaccard("", "a b"), 0.0);
    }

    #[test]
    fn shared_words_counts_intersection() {
        assert_eq!(shared_words("a b c d", "b c d e"), 3);
        assert_eq!(shared_words("a", "a"), 1);
    }

    #[test]
    fn fuzzy_gate_admits_strong_overlap_only() {
        // 4 shared of 5/5 union words = 0.8 Jaccard, 4 shared words -> admit.
        assert!(fuzzy_admissible(
            "will fake person a win fake event",
            "will fake person a win fake event soon"
        ));
        // High overlap but only 3 shared words -> reject (shared-min guard).
        assert!(!fuzzy_admissible("fake person a wins", "fake person a loses"));
        // Different shared-min AND jaccard: 2 of 5 union -> reject.
        assert!(!fuzzy_admissible("fake person a wins everything", "fake person b loses nothing"));
        // DOCUMENTED LENIENCY: a one-word difference on long titles passes
        // the fuzzy SCREEN (7/9 jaccard, 7 shared words). This is exactly
        // why a fuzzy match can never verify — the screen is advisory.
        assert!(fuzzy_admissible(
            "fake person a wins championship of league x",
            "fake person b wins championship of league x"
        ));
    }

    #[test]
    fn best_fuzzy_match_picks_highest_jaccard() {
        let kalshi = [
            "fake person a wins fake trophy one today or tomorrow", // 6/9 < 0.7 -> rejected
            "fake person a wins fake trophy one now",               // 6/8 = 0.75 -> best
        ];
        let (k, j) = best_fuzzy_match("fake person a wins fake trophy one", kalshi.iter().copied())
            .expect("expected an admissible match");
        assert_eq!(k, "fake person a wins fake trophy one now");
        assert!(j > 0.7);
    }

    #[test]
    fn best_fuzzy_match_skips_exact_title_for_fuzzy_tier() {
        // The identical title is the EXACT tier's job; the fuzzy tier skips it.
        let kalshi = ["fake person a wins fake trophy one"];
        assert!(best_fuzzy_match("fake person a wins fake trophy one", kalshi.iter().copied()).is_none());
    }

    #[test]
    fn best_fuzzy_match_none_when_disjoint() {
        let kalshi = ["totally unrelated fake title here"];
        assert!(best_fuzzy_match("fake person a wins fake trophy one", kalshi.iter().copied()).is_none());
    }
}
