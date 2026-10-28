//! The verification gate (METHODOLOGY.md §5).
//!
//! A title match — even an exact one — is NOT a verified pair. Two markets
//! about "the same question" can resolve YES on one venue and NO on the
//! other, because their resolution *definitions*, *sources* or *cutoffs*
//! differ. The gate demands positive evidence that the two contracts are the
//! same contract; anything less is emitted as a candidate with a
//! human-readable rationale.
//!
//! Why so strict: a FALSE pair is worse than no pair. A spread measured
//! between contracts with different resolution rules is contract risk
//! mislabeled as arbitrage — the exact failure class that produced the
//! negative result documented in README.md (16 candidate pairs, 0 verified).

// ---------------------------------------------------------------------------
// Resolution-source extraction (conservative known-token scan)
// ---------------------------------------------------------------------------

/// Known resolution-source tokens: (lowercase needle, canonical name).
/// Deliberately narrow: confirming a wrong source would fabricate a verified
/// pair, so only sources we can name exactly count. The earliest-position
/// match in the text wins.
pub const SOURCE_TOKENS: &[(&str, &str)] = &[
    ("uma", "UMA"),
    ("cftc", "CFTC"),
    ("bureau of labor statistics", "BLS"),
    ("bls.gov", "BLS"),
    ("associated press", "AP"),
    ("reuters", "Reuters"),
    ("bloomberg", "Bloomberg"),
    ("bureau of economic analysis", "BEA"),
    ("census bureau", "US Census"),
    ("noaa", "NOAA"),
    ("national weather service", "NWS"),
    ("cdc", "CDC"),
    ("federal election commission", "FEC"),
    ("fec.gov", "FEC"),
    ("federal reserve", "Federal Reserve"),
    ("fomc", "FOMC"),
    ("department of labor", "DOL"),
    ("department of commerce", "DOC"),
    ("department of agriculture", "USDA"),
    ("usda", "USDA"),
    ("cook political", "Cook Political Report"),
    ("new york times", "NYT"),
    ("wall street journal", "WSJ"),
    ("washington post", "WaPo"),
    ("espn", "ESPN"),
];

/// ASCII word-boundary search: the needle must not be glued to alphanumerics
/// (so "fomc" does not match inside a longer token).
fn find_token(haystack_lower: &str, needle: &str) -> Option<usize> {
    let bytes = haystack_lower.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric();
    let mut start = 0usize;
    while let Some(rel) = haystack_lower[start..].find(needle) {
        let pos = start + rel;
        let after = pos + needle.len();
        let before_ok = pos == 0 || !is_word(bytes[pos - 1]);
        let after_ok = after >= bytes.len() || !is_word(bytes[after]);
        if before_ok && after_ok {
            return Some(pos);
        }
        start = pos + 1; // needle is ASCII, so pos+1 is a char boundary
    }
    None
}

/// Extract the canonical resolution source from free text (a Polymarket
/// market description or a Kalshi rules text). `None` = no known source
/// named -> the pair stays a candidate. This is intentionally conservative:
/// most real rules text names no token from this list, and honestly fails.
pub fn extract_resolution_source(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let mut best: Option<(usize, &'static str)> = None;
    for (token, canonical) in SOURCE_TOKENS {
        if let Some(pos) = find_token(&lower, token) {
            if best.map_or(true, |(p, _)| pos < p) {
                best = Some((pos, canonical));
            }
        }
    }
    best.map(|(_, c)| c)
}

// ---------------------------------------------------------------------------
// Cutoff parsing / comparison (UTC-day granularity, no external time crate)
// ---------------------------------------------------------------------------

/// Parse the UTC calendar day out of a cutoff string. Accepts RFC-3339
/// timestamps (`YYYY-MM-DDT...`, with optional offset — the date component is
/// taken as written, which is exact enough for same-day comparison) and bare
/// `YYYY-MM-DD` dates. `None` = missing/unparseable -> never verifies.
pub fn parse_cutoff_day(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.len() < 10 {
        return None;
    }
    let bytes = t.as_bytes();
    let digit = |i: usize| bytes.get(i).is_some_and(|b| b.is_ascii_digit());
    let dash = |i: usize| bytes.get(i) == Some(&b'-');
    if !(digit(0) && digit(1) && digit(2) && digit(3) && dash(4)) {
        return None;
    }
    let day: String = t[..10].chars().collect();
    let parts: Vec<&str> = day.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = (
        parts[0].parse::<u32>().ok()?,
        parts[1].parse::<u32>().ok()?,
        parts[2].parse::<u32>().ok()?,
    );
    if y == 0 || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(day)
}

// ---------------------------------------------------------------------------
// Gate inputs / verdict
// ---------------------------------------------------------------------------

/// Everything the gate needs to judge one title-matched pair.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PairFacts {
    /// PM cutoff string (e.g. Gamma `endDate`), if present.
    pub pm_cutoff_raw: Option<String>,
    /// Kalshi cutoff string (e.g. `close_time`), if present.
    pub kalshi_cutoff_raw: Option<String>,
    /// PM resolution-source text (Gamma `description`), if fetched.
    pub pm_source_text: Option<String>,
    /// Kalshi resolution-source text (`rules_primary` + `rules_secondary`).
    pub kalshi_source_text: Option<String>,
}

/// The gate's verdict on one pair. `verified = true` ONLY when the caller has
/// established exact normalized-title equality AND all three checks pass:
/// (1) a known resolution source extractable from BOTH sides, and equal;
/// (2) both cutoffs parse;
/// (3) both cutoffs fall on the same UTC calendar day.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PairVerdict {
    pub verified: bool,
    pub pm_source: Option<String>,
    pub kalshi_source: Option<String>,
    /// Extracted + equal source (verified pairs only).
    pub resolution_source: Option<String>,
    /// Human-readable explanation — this is what a human or agent audits.
    pub rationale: String,
}

fn trunc40(s: &str) -> String {
    s.chars().take(40).collect()
}

/// Run the verification gate on one exact-title-matched pair.
pub fn classify_pair(facts: &PairFacts) -> PairVerdict {
    let pm_source = facts.pm_source_text.as_deref().and_then(extract_resolution_source);
    let kalshi_source = facts
        .kalshi_source_text
        .as_deref()
        .and_then(extract_resolution_source);
    let pm_day = facts.pm_cutoff_raw.as_deref().and_then(parse_cutoff_day);
    let kalshi_day = facts
        .kalshi_cutoff_raw
        .as_deref()
        .and_then(parse_cutoff_day);

    let mut problems: Vec<String> = Vec::new();
    if pm_source.is_none() {
        problems.push("pm resolution source not extractable from description".to_string());
    }
    if kalshi_source.is_none() {
        problems.push("kalshi resolution source not extractable from rules".to_string());
    }
    if let (Some(p), Some(k)) = (pm_source, kalshi_source) {
        if !p.eq_ignore_ascii_case(k) {
            problems.push(format!("resolution source mismatch (pm='{p}', kalshi='{k}')"));
        }
    }
    if pm_day.is_none() {
        problems.push("pm cutoff missing or unparseable".to_string());
    }
    if kalshi_day.is_none() {
        problems.push("kalshi cutoff missing or unparseable".to_string());
    }
    if let (Some(p), Some(k)) = (pm_day.as_deref(), kalshi_day.as_deref()) {
        if p != k {
            problems.push(format!("cutoff days differ (pm {p}, kalshi {k})"));
        }
    }

    if problems.is_empty() {
        let src = pm_source.expect("checked above").to_string();
        PairVerdict {
            verified: true,
            pm_source: pm_source.map(str::to_string),
            kalshi_source: kalshi_source.map(str::to_string),
            resolution_source: Some(src.clone()),
            rationale: format!(
                "exact normalized-title equality; resolution source '{src}' named by BOTH \
                 sides; cutoffs on same UTC day (pm='{}', kalshi='{}')",
                trunc40(facts.pm_cutoff_raw.as_deref().unwrap_or("")),
                trunc40(facts.kalshi_cutoff_raw.as_deref().unwrap_or("")),
            ),
        }
    } else {
        PairVerdict {
            verified: false,
            pm_source: pm_source.map(str::to_string),
            kalshi_source: kalshi_source.map(str::to_string),
            resolution_source: None,
            rationale: format!("title matched but NOT verified: {}", problems.join("; ")),
        }
    }
}

#[cfg(test)]
mod tests {
    // FIXTURE NOTE: all text below is synthetic ("FAKE-..."). The rule texts
    // mimic the SHAPE of venue rules without quoting any real market's rules.
    // See DATA.md.

    use super::*;

    #[test]
    fn extracts_known_sources_at_word_boundaries() {
        assert_eq!(extract_resolution_source("resolved per Reuters wire."), Some("Reuters"));
        assert_eq!(
            extract_resolution_source("official FAKE-agency data; backup: the associated press."),
            Some("AP")
        );
    }

    #[test]
    fn earliest_position_token_wins() {
        // "bloomberg" appears at position 0, before "reuters" -> Bloomberg.
        assert_eq!(
            extract_resolution_source("bloomberg said reuters confirmed"),
            Some("Bloomberg")
        );
    }

    #[test]
    fn glued_tokens_do_not_match() {
        assert_eq!(extract_resolution_source("the umafoundation fund"), None);
        assert_eq!(extract_resolution_source("UMA"), Some("UMA"));
    }

    #[test]
    fn unknown_source_is_none() {
        assert_eq!(extract_resolution_source("resolved per FAKE-AGENCY-9"), None);
        assert_eq!(extract_resolution_source(""), None);
    }

    #[test]
    fn cutoff_day_parsing() {
        assert_eq!(
            parse_cutoff_day("2030-01-15T12:00:00Z"),
            Some("2030-01-15".to_string())
        );
        assert_eq!(parse_cutoff_day("2030-01-15"), Some("2030-01-15".to_string()));
        assert_eq!(parse_cutoff_day(""), None);
        assert_eq!(parse_cutoff_day("not-a-date"), None);
        assert_eq!(parse_cutoff_day("2030-13-01T00:00:00Z"), None);
        assert_eq!(parse_cutoff_day("2030-00-01T00:00:00Z"), None);
    }

    fn facts(pm_cut: &str, k_cut: &str, pm_text: &str, k_text: &str) -> PairFacts {
        PairFacts {
            pm_cutoff_raw: Some(pm_cut.to_string()),
            kalshi_cutoff_raw: Some(k_cut.to_string()),
            pm_source_text: Some(pm_text.to_string()),
            kalshi_source_text: Some(k_text.to_string()),
        }
    }

    #[test]
    fn gate_verifies_matching_source_and_cutoff() {
        let v = classify_pair(&facts(
            "2030-01-15T12:00:00Z",
            "2030-01-15T09:30:00Z",
            "Resolution: Reuters.",
            "Resolution source: reuters wire.",
        ));
        assert!(v.verified, "rationale: {}", v.rationale);
        assert_eq!(v.resolution_source.as_deref(), Some("Reuters"));
    }

    #[test]
    fn gate_rejects_source_mismatch() {
        let v = classify_pair(&facts(
            "2030-01-15T12:00:00Z",
            "2030-01-15T09:30:00Z",
            "Resolution: Reuters.",
            "Resolution source: Bloomberg.",
        ));
        assert!(!v.verified);
        assert!(v.rationale.contains("source mismatch"));
    }

    #[test]
    fn gate_rejects_cutoff_day_mismatch() {
        let v = classify_pair(&facts(
            "2030-01-15T12:00:00Z",
            "2030-01-16T09:30:00Z",
            "Resolution: Reuters.",
            "Resolution source: reuters.",
        ));
        assert!(!v.verified);
        assert!(v.rationale.contains("cutoff days differ"));
    }

    #[test]
    fn gate_rejects_missing_source() {
        let v = classify_pair(&facts(
            "2030-01-15T12:00:00Z",
            "2030-01-15T09:30:00Z",
            "Resolution per FAKE-AGENCY-7.", // no known token
            "Resolution source: reuters.",
        ));
        assert!(!v.verified);
        assert!(v.rationale.contains("not extractable"));
    }

    #[test]
    fn gate_rejects_unparseable_cutoff() {
        let v = classify_pair(&facts(
            "",
            "2030-01-15T09:30:00Z",
            "Resolution: Reuters.",
            "Resolution source: reuters.",
        ));
        assert!(!v.verified);
        assert!(v.rationale.contains("unparseable"));
    }
}
