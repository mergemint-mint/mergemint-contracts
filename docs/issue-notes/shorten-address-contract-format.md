# Test: `shortenAddress` with realistic Stellar address formats

## What was implemented

`frontend/src/utils/shortenAddress.test.ts` previously only exercised
`shortenAddress` (from `frontend/src/utils/format.ts`) against generic,
short, hand-picked strings (`"GABCDE1234"`, `"GABCDE123456"`,
`"GABCDE1234567"`). None of those cases matched the actual shape of a
Stellar/Soroban address, so the boundary-case coverage never proved the
function behaves correctly on real input.

Added a new `describe("shortenAddress Stellar address formats", ...)` block
with two cases:

- A 56-character Stellar **account** address (`G...` prefix).
- A 56-character Soroban **contract** address (`C...` prefix).

Both assert the exact shortened output (`<first 6 chars>…<last 4 chars>`),
matching the existing lead/trail convention already used by
`shortenAddress` and by the other tests in the file.

## Why this is enough

`shortenAddress` is a pure string-slicing function with no knowledge of
strkey encoding — its only behavioral branch is the length check
(`<= 12` vs `> 12`). The existing boundary tests already cover that branch
at the edges (10, 12, 13 chars). This change adds realistic-length,
realistic-prefix inputs so a regression that only breaks on full-length
addresses (e.g. an off-by-one in the slice indices) would be caught, without
introducing a new prefix-validation code path that doesn't exist in the
source module today.

## Scope

No production code changed — `frontend/src/utils/format.ts` is untouched.
Only the test file was extended, consistent with the `test/` branch-naming
convention in `CONTRIBUTING.md`.
