# Contributing to MergeMint Contracts

Thank you for your interest in contributing! This guide covers everything you need to go from zero to a merged pull request.

---

## Table of Contents

- [Your First Contribution](#your-first-contribution)
- [Prerequisites](#prerequisites)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Branch Naming](#branch-naming)
- [Pull Request Process](#pull-request-process)
- [Test Snapshots](#test-snapshots)
- [Security Considerations](#security-considerations)

---

## Your First Contribution

New here? This is the shortest path from a fresh clone to a merged pull request. Each step links to the detailed section further down.

### 1. Pick an issue

- Browse issues labelled [`good first issue`](https://github.com/mergemint-mint/mergemint-contracts/labels/good%20first%20issue). These are scoped to a single area and do not require deep knowledge of Soroban.
- Leave a comment on the issue saying you are picking it up, so two people don't work on the same thing. If you have questions about the approach, ask them in the issue **before** writing code.
- Docs, test, and SDK issues are the easiest entry points. Contract changes in `src/contract/mutations.rs` touch escrowed funds and get the most careful review.

### 2. Fork, clone, and branch

```bash
# Fork on GitHub first, then:
git clone https://github.com/<your-username>/mergemint-contracts.git
cd mergemint-contracts
git remote add upstream https://github.com/mergemint-mint/mergemint-contracts.git

# Always branch from an up-to-date main
git fetch upstream
git checkout -b docs/first-issue-guide upstream/main
```

Name the branch using the [prefixes below](#branch-naming) (`feat/`, `fix/`, `docs/`, `test/`, `refactor/`, `ci/`).

### 3. Set up your toolchain

Install the [prerequisites](#prerequisites). `rust-toolchain.toml` pins the exact Rust version and the `wasm32v1-none` target, so `rustup` installs them automatically the first time you run `cargo` inside the repo.

### 4. Confirm the baseline is green

Before changing anything, check that the existing suite passes on your machine. If it doesn't, the problem is your setup, not your change. See [docs/contributor-faq.md](docs/contributor-faq.md#troubleshooting) for common fixes.

| Area you are changing          | Directory            | Command(s) to run                                                   |
| ------------------------------ | -------------------- | ------------------------------------------------------------------- |
| Smart contract (Rust)          | repo root (`src/`)   | `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`    |
| Contract WASM build            | repo root            | `cargo build --release --target wasm32v1-none`                      |
| Backend (Rust)                 | `mergemint-backend/` | `cargo test --all-features`, `cargo clippy --all-targets -- -D warnings` |
| TypeScript SDK                 | `sdk/`               | `npm install`, `npm run typecheck`, `npm test`                      |
| Frontend (React + Vite)        | `frontend/`          | `npm install`, `npx tsc --noEmit`, `npm test`                       |
| Frontend components (visual)   | `frontend/`          | `npm run storybook` (component catalog on http://localhost:6006)    |
| Docs only                      | `docs/`, `*.md`      | Preview the Markdown and check that every link and command works    |

`make test` and `make lint` wrap the contract commands.

### 5. Make the change

- Keep the change focused on the issue. Unrelated cleanups belong in a separate PR.
- Add or update tests alongside code changes. Contract tests live in `src/test.rs` and `src/contract/queries_test.rs`.
- If you change a `#[contracttype]` struct, read [Test Snapshots](#test-snapshots) and [docs/migration.md](docs/migration.md) first.
- If you change the public contract interface, add a [`CHANGELOG.md`](#changelog) entry.

### 6. Commit

Use [Conventional Commits](https://www.conventionalcommits.org/) style messages, matching the history of this repo:

```
docs: add good first issue guide
fix(contract): reject zero-share assignees in complete_bounty
feat(sdk): add getOpenBountiesPage helper
```

Run `cargo fmt` (and the relevant commands from the table above) before every commit.

### 7. Open the pull request

Push your branch to your fork and open a PR against `mergemint-mint/mergemint-contracts:main`. The PR template will ask for the items below. A PR that includes all of them is usually reviewed on the first pass.

**What a good PR looks like:**

- [ ] A title in Conventional Commit style (`docs: …`, `fix: …`, `feat: …`)
- [ ] `Closes #<issue-number>` in the description
- [ ] A short explanation of **what** changed and **why**, plus any trade-offs
- [ ] Pasted output of the test command(s) for the area you touched
- [ ] Screenshots for anything visual (frontend changes, Storybook stories)
- [ ] One logical change; small diffs get reviewed faster than large ones
- [ ] All [required CI checks](#ci--required-status-checks) green

### 8. Respond to review

A maintainer will review your PR. Push follow-up commits to the same branch to address comments; don't force-push over a review in progress unless asked. Once approved and green, a maintainer merges it. That's your first merged PR.

---

## Prerequisites

Make sure the following are installed before you start:

### 1. Rust (stable)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify with `rustc --version`. The project targets **stable Rust** — nightly is not required.

### 2. WASM compilation target

```bash
rustup target add wasm32-unknown-unknown
```

This is required to build the contract for Soroban deployment.

### 3. Stellar CLI

```bash
cargo install stellar-cli --version 23.0.1 --locked
```

Verify with `stellar --version`. Used for building, deploying, and inspecting contracts. CI pins this exact version in `interface-check.yml` so `stellar contract inspect` output stays stable across runs — install the same version locally to avoid false-positive interface diffs.

---

## Development Workflow

A `Makefile` at the repository root provides shortcuts for all common tasks:

| Command       | Description                                          |
| ------------- | ---------------------------------------------------- |
| `make build`  | Build the WASM contract                              |
| `make test`   | Run the full test suite                              |
| `make lint`   | Run Clippy (warnings as errors) and check formatting |
| `make fmt`    | Auto-format source files with rustfmt                |
| `make deploy` | Deploy the contract via `scripts/deploy.sh`          |
| `make clean`  | Remove build artifacts                               |

### Building

```bash
make build
# or: cargo build --release --target wasm32-unknown-unknown
```

### Run the test suite

```bash
make test
# or: cargo test
```

All tests run against the Soroban in-process test environment — no live network required. The suite covers `create_bounty`, `claim_bounty`, `complete_bounty`, and the bounty counter.

### Build the WASM binary

```bash
cargo build --release --target wasm32-unknown-unknown
```

Output lands at `target/wasm32-unknown-unknown/release/mergemint_contracts.wasm`. The release profile is tuned for size (`opt-level = "z"`, `lto = true`) with overflow checks enabled.

---

## Code Standards

Both of the following must pass with **zero warnings or errors** before you open a PR. CI enforces both checks.

### Formatting

```bash
cargo fmt
```

Run this before every commit. Do not disable `rustfmt` attributes without a clear reason.

### Linting

```bash
cargo clippy -- -D warnings
```

All Clippy warnings are treated as errors. Fix every diagnostic rather than suppressing it with `#[allow(...)]` unless the lint is demonstrably a false positive and you explain why in the suppression comment.

---

## Branch Naming

Use one of these prefixes followed by a short kebab-case description:

| Prefix      | When to use                                         |
| ----------- | --------------------------------------------------- |
| `feat/`     | New contract functionality or behaviour             |
| `fix/`      | Bug fixes                                           |
| `docs/`     | Documentation-only changes                          |
| `test/`     | New or updated tests with no production code change |
| `refactor/` | Internal restructuring with no behaviour change     |
| `ci/`       | Changes to GitHub Actions workflows or scripts      |

Examples: `feat/claim-expiry`, `fix/double-claim-guard`, `docs/snapshot-guide`

---

## Pull Request Process

1. **Open or find an issue first.** Every PR should be traceable to a GitHub issue. If no issue exists for your change, open one before starting work so the approach can be discussed.

2. **Link the issue in your PR description.** Use GitHub's closing keyword so the issue closes automatically on merge:

   ```
   Closes #<issue-number>
   ```

3. **Describe what changed and why.** Include:
   - A short summary of the change.
   - The motivation or the problem it solves.
   - Any trade-offs or alternatives you considered.

4. **Paste test output.** Copy the result of `cargo test` into the PR description so reviewers can confirm the suite passes locally without checking out your branch.

5. **Screenshots for UI changes.** MergeMint Contracts is a pure on-chain library with no UI, but if your PR touches the deployment scripts or produces visual output (e.g. `stellar contract inspect` output), include a screenshot or terminal capture.

6. **Keep PRs focused.** One logical change per PR. Split unrelated fixes into separate branches.

7. **Respond to review comments promptly.** A PR that goes two weeks without a response may be closed and re-opened when you are ready to continue.

---

## Changelog

Every pull request that changes the contract interface **must** include a `CHANGELOG.md` entry under the `[Unreleased]` section. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

A "contract interface change" includes:

- Adding, removing, or renaming a public contract function
- Changing the parameters or return type of a public function
- Adding, removing, or reordering fields in `Bounty`, `Contributor`, `BountyMeta`, or `DataKey`
- Changing the set of events emitted by any function

Use the appropriate subsection (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`) inside `[Unreleased]`. Example:

```markdown
## [Unreleased]

### Added

- `update_contributor_metadata` — lets contributors update their off-chain profile URI.

### Changed

- `Bounty` — added optional `deadline` field (ledger sequence number).
```

PRs that touch only tests, documentation, CI, or tooling do not require a changelog entry, but one is welcome.

## Test Snapshots

### What are snapshots?

Files under `test_snapshots/` capture the full Soroban ledger state produced by each test. They verify that storage layout, type encodings, and struct field order remain stable across code changes. A snapshot mismatch is a breaking change to the on-chain storage format.

### When snapshots become stale

The current snapshots and the structs they cover:

| Snapshot file                                      | Primary struct tested       |
| -------------------------------------------------- | --------------------------- |
| `test_bounty_count.1.json`                         | `DataKey::BountyCount`      |
| `test_claim_bounty.1.json`                         | `Bounty`, `Contributor`     |
| `test_complete_bounty_updates_status.1.json`       | `Bounty` status transitions |
| `test_contributor_reputation.1.json`               | `Contributor`               |
| `test_create_bounty.1.json`                        | `Bounty`, `DataKey`         |
| `test_status_index_tracks_bounty_lifecycle.1.json` | `DataKey::StatusIndex`      |

### How Snapshots Are Generated

Soroban's test infrastructure writes snapshot files automatically when a test that uses `Env::default()` completes. Running `cargo test` with a clean `test_snapshots/` directory will regenerate all files. On subsequent runs the framework compares the live ledger state against the stored JSON; a mismatch fails the test.

### When Snapshots Become Stale

Snapshots must be regenerated if:

- A field is added to `Bounty`, `Contributor`, or other `#[contracttype]` structs
- A field is removed or reordered
- A field type changes (e.g., `u32` → `u64`)
- Field visibility or attributes change
- A new `DataKey` variant is added

If you are unsure whether your change affects storage layout, run `cargo test` and check whether any snapshot diffs appear.

Delete the existing snapshots and rerun the test suite:

```bash
rm -f test_snapshots/test/*.json
cargo test
```

The test run will recreate all snapshot files from the current ledger state. Review the new JSON files with `git diff` before committing to confirm the changes are intentional.

If you only want to regenerate a single snapshot, delete that file and run the specific test:

```bash
rm test_snapshots/test/test_create_bounty.1.json
cargo test test_create_bounty
```

1. Review the diff with `git diff test_snapshots/` and confirm each change is intentional.
2. Commit the updated snapshot files in the same commit as the struct change — never in a separate commit, because the snapshots and the code must stay in sync.

To ensure all snapshots are current and pass:

```bash
cargo test
```

If all tests pass without modification, the snapshots are valid against the current schema.

## Changelog

[CHANGELOG.md](./CHANGELOG.md) records the history of the public contract
interface. It follows the [Keep a Changelog](https://keepachangelog.com) format.

**Any PR that changes the public interface must update `CHANGELOG.md`.**
This includes:

- Adding, removing, or renaming a contract function
- Changing function parameter types or order
- Adding, removing, or reordering fields in `Bounty`, `BountyMeta`,
  `Contributor`, or `DataKey`
- Adding or removing emitted events, or changing their payloads

Add your entry under the `[Unreleased]` section at the top of the file using
one of the standard categories: `Added`, `Changed`, `Deprecated`, `Removed`,
`Fixed`, or `Security`.

## Code Style

---

## CI & Required Status Checks

All of the following GitHub Actions workflows must pass before a pull request can be merged to `main`:

| Workflow file             | Status check name               | Required |
| ------------------------- | ------------------------------- | -------- |
| `interface-check.yml`     | Interface Compatibility Check   | ✅ Yes   |
| `ci.yml` (Backend CI)     | Backend CI                      | ✅ Yes   |
| `lint.yml`                | Lint                            | ✅ Yes   |
| `security.yml`            | Security Audit (`cargo-audit`)  | ✅ Yes   |
| `frontend-ci.yml`         | Frontend CI                     | ✅ Yes   |

> **`interface-check.yml` is the critical gate.** It detects breaking changes to the public contract interface by comparing the current ABI against the last recorded snapshot. A failure here means a public function signature, parameter type, or event payload has changed in a backwards-incompatible way. This check **must pass** before merging to `main`.

### Configuring branch protection rules

If the required status checks are not yet enforced on `main`, set them up via **Settings → Branches → Branch protection rules** for the `main` branch:

1. Enable **Require status checks to pass before merging**.
2. Search for and add each status check name from the table above.
3. Enable **Require branches to be up to date before merging** to prevent stale-branch bypasses.
4. Save the rule.

---

## Security Considerations

- Every state-mutating function must call `require_auth()` on the relevant `Address` argument before touching storage.
- Validate all external inputs at the contract boundary — do not rely on the caller to pass well-formed data.
- Do not introduce arithmetic that bypasses the overflow protection provided by `overflow-checks = true` in the release profile.
- If your change introduces a new trust boundary or changes which address is authorised to perform an action, call it out explicitly in the PR description and link to the relevant section of `docs/security.md`.
