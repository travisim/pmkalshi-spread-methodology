//! pmkalshi-harness — CLI for the cross-venue pair-discovery methodology.
//!
//! Fetches current public listings from Polymarket (Gamma, keyless) and
//! Kalshi (public election API, keyless), runs exact + fuzzy title matching
//! and the verification gate, and writes a candidates JSON for a human or
//! agent to verify (CHECKLIST.md).
//!
//! This is the DISCOVERY half only: it emits candidates, never verified
//! pairs, never prices, never a database. See README.md for the negative
//! result this methodology produced (16 candidates, 0 verified).

use clap::Parser;
use harness::emit::{run_matching, VenueStatus};
use harness::fetch;
use harness::MIN_REQUEST_INTERVAL;

#[derive(Parser)]
#[command(
    name = "pmkalshi-harness",
    about = "Discover candidate Polymarket<->Kalshi correlated-market pairs (discovery half only; emits candidates JSON, never verified pairs).",
    version
)]
struct Cli {
    /// Number of Polymarket Gamma pages to fetch (100 markets per page).
    #[arg(long, value_name = "N", default_value_t = 3)]
    pm_pages: u32,
    /// Output path for the candidates JSON ("-" prints to stdout).
    #[arg(long, value_name = "PATH", default_value = "candidates.json")]
    out: String,
}

/// One unix-seconds timestamp; local errors only (clock before epoch).
fn now_unix() -> Result<i64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock before epoch: {e}"))?
        .as_secs() as i64)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(&cli).await {
        eprintln!("pmkalshi-harness: error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: &Cli) -> Result<(), String> {
    let client = fetch::make_client()?;

    // Fetch both venues' current listings (keyless, polite, sequential).
    // A venue outage is data, not a crash: it is recorded in the doc.
    let (pm, pm_status) = match fetch::fetch_pm_markets(&client, cli.pm_pages).await {
        Ok(m) => (
            m,
            VenueStatus { status: "ok".to_string(), reason: None },
        ),
        Err(reason) => (
            Vec::new(),
            VenueStatus { status: "unavailable".to_string(), reason: Some(reason) },
        ),
    };
    tokio::time::sleep(MIN_REQUEST_INTERVAL).await;
    let (kalshi, kalshi_status) = match fetch::fetch_kalshi_markets(&client).await {
        Ok(m) => (
            m,
            VenueStatus { status: "ok".to_string(), reason: None },
        ),
        Err(reason) => (
            Vec::new(),
            VenueStatus { status: "unavailable".to_string(), reason: Some(reason) },
        ),
    };

    let ts = now_unix()?;
    let doc = run_matching(ts, &pm, &kalshi);

    // Wrap with fetch statuses for an honest record of the run.
    let envelope = serde_json::json!({
        "generated_unix_ts": doc.generated_unix_ts,
        "pm": pm_status,
        "kalshi": kalshi_status,
        "pm_markets_fetched": doc.pm_markets_fetched,
        "kalshi_markets_fetched": doc.kalshi_markets_fetched,
        "exact_title_matched": doc.exact_title_matched,
        "fuzzy_matched": doc.fuzzy_matched,
        "verified_pairs": doc.verified_pairs,
        "candidates": doc.candidates,
    });

    let rendered = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("serializing candidates: {e}"))?;
    if cli.out == "-" {
        println!("{rendered}");
    } else {
        std::fs::write(&cli.out, format!("{rendered}\n"))
            .map_err(|e| format!("writing {}: {e}", cli.out))?;
        eprintln!(
            "pmkalshi-harness: wrote {} (pm_markets={}, kalshi_markets={}, exact={}, fuzzy={}, verified_pairs={})",
            cli.out,
            doc.pm_markets_fetched,
            doc.kalshi_markets_fetched,
            doc.exact_title_matched,
            doc.fuzzy_matched,
            doc.verified_pairs,
        );
        eprintln!(
            "pmkalshi-harness: next step -> verify each candidate against CHECKLIST.md with live evidence."
        );
    }
    Ok(())
}
