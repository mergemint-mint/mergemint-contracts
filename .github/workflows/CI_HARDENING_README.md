# CI Workflow Hardening — Implementation Notes

Branch: `ci/workflow-hardening` (4 commits, one per issue)

## Issue 1 — Cache WASM build artifacts in `build.yml`

The existing cargo registry cache step in `.github/workflows/build.yml` cached
`~/.cargo/registry` and `~/.cargo/git` but not the compiled WASM target
directory. Added `target/wasm32v1-none` to the cache `path`, keeping the same
key (`hashFiles('**/Cargo.lock')`) and `restore-keys` scheme so a cache hit
now restores prior build artifacts, not just downloaded crates. Also added
`workflow_dispatch` to the `on:` block so the workflow can be run manually to
compare cold vs. warm cache timings.

## Issue 2 — WASM binary size regression gate in `build.yml`

Added a `Check WASM size regression` step that runs after the existing size
report. It reads a baseline byte count from
`.github/wasm-size-baseline.txt`, computes an allowed ceiling using a
`WASM_SIZE_THRESHOLD_PERCENT` env var (currently `10`), and fails the job
with an `::error::` annotation if the freshly built `.wasm` exceeds that
ceiling. If no baseline file exists or no `.wasm` artifact is found, the step
exits cleanly (no false failures on first run). The baseline file holds a
single integer (bytes) and should be regenerated with `wc -c` on the release
`.wasm` whenever an intentional size increase is merged.

## Issue 3 — Pin `stellar-cli` version in `interface-check.yml`

`cargo install stellar-cli` had no version pin, so an upstream release could
silently change `stellar contract inspect` output and break interface
comparisons unpredictably. Pinned to `stellar-cli 23.0.1` with `--locked` in
the workflow, and updated `CONTRIBUTING.md`'s prerequisites section to
install the same pinned version locally, so contributors don't get spurious
interface diffs from a mismatched CLI version. Also added
`workflow_dispatch` for manual verification runs.

## Issue 4 — Scheduled run for `security.yml`

`security.yml` already had a weekly `schedule: cron: '0 6 * * 1'` trigger
alongside push/PR, so this was largely already in place. Added
`workflow_dispatch` (per the issue's own verification instructions) and a
step-summary step that records which event (`push`, `pull_request`,
`schedule`, `workflow_dispatch`) triggered the run, so scheduled audits are
distinguishable from push-triggered ones in the Actions history.

## Verification

These are GitHub Actions workflow changes; per the task instructions no
local build/test run was performed. Each workflow now exposes
`workflow_dispatch` so a maintainer with repo access can trigger a manual run
from the Actions tab to confirm the `on:` block parses and the jobs execute
before merging.
