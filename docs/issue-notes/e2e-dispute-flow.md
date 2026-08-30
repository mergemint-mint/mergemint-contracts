# E2E coverage: dispute-raise flow

## What was implemented

`frontend/e2e/happy-path.spec.ts` (issue #522) only covers
create → claim → complete. The contract (`src/contract/mutations.rs`)
also exposes `raise_dispute` and `resolve_dispute`, and
`mergemint-backend/src/routes/tx.rs` enforces that only the bounty creator
may act as arbitrator in `resolve_dispute` — none of that was exercised
end-to-end.

Added `frontend/e2e/dispute-flow.spec.ts`, following the same structure and
conventions as `happy-path.spec.ts` (two connected-wallet `page`/
`contributorPage` instances, `getByRole`/`getByText` selectors, status-text
assertions):

1. Creator posts a bounty (`Status: open`).
2. Contributor claims it (`Status: claimed`).
3. Contributor raises a dispute (`Status: disputed`) — visible to both
   parties.
4. Creator, acting as arbitrator, resolves the dispute in the
   contributor's favor (`Status: completed`) — visible to both parties.

## Notes on selectors

The "Raise Dispute" / "Resolve Dispute" / "Winner" / "Confirm Resolution"
selectors follow the same naming the contract and backend already use
(`raise_dispute`, `resolve_dispute`, `winner`) and the same role-query style
as the existing happy-path spec. `frontend/src/components/BountyDetail.tsx`
does not yet render dispute controls — only `Claim` — so, like
`happy-path.spec.ts` already does for a `Create Bounty` button that isn't
wired up either, this spec documents the expected UI contract for the
dispute flow ahead of that wiring landing.

## Scope

No production component code was changed. This is a test-only addition,
consistent with the `test/` branch-naming convention in `CONTRIBUTING.md`.
