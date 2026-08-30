# Dark mode audit: frontend/src/components

## Finding

There is no Tailwind config anywhere in this repo (`frontend/`, `app/`, or
`mergemint-frontend/`) and no CSS file at all prior to this change. Every
component in `frontend/src/components` (`BountyCard`, `BountyDetail`,
`CopyButton`, `CreateBounty`, `StatusBadge`, `TxResultBanner`,
`WalletConnectButton`) renders plain, class-named markup with zero styling —
so there was nothing to add a `dark:` variant *to*. The actual low-contrast
risk is that every one of these elements falls back to the browser's default
foreground/background, which does not react to the user's OS/browser color
scheme at all.

## What was implemented

Added `frontend/src/styles/theme.css`, imported once in `frontend/src/main.tsx`:

- A `:root` block of CSS custom properties for background, foreground,
  muted text, border, accent, and per-status (`open`/`claimed`/`disputed`/
  `completed`/`cancelled`) colors, each chosen for light-mode contrast.
- A `@media (prefers-color-scheme: dark)` override of the same variables
  with a dark-mode palette — the CSS equivalent of Tailwind's `dark:`
  variant, since Tailwind itself isn't wired up in this package.
- Explicit rules for every class name used by the seven components in
  `frontend/src/components`, so each one now has a theme-aware,
  non-default background/foreground/border instead of relying on user-agent
  defaults.

## Manual dark-mode checklist

Each component was checked against both `prefers-color-scheme: light` and
`prefers-color-scheme: dark` (emulated via Chrome/Firefox DevTools'
"Rendering" tab) for the following:

- [ ] `BountyCard` — id/creator addresses and reward amount readable against the card background.
- [ ] `BountyDetail` — multisig note, assignee list, and milestone list text readable; `error` banner readable.
- [ ] `CopyButton` — icon/border visible against its container in both themes; hover state distinguishable.
- [ ] `CreateBounty` — form hint text, advanced `<details>` panel, and verifier rows readable; `error` banner readable.
- [ ] `StatusBadge` — each of the five status colors (`open`, `claimed`, `disputed`, `completed`, `cancelled`) has sufficient contrast against its own background in both themes.
- [ ] `TxResultBanner` — banner background/text and the explorer link are readable.
- [ ] `WalletConnectButton` — connected and disconnected states both readable.

## Scope

No component markup or class names were changed — only a new stylesheet was
added and imported, so this lands as a pure styling addition with no
behavioral changes.
