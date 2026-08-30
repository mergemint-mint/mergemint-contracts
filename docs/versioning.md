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
of unreleased changes on `main`.
