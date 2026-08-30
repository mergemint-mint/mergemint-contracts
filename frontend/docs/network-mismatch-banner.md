# Network-mismatch warning banner

## What was implemented

The connected wallet's network (testnet/mainnet) can differ from the
network the SDK is actually configured against, which causes any submitted
transaction to fail. This change surfaces that mismatch to the user
*before* they attempt a transaction, instead of letting it fail silently
at submit time.

### `frontend/src/lib/network.ts`

- `CONFIGURED_NETWORK` — the network this build targets, read from the
  `VITE_NETWORK` env var and defaulting to `"testnet"`.
- `normalizeWalletNetwork(raw)` — maps the various strings wallets report
  (Freighter uses `"TESTNET"` / `"PUBLIC"`) onto our `NetworkName` type.
- `checkNetworkMismatch(walletNetworkRaw)` — compares the wallet's reported
  network against `CONFIGURED_NETWORK` and returns
  `{ mismatched, walletNetwork, configuredNetwork }`. Returns
  `mismatched: false` when the wallet network is unknown (e.g. no wallet
  connected yet), so the banner only appears once there's something
  concrete to warn about.

### `frontend/src/lib/wallet.ts`

- `getWalletNetwork()` — reads the active network from the Freighter
  extension (`window.freighter.getNetwork()`), returning `null` if
  Freighter isn't installed or the call fails.

### `frontend/src/hooks/useNetworkMismatch.ts`

- Polls `getWalletNetwork()` every 5s while a wallet address is connected
  and runs the result through `checkNetworkMismatch()`, so the banner
  reacts if the user switches networks inside their wallet extension
  without reconnecting.

### `frontend/src/components/NetworkMismatchBanner.tsx`

- Renders a dismissible warning banner (`role="alert"`) naming both the
  wallet's network and the app's configured network when they differ.
  Dismissal is per-mount local state; the banner re-arms itself if the
  mismatch resolves and then reoccurs (e.g. the user flips networks
  again), rather than staying permanently dismissed.

### `frontend/src/App.tsx`

- Mounts `<NetworkMismatchBanner />` once at the app-shell level (below
  `<Nav />`), so the warning is visible across every route rather than
  only on pages that individually opt in.

## Why two commits

The banner logic/component landed first as its own commit; this commit
wires it into the app shell so it's actually shown to users on every page,
keeping each commit small and focused per the linked issues.
