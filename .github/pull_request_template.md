## Summary of Changes

<!-- Describe what this PR changes and why. -->

## Related Issue

Closes #

## How to Test

<!-- Steps to verify this change works correctly. -->

1. 
2. 

## Screenshots / Output

<!-- Paste relevant command output, test results, or screenshots. -->

## Checklist

- [ ] Tests added or updated
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt` applied
- [ ] PR description includes `Closes #<issue_id>`

## Storage schema changes (only if `src/types.rs` was modified)

- [ ] Reviewed [docs/migration.md](docs/migration.md) backward-compatibility rules before adding, removing, or reordering fields on `Bounty`, `Contributor`, or other `#[contracttype]` structs
- [ ] Chose the migration strategy from the versioning policy table (versioned struct, lazy write-back, or admin migration) and documented it in the PR description
