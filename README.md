# MergeMint

MergeMint is a Soroban smart-contract toolkit for the Stellar network, with a Next.js frontend for minting and managing NFTs.

## Repository layout

- `frontend/` — the canonical Next.js frontend package (per `docs/adr-0001-frontend-canonical-directory.md`). All wallet, UI, and app code lives here.
- `contracts/` — Soroban smart contracts.
- `docs/` — architecture decision records and project documentation.

> **Note:** `mergemint-frontend/` is deprecated. It has been consolidated into `frontend/` per ADR-0001. Do not add new code or scripts targeting `mergemint-frontend/`; use `frontend/` instead.

## Quickstart

### Prerequisites

- Node.js 20+
- npm 10+
- Rust and the Soroban CLI (for contract work)

### Frontend

```bash
cd frontend
npm install
npm run dev
```

The dev server runs at http://localhost:3000.

From the repository root you can also run:

```bash
npm run dev:frontend
```

which delegates to the canonical `frontend/` package.

### Contracts

```bash
cd contracts
cargo build --target wasm32-unknown-unknown --release
```

## Testing

```bash
cd frontend
npm test
npm run build
```

## Documentation

See `docs/` for ADRs and guides. The canonical frontend directory decision is documented in `docs/adr-0001-frontend-canonical-directory.md`.
