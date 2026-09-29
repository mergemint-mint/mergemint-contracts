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

The contract exposes a cheap `version(env) -> Symbol` query in
`src/contract/queries.rs` that returns the deployed contract version as a semver
string. The value is sourced from a single constant (`CONTRACT_VERSION`) so the
contract, the SDK and the docs cannot drift apart.

Clients (frontend, SDK, backend indexer) should call `version()` on startup and
compare the result against the version they were built for, warning the user when
they are pointed at an unexpected contract version instead of surfacing a
confusing runtime error later.

### Bumping the version

When a release is tagged, update the version in **both** places in the same pull
request, keeping them identical:

1. `CONTRACT_VERSION` in `src/contract/queries.rs` — the value returned by `version()`.
2. The `[Unreleased]` section in [CHANGELOG.md](../CHANGELOG.md), renamed to the new
   version as described below.

Choose the new value using the bump rules above (MAJOR / MINOR / PATCH). The
`version()` query itself is a backwards-compatible addition, so introducing it is a
MINOR bump; subsequent changes follow the rules for the change being made.

## Changelog Discipline

Every pull request that changes contract behaviour must add an entry under
`## [Unreleased]` in [CHANGELOG.md](../CHANGELOG.md) before merging. Use one of:

- `### Added` — new features
- `### Changed` — changes to existing behaviour
- `### Deprecated` — soon-to-be-removed features
- `### Removed` — removed features
- `### Fixed` — bug fixes
- `### Security` — security fixes

When a release is tagged, the `[Unreleased]` section is renamed to the new version
and a fresh `[Unreleased]` section is added at the top.

## Current Version

See [CHANGELOG.md](../CHANGELOG.md) for the current released version and the list
of unreleased changes on `main`. The value returned by the on-chain `version()`
query must always match the latest released version documented there.
