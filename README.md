# pmkalshi-spread-methodology

**Cross-venue prediction-market spread recording methodology (Polymarket ↔ Kalshi) — method, harness, and an honest zero result.**

> **The result is zero.** Over a 7-day window we ran this methodology end to end:
> **16 candidate pairs processed through the verification gate, 0 verified.**
> Every single failure was a *real source divergence* between the two venues —
> resolution definitions, resolution sources, or cutoffs that genuinely differ —
> not a plumbing artifact. Conclusion: **naive title-matching cross-venue
> arbitrage discovery at this granularity produced zero verified pairs in a
> 7-day window.**

This repository contains **no recorded market data** (there is nothing to
publish — see [DATA.md](DATA.md)), no revenue claims, and no "opportunity"
claims. It contains:

1. **METHODOLOGY.md** — the full method: candidate generation, the
   verification gate, why the gate is strict, and the divergence classes that
   killed every candidate.
2. **A re-runnable harness** (`pmkalshi-harness`, Rust): fetches current
   public listings from both venues' keyless public APIs, runs exact + fuzzy
   title matching plus the automated portion of the verification gate, and
   emits a `candidates.json` work queue for a human or agent to verify.
3. **CHECKLIST.md** — the exact divergence checks used to verify or reject a
   candidate pair, so anyone can replicate the gate by hand.
4. **DATA.md** — what is deliberately *not* published and why.

## Why publish a zero?

A zero that survives a strict gate is a measurement. The gate exists because
**a false pair is worse than no pair**: two markets that look like "the same
question" but resolve under different definitions can resolve YES on one venue
and NO on the other. A "spread" between them is not arbitrage — it is contract
risk wearing an arbitrage costume. Our recorded candidates all failed on
exactly that class of divergence (see [METHODOLOGY.md §6](METHODOLOGY.md)).

Anyone who wants to claim cross-venue Polymarket↔Kalshi spreads exist at the
level of individual correlated questions must first clear this same gate. We
could not, in 7 days, on 16 candidates, and we document precisely why.

## What the harness does (and does not do)

The harness is the **discovery half only**:

- Fetches current open listings from the Polymarket Gamma API and the Kalshi
  public election API — both keyless, rate-limited to ≤ 1 request/second.
- Matches titles on two tiers: **exact** (normalized-title equality) and
  **fuzzy** (token-overlap Jaccard ≥ 0.70 with ≥ 4 shared words; advisory
  only, never verifiable by construction).
- Runs the automated portion of the verification gate (resolution-source
  extraction from both sides, cutoff parse + same-UTC-day comparison) and
  attaches the pass/fail rationale to each candidate.
- Writes `candidates.json` — a **work queue for human or agent verification**
  against [CHECKLIST.md](CHECKLIST.md). It never emits "verified pairs", never
  prices spreads, never writes a database.

```bash
cargo run --release -- --pm-pages 3 --out candidates.json
# then: open candidates.json and work through each candidate with CHECKLIST.md
```

## Repository layout

```
src/            harness crate (lib + CLI; tests use labeled synthetic fixtures only)
METHODOLOGY.md  the method, the gate, the divergence classes
CHECKLIST.md    the verification-gate checks, as an actionable checklist
DATA.md         what is withheld and why
```

## Honest scope notes

- The Kalshi-side listing fetch is **coverage-limited by design**: it reads one
  unpaginated request (up to 200 open events with nested markets), so overlap
  beyond that slice is not probed. We record this limit rather than hide it.
  Note, though, that the verified-pair limiter in our recorded run was the
  source/definition gate, not coverage: every title-matched candidate failed
  on a real divergence.
- The resolution-source scan is a conservative known-token list; most real
  rules text names no known token and *honestly* stays a candidate. Widening
  the matcher was deliberately **not** done before the pre-registered
  checkpoint — extension-then-kill, never extend-to-survive.
- Tests use **synthetic fixtures only** (`FAKE-EVENT-A`-style placeholders).
  No recorded market data of any kind ships with this repo ([DATA.md](DATA.md)).

## License

MIT — see [LICENSE](LICENSE).
