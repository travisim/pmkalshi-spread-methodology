# Methodology: cross-venue prediction-market pair discovery and verification

This document describes the full method, in the order it is applied. It was
executed end to end over a 7-day window with this outcome:

> **16 candidate pairs processed through the verification gate. 0 verified.**
> Every failure was a real source divergence between the venues (§6), not a
> data-plumbing artifact.

The raw recorded market data behind that run is deliberately **not published**
(see [DATA.md](DATA.md)); this repo is method + code + the negative result.
No recorded data is required to re-run the method: the harness re-fetches
current public listings from both venues' keyless public APIs.

---

## 1. Goal, and what this is not

**Goal:** produce a decision-grade picture of price gaps between Polymarket
and Kalshi on *correlated markets that are actually the same contract*, with
depth, so that any GO/KILL decision on cross-venue spread strategies is
measurable instead of argued.

**What this is not:** an arbitrage strategy, a revenue claim, or an
"opportunity" claim. The recorded run produced **zero verified pairs**, so
there is no spread distribution to report, and none is claimed.

## 2. Design stance: a false pair is worse than no pair

The central design decision is a **strict verification gate** between
"title looks similar" and "these two markets are the same contract".

Two venues can phrase the same-sounding question with materially different
resolution rules. When that happens, the same news event resolves YES on one
venue and NO on the other. A price gap between such markets is not arbitrage —
it is **contract risk mislabeled as a spread**. Measuring it, recording it, or
trading it poisons every downstream number. Hence:

- A pair enters the verified pool only with positive evidence that resolution
  definitions, sources and cutoffs agree.
- Everything else — including exact title matches that cannot clear the
  source/cutoff checks — remains a **candidate**, never a measurement.
- Fuzzy matches can **never** verify, by construction.
- Widening the matcher to manufacture verified pairs is prohibited before a
  pre-registered checkpoint; the honest overlap is a measurement even when
  (especially when) it is zero.

## 3. Title normalization ("matchnorm")

Both matching tiers operate on a canonicalized title:

- lowercase;
- every non-alphanumeric character becomes a space;
- whitespace runs collapse; trim.

Design rules: the function is pure; **no domain words are dropped** (dropping
"will"/"the" would create false collisions — a false pair is worse than no
pair); Unicode letters are preserved. Implementation: `src/normalize.rs`.

## 4. Two-tier candidate matching

Given the two venues' current open listings:

**Exact tier — normalized-title equality.** This is the only tier that may
proceed to the verification gate. Collisions are deduplicated (first quote
wins), and each normalized title is processed once.

**Fuzzy tier — token-overlap screen.** Token-set Jaccard ≥ 0.70 **and** ≥ 4
shared normalized words (the shared-word guard stops tiny-title Jaccard
inflation). The best-scoring admissible Kalshi title is attached to each
unmatched Polymarket question. Fuzzy candidates are **advisory only**: they
seed human review and can never verify by construction. The leniency of the
screen (e.g. a one-word difference on long titles passes it) is exactly why.

Implementation: `src/matching.rs`.

## 5. The verification gate

Applied to every exact-tier match. A pair is verified only if **all** of the
following hold; otherwise it is emitted as a candidate with a machine- and
human-readable rationale stating which checks failed.

1. **Title identity** — exact normalized-title equality (established by the
   caller; fuzzy matches never reach the gate).
2. **Resolution source extractable from BOTH sides, and equal.** Each side's
   rules text (Polymarket market description; Kalshi `rules_primary` +
   `rules_secondary`) is scanned against a conservative list of known
   resolution-source tokens (UMA, CFTC, BLS, AP, Reuters, …). The earliest
   word-boundary match wins. If either side names no known token → the pair
   stays a candidate. If both name tokens and they differ → rejection.
3. **Cutoffs parse and fall on the same UTC calendar day.** Polymarket
   `endDate` and Kalshi `close_time` must both parse; dates are compared at
   UTC-day granularity because venues quote cutoffs at different hours. A
   missing or unparseable cutoff → rejection; different days → rejection.
   Raw cutoff strings are always retained on the candidate so the comparison
   is auditable.

The conservative source scan means most real pairs *honestly* stay candidates:
confirming a wrong source would fabricate a verified pair, which is the one
unacceptable outcome. Implementation: `src/verify.rs`; the human-audit
version of the same checks is [CHECKLIST.md](CHECKLIST.md).

## 6. What actually killed the candidates: the recorded divergence classes

In the recorded 7-day run, **every one of the 16 distinct candidates belonged
to a single family**: Polymarket single-question markets ("Will X be the next
<title>?") title-matched (exact normalized-title equality) to the nested
per-outcome markets of the corresponding Kalshi multi-outcome event ("Who
will succeed <incumbent> as <title>?", asking the same per-person question).
Verification was attempted **by hand with live API evidence on both sides**
(Gamma and Kalshi events endpoints), not from memory. All 16 failed on real
divergences, in three classes:

1. **Resolution definitions differ materially.** The Kalshi rules stated that
   acting or interim appointments count as holding the position (resolution =
   "first new person to hold the office after issuance"), while the Polymarket
   description explicitly excluded interim/caretaker appointees, required the
   person to be sworn in after a specified election, and provided an
   "Other" fallback with a fixed deadline. **The same news event can resolve
   Yes on one venue and No on the other** — a gap there is contract risk, the
   exact UMA-vs-CFTC-class hazard cross-venue matching must exclude.
2. **Resolution sources differ.** The Polymarket side cited official
   information from the relevant government, with no extractable source token
   (the `resolutionSource` field was empty); the Kalshi side cited official
   government announcements with a named newspaper as secondary. Not
   identical sources, and the automated gate correctly could not extract equal
   sources from both sides.
3. **Cutoffs differ in kind.** Kalshi's `close_time` was a far-future
   placeholder (no scheduled close), while Polymarket's `endDate` rolled
   forward with an internal fixed deadline of a different year. Even
   artifact-aware cutoff handling (treating the placeholder as "open") cannot
   make these equal, so the same-day comparison fails — correctly.

Consequence, per the pre-registered rules: **verified_pairs = 0 stands as a
real measurement.** The pool is also coverage-limited (the Kalshi fetch reads
one unpaginated request — up to 200 open events ≈ ~1850 nested markets — so
more overlap may exist beyond it), but the limiter in the recorded run was the
source/definition gate above, which is conservative by design. No matcher
extension was made before the pre-registered checkpoint.

## 7. Re-running the method

The harness (`src/`) implements the **discovery half** so anyone can
regenerate candidates from live data:

```
cargo run --release -- --pm-pages 3 --out candidates.json
```

- `--pm-pages N`: how many 100-market Polymarket Gamma pages to fetch
  (default 3; sequential, ≤ 1 req/s overall).
- Kalshi: one keyless request to the public election API events endpoint
  (up to 200 open events, nested per-outcome markets feed the matcher).
- Output `candidates.json`: one entry per candidate with `match_kind`
  (`exact_unverified` | `fuzzy`), both sides' cutoff strings, extracted
  resolution sources, the Jaccard score for fuzzy matches, and the gate
  rationale (which checks failed and why). `verified_pairs` is always 0 in
  this output by construction.

Then do the part automation cannot: work through each candidate with
[CHECKLIST.md](CHECKLIST.md) using live evidence from both venues. A candidate
that passes every checklist item may be counted as a verified pair — by you,
on your evidence. None of ours did.

## 8. Known limits (recorded, not hidden)

- Kalshi-side coverage = one unpaginated events request (§6). Wider coverage
  requires pagination, which we did not add before the checkpoint.
- The automated source scan is a known-token list; a pair whose rules name a
  source outside the list stays a candidate forever. The checklist's
  manual-source step is the remedy.
- The harness does not price anything and exposes no order-book depth; the
  recorded run also found that the public Kalshi markets endpoint exposes no
  order book at all, so Kalshi-side depth must be recorded as explicitly
  unavailable.
- Cutoff comparison is UTC-day granularity; venues quoting different *hours*
  of the same day still pass (the raw strings are retained for audit).
