# Visual regression snapshots

Playwright screenshots of the Storybook stories for `BountyCard`,
`StatusBadge`, `TopUpModal`, `TxButton` and `CancelBountyDialog`, in light and
dark color schemes. Baselines live in `__screenshots__/<light|dark>/`, one PNG
per story ID.

## Run

```bash
npm run build-storybook
npm run test:visual
```

The config (`playwright.visual.config.ts`) serves `storybook-static/` with
`vite preview` on port 6007. On failure, open the HTML report
(`npx playwright show-report`) to see expected, actual and diff images.

## Update baselines

When a styling change is intentional, regenerate the affected screenshots and
commit them with the change:

```bash
npm run build-storybook
npm run test:visual:update
```

Check the new PNGs in the diff before committing.

CI runs these tests in the `mcr.microsoft.com/playwright:v1.62.1-noble` image.
If your machine renders slightly differently (CI fails but your local run
passes), generate the baselines in the same image:

```bash
docker run --rm -v "$PWD":/work -w /work/frontend \
  mcr.microsoft.com/playwright:v1.62.1-noble \
  sh -c "npm ci && npm run build-storybook && npm run test:visual:update"
```

When you bump `@playwright/test`, bump the image tag in
`.github/workflows/frontend-ci.yml` and this file to match, then regenerate
the baselines.

## Keeping renders stable

- Text uses a bundled copy of Inter (`@fontsource/inter`), not system fonts.
- Animations and transitions are disabled, and the clock is frozen at
  2026-10-20T12:00:00Z so relative dates don't change.
- Locale is `en-US`, timezone `UTC`, viewport 800×600 at 1x scale.

## Adding components

Add the Storybook title to `COMPONENTS` in `stories.visual.spec.ts`. Every
story under that title is picked up automatically. Overlay components (modals,
dialogs) also go in `FULL_PAGE`.
