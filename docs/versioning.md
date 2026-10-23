# Versioning Policy

This project follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
All notable changes are recorded in [CHANGELOG.md](../CHANGELOG.md).

## Version Format

```
MAJOR.MINOR.PATCH
```

## Bump Rules

### MAJOR — breaking contract interface change

Increment the major version when a change breaks on-chain compatibility or requires
client-side migration. Examples:

- Removing or renaming a public contract function (`create_bounty`, `claim_bounty`, etc.)
- Changing the argument list or return type of any public function
- Changing the storage key layout in a way that invalidates existing ledger entries
- Changing the meaning of an event field (e.g. renaming a topic symbol)
- Dropping support for a previously valid status value or resolution symbol

### MINOR — backwards-compatible feature addition

Increment the minor version when new functionality is added without breaking existing
clients. Examples:

- Adding a new public contract function
- Adding an optional parameter with a default that preserves existing behaviour
- Adding a new event type that existing clients can safely ignore
- Adding a new error variant that existing clients do not need to handle
- Introducing a new storage key that does not affect reads of existing keys

### PATCH — backwards-compatible bug fix or internal improvement

Increment the patch version for fixes and refactors that do not affect the public
interface. Examples:

- Fixing incorrect payout arithmetic without changing function signatures
- Improving error messages (the message string is not part of the public interface)
- Refactoring internal helpers with no observable behaviour change
- Updating documentation or comments only
- Dependency version bumps with no API impact

## Contract Interface Changes

The public interface of this contract is defined by the functions exposed through
`#[contractimpl]` in `src/contract/mutations.rs` and `src/contract/queries.rs`.

Any change to those signatures — argument types, return types, function names — is
a **breaking change** and requires a MAJOR bump. Storage layout changes that prevent
existing data from being read correctly also require a MAJOR bump.

## Contract Version Query

The contract exposes a cheap `version(env) -> String` query in
`src/contract/queries.rs` that returns the deployed contract version as a semver
string. The value is sourced from a single constant (`CONTRACT_VERSION`) so the
contract, the SDK and the docs cannot drift apart.

Clients (frontend, SDK, backend indexer) should call `version()` on startup and
compare the result against the version they were built for, warning the user when
they are pointed at an unexpected contract version instead of surfacing a
confusing runtime error later.

### Bumping the version

Version bumps are automated by release-please (see [Automated releases](#automated-releases)).
When a contract release PR is merged, release-please updates **both**:

1. `CONTRACT_VERSION` in `src/contract/queries.rs` — the value returned by `version()`.
   The line carries a `// x-release-please-version` marker; do not remove it.
2. `version` in the root `Cargo.toml` (and `Cargo.lock`).

The bump size comes from the commit types since the last release, following the bump
rules above. Do not edit these versions by hand.

## Automated releases

Releases are cut by [release-please](https://github.com/googleapis/release-please)
from [Conventional Commits](https://www.conventionalcommits.org/) on `main`. The
workflow is [`.github/workflows/release-please.yml`](../.github/workflows/release-please.yml);
its configuration lives in [`release-please-config.json`](../release-please-config.json)
and the current versions in [`.release-please-manifest.json`](../.release-please-manifest.json).

### Components

Each component has its own version, changelog, tag and release PR:

| Component  | Path                 | Release type | Tag               | Changelog                                                             |
| ---------- | -------------------- | ------------ | ----------------- | --------------------------------------------------------------------- |
| `contract` | `.` (root crate)     | `rust`       | `contract-vX.Y.Z` | [`CHANGELOG.md`](../CHANGELOG.md)                                     |
| `backend`  | `mergemint-backend/` | `rust`       | `backend-vX.Y.Z`  | [`mergemint-backend/CHANGELOG.md`](../mergemint-backend/CHANGELOG.md) |
| `sdk`      | `sdk/`               | `node`       | `sdk-vX.Y.Z`      | `sdk/CHANGELOG.md` (created on the first SDK release)                 |

A commit belongs to a component when it touches files under that component's path.
The `contract` component ignores `sdk/`, `mergemint-backend/`, `backend/`, the
frontends, `docs/` and `.github/`.

### How a release happens

1. Pull requests land on `main` with conventional commit messages (enforced by the
   `commitlint` and `Lint PR Title` workflows).
2. On each push to `main`, release-please opens or updates one release PR per
   component with changes, titled `chore(<component>): release X.Y.Z`. The PR
   bumps the version and adds a changelog entry.
3. A maintainer reviews the PR — the changelog text can be edited in the PR — and
   merges it.
4. release-please tags the merge commit (`<component>-vX.Y.Z`) and creates a GitHub
   release with the same notes.
5. For an `sdk` release, the workflow then calls
   [`sdk-publish.yml`](../.github/workflows/sdk-publish.yml), which checks the tag
   matches `sdk/package.json` and publishes `@mergemint/sdk` to npm. Pushing an
   `sdk-v*` tag by hand still triggers the same publish.

### Commit types and changelog sections

| Commit type                                        | Changelog section  | Version bump      |
| -------------------------------------------------- | ------------------ | ----------------- |
| `feat`                                             | Added              | MINOR             |
| `fix`                                              | Fixed              | PATCH             |
| `perf`, `refactor`                                 | Changed            | PATCH             |
| `revert`                                           | Reverted           | PATCH             |
| `docs`, `style`, `test`, `chore`                   | hidden             | none on their own |
| `!` after the type, or a `BREAKING CHANGE:` footer | listed as breaking | MAJOR             |

While a component is below `1.0.0`, breaking changes bump MINOR instead of MAJOR.

Use the `!` marker or a `BREAKING CHANGE:` footer for every change listed under
[Contract Interface Changes](#contract-interface-changes), for example:

```
feat(contract)!: rename claim_bounty to claim

BREAKING CHANGE: clients must call `claim` instead of `claim_bounty`.
```

To force a specific version, add a `Release-As: X.Y.Z` footer to a commit.

### Dry run

Run the **Release Please** workflow manually (Actions → Release Please → Run
workflow) with `dry-run` checked. It prints the release PRs and changelogs it would
create without opening PRs or tags. Use this on a fork to check changes to the
configuration.

### Tokens and secrets

- `GITHUB_TOKEN` is enough to open release PRs and create tags. CI does not run on
  PRs opened by `GITHUB_TOKEN`; to get CI on release PRs, add a `RELEASE_PLEASE_TOKEN`
  secret (a fine-grained PAT with contents and pull-request write access) and the
  workflow will use it instead.
- `NPM_TOKEN` is required for the SDK publish.

## Changelog Discipline

Changelog entries are generated from commit messages, so the commit (or squashed
PR title) is the changelog entry. Write it for the people who read the changelog:
name the function, endpoint or SDK method that changed.

Hand-written notes under `## [Unreleased]` are no longer required. release-please
inserts each new release above the existing `## [Unreleased]` heading; on the first
automated release of each component, move any notes still listed there into the
release PR's changelog entry and delete the `[Unreleased]` section.

## Current Version

The current version of each component is recorded in
[`.release-please-manifest.json`](../.release-please-manifest.json). The value returned
by the on-chain `version()` query must always match the latest `contract` release in
[CHANGELOG.md](../CHANGELOG.md); release-please keeps the two in step.
