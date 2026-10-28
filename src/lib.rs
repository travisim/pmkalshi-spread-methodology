//! pmkalshi-methodology-harness
//!
//! Cross-venue prediction-market pair discovery harness
//! (Polymarket <-> Kalshi), implementing the live discovery half of the
//! methodology documented in METHODOLOGY.md.
//!
//! THIS CRATE SHIPS CODE, NOT DATA. See DATA.md: no recorded market data of
//! any kind is published with this repository. Tests use SYNTHETIC fixtures
//! with obviously-fake names (e.g. `FAKE-EVENT-A`) and never touch the
//! network.

pub mod emit;
pub mod fetch;
pub mod matching;
pub mod normalize;
pub mod verify;

/// Identify ourselves politely to both venues' public APIs.
pub const USER_AGENT: &str = concat!(
    "pmkalshi-methodology-harness/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/travisim/pmkalshi-spread-methodology)"
);

/// Polite ceiling: at most one API request per ~1.1s per venue.
pub const MIN_REQUEST_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1100);
