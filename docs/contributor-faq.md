# Contributor FAQ

This FAQ has two parts:

- **[Contributing code to this repo](#contributing-code-to-this-repo)**: for developers opening pull requests against `mergemint-contracts`.
- **[Claiming and completing bounties](#claiming-and-completing-bounties)**: for contributors using the MergeMint platform.

---

## Contributing code to this repo

The full step-by-step path is in [CONTRIBUTING.md → Your First Contribution](../CONTRIBUTING.md#your-first-contribution). These are the questions new contributors ask most often.

---

**Where should I start?**

Pick an issue labelled `good first issue` and comment that you are taking it. Docs, test, and SDK issues are the gentlest introduction. Before touching contract code, read [architecture.md](architecture.md) for an overview of `src/`.

---

**What do I need installed?**

- Rust via `rustup`. The exact version and the `wasm32v1-none` target are pinned in `rust-toolchain.toml` and installed automatically.
- `stellar-cli` 23.0.1 (`cargo install stellar-cli --version 23.0.1 --locked`), only needed to build/deploy WASM or run the scripts in `scripts/`.
- Node.js 20 and npm, only needed for `sdk/` or `frontend/` work.
- Docker, only needed for `scripts/integration_test.sh`.

---

**How do I run the tests?**

From the repository root:

```bash
cargo test                       # contract unit + snapshot tests (no network needed)
cargo fmt --check                # formatting
cargo clippy -- -D warnings      # lints, warnings are errors
```

For other parts of the repo:

```bash
cd sdk && npm install && npm run typecheck && npm test
cd frontend && npm install && npx tsc --noEmit && npm test
cd mergemint-backend && cargo test --all-features
```

See [testing.md](testing.md) for the end-to-end shell scripts (`integration_test.sh`, `smoke_test.sh`).

---

**A snapshot test failed after my change. What do I do?**

Files in `test_snapshots/` record ledger state. If you intentionally changed a `#[contracttype]` struct, regenerate them (see [CONTRIBUTING.md → Test Snapshots](../CONTRIBUTING.md#test-snapshots)) and commit the new JSON in the **same** commit as the struct change. If you did not intend to change storage layout, the failing snapshot is telling you that you did; treat it as a bug.

---

**What does a good pull request look like?**

- Conventional-commit title, e.g. `docs: add good first issue guide`.
- `Closes #<issue>` in the description.
- A short "what and why", pasted test output, and screenshots for visual changes.
- One focused change; small PRs are reviewed faster.
- All required CI checks green (listed in [CONTRIBUTING.md](../CONTRIBUTING.md#ci--required-status-checks)).

---

**Do I need to update `CHANGELOG.md`?**

Only if you change the public contract interface: function signatures, `#[contracttype]` fields, or emitted events. Docs, tests, CI, and tooling changes don't need an entry.

---

**How long until someone reviews my PR?**

Maintainers aim to give a first review within a few working days. If a week passes with no response, leave a polite comment on the PR. If you stop responding for two weeks, the PR may be closed; you can reopen it when you're ready.

---

**My CI failed but tests pass locally. Why?**

The usual causes are an unformatted file (`cargo fmt`), a clippy lint that only appears with `-D warnings`, or a different toolchain. Make sure you are not overriding `rust-toolchain.toml`. For the `Interface Compatibility Check`, see the note on `interface-check.yml` in CONTRIBUTING.md.

---

## Claiming and completing bounties

---

**How do I find open bounties?**

Open bounties are listed on the MergeMint platform and indexed from on-chain events. Each bounty shows the title, description, reward amount, and reward token before you commit to anything.

---

**How do I claim a bounty?**

Call `claim_bounty` with your wallet address and the bounty ID. This assigns the bounty to you and moves its status to `in_progress`. Only one contributor can claim a given bounty — first claim wins.

---

**What happens if I claim a bounty but cannot complete it?**

Currently, nothing automatic happens. The bounty stays assigned to you and no one else can claim it. If you cannot complete the work, communicate with the bounty creator or verifier so they can make arrangements. A future version of the contract will introduce claim expiry to handle abandoned bounties automatically.

---

**Who is the verifier and how are they chosen?**

The verifier is the address that calls `complete_bounty` to release the reward. In practice this is typically the bounty creator or a trusted maintainer of the project. How verifiers are designated is determined off-chain by the project — the contract itself does not enforce a specific verifier address.

---

**What if the verifier never calls `complete_bounty`?**

At present, there is no on-chain timeout or escalation mechanism. If a verifier is unresponsive after you have completed the work, your recourse is off-chain: contact the project maintainers or raise the issue publicly. Automatic expiry and dispute mechanisms are planned for a future contract version.

---

**How is my reputation calculated?**

Each time a verifier calls `complete_bounty` for a bounty you completed, your reputation score increases by 10. It never decreases. Your profile also tracks total tokens earned and total bounties completed.

---

**How do I dispute a completion decision?**

There is currently no on-chain dispute mechanism. If you believe a completion was handled incorrectly — for example, a reward was not paid after work was accepted — raise the issue with the project maintainers. On-chain dispute resolution is a planned future feature.

---

## Troubleshooting

**`cargo build --target wasm32-unknown-unknown` fails, or produces no `.wasm` file where a doc/script expects it**

`rust-toolchain.toml` pins the `wasm32v1-none` target, and that's also the target CI actually builds and inspects (`.github/workflows/build.yml`, `.github/workflows/interface-check.yml`). Some setup docs and scripts still reference the older `wasm32-unknown-unknown` target. If a build command from a doc or script doesn't produce a binary where you expect it, try the toolchain-pinned target instead:

```bash
rustup target add wasm32v1-none
cargo build --release --target wasm32v1-none
```

The resulting `.wasm` will be under `target/wasm32v1-none/release/`, not `target/wasm32-unknown-unknown/release/`.

---

**`stellar` command not found, or behaves differently than expected**

Install (or reinstall) a pinned version of the CLI:

```bash
cargo install stellar-cli --locked
```

If you already have `stellar-cli` installed from a while back, an outdated version is a common source of flags or subcommands not matching what a script expects. Reinstalling with `--locked` gets you a consistent, reproducible build.

---

**`stellar account fund` / Friendbot fails, or a testnet transaction fails with an unfunded-account error**

Friendbot (the testnet funding faucet) is occasionally rate-limited or briefly unavailable. Wait a minute and retry the fund command. If it keeps failing, confirm you're targeting `--network testnet` and not `futurenet` or `local` by mistake.

---

**`scripts/integration_test.sh` hangs or fails to reach the sandbox**

This script needs a local Soroban sandbox running via Docker. Make sure Docker is installed and running before invoking the script — see `docs/testing.md` for the full prerequisites and what each test script actually covers.

---

**A contract invocation fails with an "unrecognized argument" or similar CLI error**

Double-check the flag names against the contract's actual current entrypoint signature (in `src/contract/mutations.rs`), not just an example in a doc or script — entrypoint parameters have changed over time, and not every doc/script has been kept in sync. `docs/testing.md` calls out at least one known case of this drift.
