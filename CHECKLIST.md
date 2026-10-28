# Verification-gate checklist

Apply to ONE candidate pair (a title match from `candidates.json`, or any
Polymarket↔Kalshi pair you believe is "the same question"). Every check must
be answered with **live evidence fetched now** from the venue's public API or
website — not memory, not a screenshot from last week, not the other venue's
claim about itself. A candidate that fails ANY check is a candidate forever.

The automated harness (`pmkalshi-harness`) runs the *automatable subset* of
this checklist (normalized-title equality, source-token extraction, cutoff
parse + same-UTC-day comparison) and records its rationale on each candidate.
These checks are the human-audit layer on top.

---

## A. Title identity

- [ ] **A1. Exact normalized-title equality.** Lowercase both titles, replace
      punctuation with spaces, collapse whitespace: are the results identical?
      (If not identical, the pair is a FUZZY match. Fuzzy matches are advisory
      only and can never verify. Stop here.)

## B. Resolution definition (the check that killed 100% of our candidates)

Read BOTH sides' full resolution text (Polymarket description/rules; Kalshi
`rules_primary` + `rules_secondary`), live, and compare clause by clause:

- [ ] **B1. Winning condition identical?** Does each side define exactly the
      same event as "Yes"? Watch for: interim/acting/caretaker appointments
      counting on one side and not the other; "sworn in" vs "appointed";
      different eligibility windows; different fallback ("Other"/"none of the
      above") semantics and deadlines.
- [ ] **B2. No one-sided edge cases.** Can you construct a concrete news event
      that resolves YES on one venue and NO on the other? If yes → the pair
      measures contract risk, not a spread. REJECT.
- [ ] **B3. Tie/early-termination rules identical?** Delays, extensions,
      rescheduling, and void policies on both sides.

## C. Resolution source

- [ ] **C1. Each side names a source you can identify exactly** (an agency,
      wire service, official release, or oracle). If a side names none, or
      names one too vague to check, the automated gate will honestly refuse —
      and so should you unless you can name the source for both sides.
- [ ] **C2. The sources are THE SAME.** Different primary sources (e.g. one
      venue's official announcements vs the other's named-newspaper secondary)
      count as different, even if they usually agree.

## D. Cutoff / timeline

- [ ] **D1. Both cutoffs parse** (Polymarket `endDate`; Kalshi `close_time`)
      and neither is a placeholder for "no scheduled close". A placeholder
      must be treated as a different-in-kind timeline, not mapped to "open".
- [ ] **D2. Cutoffs fall on the same UTC calendar day** (or both are
      genuinely open-ended in the same sense). Record both raw strings.
- [ ] **D3. Internal deadlines match.** Some venues carry an internal final
      deadline in the rules text while the API cutoff rolls forward; compare
      those too (one side said "the rolling end date", the other "fixed date
      far in the future" — that is a fail).

## E. Structural sameness

- [ ] **E1. Same outcome space.** Single YES/NO on both sides, same "the"
      entity, no per-outcome-vs-multi-outcome conflation (a Kalshi market
      nested in a multi-outcome event is comparable to a Polymarket single
      question ONLY if B and C pass).
- [ ] **E2. Same trading state.** Both markets open and tradeable at the same
      time; one being close-only on your geography is not a venue property —
      record it, do not verify on it.

## F. Verdict

- [ ] **VERIFIED** only if A1, B1–B3, C1–C2, D1–D3, E1–E2 all pass. Record:
      the normalized title, both raw cutoff strings, the named resolution
      source(s), and the date you checked, with links to the live evidence.
- [ ] Anything else → **CANDIDATE**. Write down which check failed and the
      concrete divergence. That sentence is the real output of this method:
      in our recorded run, all 16 candidates died on class B (resolution
      definitions), C (sources), or D (cutoffs) — see METHODOLOGY.md §6.
