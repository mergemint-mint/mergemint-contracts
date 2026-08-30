# Contributor FAQ

Answers to common questions about claiming and completing bounties on MergeMint.

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
