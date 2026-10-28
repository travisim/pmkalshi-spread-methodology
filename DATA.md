# DATA.md — what this repository does and does not contain

## There is no data here

This repository publishes a **method, a code harness, and a negative result**.
It contains **no recorded market data of any kind** from either venue. There
is nothing to download, recombine, or resell, and no fixture in the test
suite represents a real market.

## Withheld: recorded Kalshi market data

**No recorded Kalshi market data appears anywhere in this repository.** That
includes, explicitly:

- market prices (bids, asks, last trades, implied probabilities);
- order-book depth or any aggregate liquidity measure;
- any other recorded market aggregate;
- **market titles and market/event tickers** (identifiers of real markets);
- recorded rules or resolution text of real markets.

**Why:** Kalshi's Developer Agreement (§3.1) restricts third-party sharing of
recorded data. At publish time we could not re-verify the full agreement text
(the public site returned rate-limit responses to our fetches), so **the
constraint binds conservatively**: anything that could constitute recorded
Kalshi market data is withheld entirely, including metadata such as titles and
tickers that might otherwise seem harmless.

Practical effect on this repo:

- The harness **fetches live, current data from Kalshi's keyless public API at
  run time** and keeps it in memory only; it writes nothing Kalshi-derived to
  disk. If you run the harness, what you fetch and retain is governed by *your*
  agreement with Kalshi, not by this repository.
- Test fixtures are **synthetic and labeled as such**: they mimic the JSON
  *response shape* of the public endpoints using obviously-fake placeholder
  names (`FAKE-EVENT-A`, `FAKE-PERSON-A`, `FIXTURE-*`) and invented rules
  text. They are never presented as data and never feed any reported metric.
- This document intentionally does not quote real market titles, tickers, or
  rules text, even descriptively.

## Withheld: recorded Polymarket market data

No recorded Polymarket data is published either. The reason here is trivial
rather than contractual: every datum the harness reads is **re-fetchable, by
anyone, from Polymarket's keyless public API** in seconds. Shipping a stale
copy would add nothing and age badly.

## The negative result

The run behind the negative result produced **16 candidate pairs processed,
0 verified**. The published record of that run consists of the divergence
*classes* and the gate design ([METHODOLOGY.md](METHODOLOGY.md)) — not the
candidate rows themselves, since those rows carry real market titles and
identifiers. Anyone can regenerate equivalent candidates with the harness and
verify them by hand with [CHECKLIST.md](CHECKLIST.md).
