import { defineConfig, devices } from "@playwright/test";

/**
 * Visual regression snapshots of Storybook stories (#1124).
 *
 * Runs against the static Storybook build, so build it first:
 *
 *   npm run build-storybook
 *   npm run test:visual
 *
 * See visual/README.md for how to update the baselines.
 */
const port = 6007;

export default defineConfig({
  testDir: "./visual",
  snapshotPathTemplate: "{testDir}/__screenshots__/{projectName}/{arg}{ext}",
  timeout: 30_000,
  forbidOnly: Boolean(process.env.CI),
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  // Never write new baselines in CI; a missing baseline is a failure there.
  updateSnapshots: process.env.CI ? "none" : "missing",
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      scale: "css",
      // Absorbs sub-pixel anti-aliasing differences between machines.
      maxDiffPixelRatio: 0.002,
    },
  },
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://localhost:${port}`,
    viewport: { width: 800, height: 600 },
    deviceScaleFactor: 1,
    locale: "en-US",
    timezoneId: "UTC",
    reducedMotion: "reduce",
  },
  projects: [
    { name: "light", use: { colorScheme: "light" } },
    { name: "dark", use: { colorScheme: "dark" } },
  ],
  webServer: {
    command: `npx vite preview --outDir storybook-static --port ${port} --strictPort`,
    url: `http://localhost:${port}/iframe.html`,
    reuseExistingServer: !process.env.CI,
  },
});
