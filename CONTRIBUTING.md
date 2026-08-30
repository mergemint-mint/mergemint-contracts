# Contributing to MergeMint Contracts

Thank you for your interest in contributing! This guide covers everything you need to go from zero to a merged pull request.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Branch Naming](#branch-naming)
- [Pull Request Process](#pull-request-process)
- [Test Snapshots](#test-snapshots)
- [Security Considerations](#security-considerations)

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
