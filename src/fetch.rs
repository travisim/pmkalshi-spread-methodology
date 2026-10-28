//! Live fetchers for the two venue public APIs — the DISCOVERY half only.
//!
//! Both endpoints are keyless public API reads, hit politely (<= 1 req/s,
//! see `MIN_REQUEST_INTERVAL`). This module fetches *current* listings so
//! anyone can regenerate candidates themselves; it intentionally records
//! nothing to disk beyond the caller-chosen output file.
//!
//! NOTE ON SCOPE: this harness stops at candidates. It does NOT price spreads,
//! does NOT write to any database, and does NOT contain any recorded market
//! data (see DATA.md).

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::USER_AGENT;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Normalized venue listings
// ---------------------------------------------------------------------------

/// One Polymarket market, reduced to the fields matching needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PmMarket {
    pub id: String,
    pub question: String,
    /// Gamma `endDate` raw string (cutoff evidence).
    pub end_date: Option<String>,
    /// Gamma `description` (resolution-source evidence).
    pub description: Option<String>,
}

/// One Kalshi market (nested in an event), reduced to the fields matching
/// needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiMarketLite {
    /// Market ticker — venue's own identifier for its own market.
    pub ticker: Option<String>,
    pub title: String,
    /// `close_time` raw string (cutoff evidence).
    pub close_time: Option<String>,
    /// `rules_primary` + `rules_secondary` concatenated.
    pub rules_text: Option<String>,
}

// ---------------------------------------------------------------------------
// Polymarket Gamma
// ---------------------------------------------------------------------------

/// Public Gamma listings endpoint (keyless).
pub const GAMMA_MARKETS_URL: &str = "https://gamma-api.polymarket.com/markets";

/// Parse one Gamma list page (a JSON array of market objects).
pub fn parse_gamma_page(body: &str) -> Result<Vec<PmMarket>, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Gamma page is not valid JSON: {e}"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| "Gamma page is not a JSON array".to_string())?;
    let mut out = Vec::new();
    for m in arr {
        let Some(id) = m.get("id").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(question) = m
            .get("question")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|q| !q.is_empty())
        else {
            continue;
        };
        out.push(PmMarket {
            id: id.to_string(),
            question: question.to_string(),
            end_date: m
                .get("endDate")
                .and_then(|x| x.as_str())
                .map(str::to_string),
            description: m
                .get("description")
                .and_then(|x| x.as_str())
                .map(str::to_string),
        });
    }
    Ok(out)
}

/// Fetch `pages` Gamma pages (100 markets each), sequentially, with a polite
/// pause between requests.
pub async fn fetch_pm_markets(client: &reqwest::Client, pages: u32) -> Result<Vec<PmMarket>, String> {
    let mut out = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    for page in 0..pages {
        let offset = page * 100;
        let resp = client
            .get(GAMMA_MARKETS_URL)
            .query(&[
                ("limit", "100".to_string()),
                ("offset", offset.to_string()),
                ("closed", "false".to_string()),
                ("active", "true".to_string()),
            ])
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| format!("HTTP request to Polymarket Gamma failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Gamma returned HTTP {}", resp.status()));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read Gamma response body: {e}"))?;
        for m in parse_gamma_page(&body)? {
            if seen_ids.insert(m.id.clone()) {
                out.push(m);
            }
        }
        if page + 1 < pages {
            tokio::time::sleep(crate::MIN_REQUEST_INTERVAL).await;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Kalshi election API (public, keyless)
// ---------------------------------------------------------------------------

/// Public Kalshi events endpoint with nested markets (keyless). One
/// unpaginated request returns up to 200 open events. Nested per-outcome
/// markets carry the PM-style single-question titles the matcher needs;
/// prices in this endpoint are quoted in DOLLARS and are deliberately NOT
/// used here — the discovery half does not price anything.
pub const KALSHI_EVENTS_URL: &str = "https://api.elections.kalshi.com/trade-api/v2/events?limit=200&status=open&with_nested_markets=true";

/// Parse a Kalshi events body
/// (`{"events":[{"title","event_ticker","markets":[...]}]}`) into
/// `KalshiMarketLite` values, one per nested market with a usable title.
pub fn parse_kalshi_events(body: &str) -> Result<Vec<KalshiMarketLite>, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Kalshi events response is not valid JSON: {e}"))?;
    let events = v
        .get("events")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "Kalshi events response missing `events` array".to_string())?;
    let mut out = Vec::new();
    for ev in events {
        let Some(markets) = ev.get("markets").and_then(|m| m.as_array()) else {
            continue;
        };
        for m in markets {
            if let Some(status) = m.get("status").and_then(|s| s.as_str()) {
                if !status.eq_ignore_ascii_case("active") {
                    continue;
                }
            }
            let Some(title) = m
                .get("title")
                .and_then(|t| t.as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
            else {
                continue;
            };
            let p = m.get("rules_primary").and_then(|x| x.as_str()).unwrap_or("");
            let s = m.get("rules_secondary").and_then(|x| x.as_str()).unwrap_or("");
            let combined = format!("{p} {s}").trim().to_string();
            out.push(KalshiMarketLite {
                ticker: m.get("ticker").and_then(|x| x.as_str()).map(str::to_string),
                title: title.to_string(),
                close_time: m.get("close_time").and_then(|x| x.as_str()).map(str::to_string),
                rules_text: if combined.is_empty() { None } else { Some(combined) },
            });
        }
    }
    Ok(out)
}

/// Fetch open Kalshi markets via the events endpoint (keyless). Quotes are
/// deduped by title (first wins).
pub async fn fetch_kalshi_markets(client: &reqwest::Client) -> Result<Vec<KalshiMarketLite>, String> {
    let resp = client
        .get(KALSHI_EVENTS_URL)
        .timeout(HTTP_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("HTTP request to Kalshi events failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Kalshi returned HTTP {}", resp.status()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| format!("failed to read Kalshi response body: {e}"))?;
    let mut out = parse_kalshi_events(&body)?;
    let mut seen: HashSet<String> = HashSet::new();
    out.retain(|m| seen.insert(m.title.clone()));
    Ok(out)
}

/// Build a polite shared HTTP client.
pub fn make_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("building http client: {e}"))
}

#[cfg(test)]
mod tests {
    // FIXTURE NOTE: every JSON string below is a synthetic fixture using
    // obviously-fake names ("FAKE-..."). It mimics the RESPONSE SHAPE of the
    // public endpoints and contains no real market data of any kind
    // (no real titles, tickers, ids, prices or rules). See DATA.md.

    use super::*;

    #[test]
    fn parses_synthetic_gamma_page_fixture() {
        let body = r#"[
            {
                "id": "FIXTURE-ID-0001",
                "question": "Will FAKE-EVENT-A be approved?",
                "endDate": "2030-01-15T12:00:00Z",
                "description": "Resolves Yes if FAKE-EVENT-A is approved per Reuters."
            },
            { "id": "FIXTURE-ID-0002", "question": "  " },
            { "question": "no id here" }
        ]"#;
        let got = parse_gamma_page(body).expect("fixture must parse");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "FIXTURE-ID-0001");
        assert_eq!(got[0].question, "Will FAKE-EVENT-A be approved?");
        assert_eq!(got[0].end_date.as_deref(), Some("2030-01-15T12:00:00Z"));
        assert!(got[0].description.as_deref().unwrap().contains("Reuters"));
    }

    #[test]
    fn gamma_fixture_garbage_is_an_error() {
        assert!(parse_gamma_page("not json").is_err());
        assert!(parse_gamma_page(r#"{"not":"an array"}"#).is_err());
    }

    #[test]
    fn parses_synthetic_kalshi_events_fixture() {
        let body = r#"{
            "events": [
                {
                    "title": "FAKE-EVENT-A outcome market (fixture)",
                    "event_ticker": "FIXTURE-EVT-1",
                    "markets": [
                        {
                            "ticker": "FIXTURE-MKT-1",
                            "title": "Will FAKE-PERSON-A be chosen for FAKE-ROLE-X?",
                            "status": "active",
                            "close_time": "2045-01-01T15:00:00Z",
                            "rules_primary": "Resolves per official FAKE-AGENCY announcements.",
                            "rules_secondary": "Secondary source: new york times."
                        },
                        { "title": "closed fixture", "status": "settled" },
                        { "rules_primary": "no title" }
                    ]
                },
                { "markets": null }
            ]
        }"#;
        let got = parse_kalshi_events(body).expect("fixture must parse");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ticker.as_deref(), Some("FIXTURE-MKT-1"));
        assert_eq!(got[0].title, "Will FAKE-PERSON-A be chosen for FAKE-ROLE-X?");
        assert_eq!(got[0].close_time.as_deref(), Some("2045-01-01T15:00:00Z"));
        // rules_primary + rules_secondary concatenated, case-normalized by the
        // caller's source scan later.
        let rules = got[0].rules_text.as_deref().unwrap();
        assert!(rules.contains("FAKE-AGENCY"));
        assert!(rules.contains("new york times"));
    }

    #[test]
    fn kalshi_fixture_missing_events_array_is_error() {
        assert!(parse_kalshi_events("nope").is_err());
        assert!(parse_kalshi_events(r#"{"foo":1}"#).is_err());
    }
}
