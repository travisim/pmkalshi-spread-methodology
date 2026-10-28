//! Title normalization ("matchnorm"): the deterministic text canonicalization
//! both matching tiers operate on.
//!
//! Design rules (METHODOLOGY.md §3):
//! 1. Pure function of the input — same title in, same normalized title out.
//! 2. Lossy in the right direction: punctuation/case/whitespace noise is
//!    removed, but NO domain words are dropped (dropping "will" or "the"
//!    makes "Will X win" == "The X who..." collisions possible; a false pair
//!    is worse than no pair, so we stay conservative).
//! 3. Unicode-aware: alphabetic scripts other than Latin are preserved.

/// Normalize a market title for cross-venue matching:
/// lowercase, replace every non-alphanumeric character with a space, collapse
/// runs of whitespace, trim. Empty input yields an empty string.
pub fn normalize_title(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_space = true;
    for c in lower.chars() {
        if c.is_alphanumeric() {
            out.push(c);
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    // FIXTURE NOTE: every string in these tests is a synthetic, obviously
    // fake placeholder. No real market title, ticker, or identifier appears
    // anywhere in this crate's tests. See DATA.md.

    use super::*;

    #[test]
    fn normalizes_case_punct_whitespace() {
        assert_eq!(
            normalize_title("Will FAKE-EVENT-A happen?"),
            "will fake event a happen"
        );
    }

    #[test]
    fn identical_titles_normalize_equal() {
        assert_eq!(
            normalize_title("FAKE-MARKET-ALPHA: Yes or No?"),
            normalize_title("fake market alpha   yes or no!!")
        );
    }

    #[test]
    fn empty_and_noise_input() {
        assert_eq!(normalize_title(""), "");
        assert_eq!(normalize_title("?!- —."), "");
    }

    #[test]
    fn unicode_letters_survive() {
        assert_eq!(normalize_title("FAKE-ÉVENT-Ω"), "fake évent ω");
    }

    #[test]
    fn different_titles_stay_different() {
        // A conservative normalizer must NOT merge semantically different
        // questions (no stop-word stripping).
        assert_ne!(
            normalize_title("Will FAKE-PERSON-A win FAKE-EVENT-A?"),
            normalize_title("Will FAKE-PERSON-A be appointed to FAKE-EVENT-A?")
        );
    }
}
