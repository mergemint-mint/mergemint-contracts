# ADR-0001: Canonical Frontend Directory

**Status:** Accepted

## Context

The repository currently contains three parallel frontend directories with overlapping components:

| Directory | `package.json` name | Scope |
|---|---|---|
| `frontend/` | `mergemint-frontend` | Full Vite + React app: routing (`App.tsx`), pages (`BountyList`, `BountyDetail`, `CreateBounty`, `ContributorProfile`), a wallet integration (`WalletContext`, `WalletConnectButton`), an API client wired to `mergemint-backend` (`lib/api.ts`), a tx-flow hook (`useTxFlow`), Vitest unit tests, and a Playwright e2e suite (`e2e/happy-path.spec.ts`). |
| `mergemint-frontend/` | `mergemint-frontend` | A smaller scaffold: `App.tsx` plus a single `BountyDetail` component with role-derived status/action logic and its own Vitest coverage (`BountyDetail.test.tsx`). Has a nested `.github/workflows/test.yml` (typecheck + build only; its own test step is commented out pending #75/#106). |
| `app/` | `@mergemint/app` | An isolated, untested form-components package ("Frontend form components for the MergeMint bounty and contributor flows"): `CreateBounty`, `ContributorProfile`, `CharCounter`, and a `lib/validation.ts` with on-chain-aware field validators (Soroban Symbol length, strkey address format, reward-amount format). Only wires a `typecheck` script — no `test`, `dev`, or `build` script. |

`BountyDetail.tsx`, `CreateBounty.tsx`, and `ContributorProfile.tsx` each exist in more than one of these, with different implementations and no shared source of truth. None of the three is exercised by the repository's top-level CI (`.github/workflows/`) — the nested workflow file under `mergemint-frontend/.github/` predates that directory being merged into this repo and is not picked up by GitHub Actions from a non-root path. `docker-compose.yml` and the root `package.json` (`workspaces: ["mergemint-contracts/sdk", "mergemint-frontend"]`, `dev:frontend` script) both reference directory names that no longer match the current layout — `docker-compose.yml`'s `./frontend` build context has no `Dockerfile`, and `mergemint-contracts/sdk` doesn't exist as a subpath of this repo — so neither is a reliable signal of intent and both are themselves drift left over from an earlier layout.

## Decision

**`frontend/` is the canonical frontend going forward.**

Reasoning:

- It is the only one of the three with working end-to-end routing, a real API client targeting `mergemint-backend`'s routes, and a wallet connection flow — the other two are partial component scaffolds, not deployable apps.
- It has both unit tests (Vitest) and an e2e suite (Playwright), giving it the strongest existing test coverage of the three.
- It has the most recent and most substantial history of feature work (e.g. `feat: implement create_bounty multi-assignee/multi-sig params and milestone completion`), indicating it's where active frontend development is already happening in practice.

`mergemint-frontend/` and `app/` are not canonical. They are not deleted by this ADR — see the retirement plan below — because each contains functionality not yet present in `frontend/` that should be ported first.

## Retirement plan

This ADR does not itself move code; it unblocks follow-up issues to do so safely:

1. **Port `mergemint-frontend/`'s role-derived `BountyDetail` status/action logic** (and its test coverage) into `frontend/src/pages/BountyDetail.tsx` / `frontend/src/components/BountyDetail.tsx`, then delete `mergemint-frontend/`.
2. **Port `app/`'s validation helpers** (`lib/validation.ts` — Symbol length, strkey address format, reward-amount format) into `frontend/`'s `CreateBounty`/`ContributorProfile` forms, which do not currently perform this validation, then delete `app/`.
3. Once both are ported, remove the stale `mergemint-frontend` entry from the root `package.json` workspaces list and point `docker-compose.yml`'s frontend service at `./frontend` (adding the missing `Dockerfile`) as a separate, focused change.
4. Update `mergemint-backend`'s CORS/allowed-origin configuration, if any, to match `frontend/`'s dev/prod origins once it's the only frontend being deployed.

Each step above should land as its own PR so review stays scoped to one directory at a time.
