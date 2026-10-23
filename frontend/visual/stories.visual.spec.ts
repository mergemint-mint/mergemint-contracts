import { readFileSync, existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";

/**
 * Screenshot every Storybook story of the key components listed below, in
 * the light and dark projects from playwright.visual.config.ts. New stories
 * for these components are picked up automatically from the Storybook
 * index; they fail until a baseline is committed.
 */
const COMPONENTS = [
  "Components/BountyCard",
  "Components/StatusBadge",
  "Components/TopUpModal",
  "Components/TxButton",
  "Components/CancelBountyDialog",
];

/** Components that render a full-screen overlay; screenshot the whole viewport. */
const FULL_PAGE = new Set(["Components/TopUpModal", "Components/CancelBountyDialog"]);

/** Stories render relative times ("in 3 days"), so freeze the clock. */
const FIXED_NOW = new Date("2026-10-20T12:00:00Z");

const here = dirname(fileURLToPath(import.meta.url));
const indexPath = join(here, "..", "storybook-static", "index.json");
if (!existsSync(indexPath)) {
  throw new Error("storybook-static/index.json not found. Run `npm run build-storybook` first.");
}

interface StoryEntry {
  id: string;
  title: string;
  name: string;
  type: string;
}

const stories = Object.values(
  (JSON.parse(readFileSync(indexPath, "utf8")) as { entries: Record<string, StoryEntry> }).entries,
).filter((e) => e.type === "story" && COMPONENTS.includes(e.title));

// Machines ship different system fonts, so render with a bundled copy of
// Inter instead. Embedded as data URLs so no network request is involved.
const require = createRequire(import.meta.url);
const fontFaces = [400, 500, 600, 700]
  .map((weight) => {
    const file = require.resolve(`@fontsource/inter/files/inter-latin-${weight}-normal.woff2`);
    const data = readFileSync(file).toString("base64");
    return `@font-face { font-family: "MM Visual"; font-weight: ${weight}; src: url(data:font/woff2;base64,${data}) format("woff2"); }`;
  })
  .join("\n");
const STABLE_CSS = `${fontFaces}
* { font-family: "MM Visual", sans-serif !important; }
*, *::before, *::after { transition: none !important; animation: none !important; }`;

async function openStory(page: Page, id: string) {
  await page.clock.setFixedTime(FIXED_NOW);
  await page.goto(`/iframe.html?id=${id}&viewMode=story`);
  await page.addStyleTag({ content: STABLE_CSS });
  await expect(page.locator("#storybook-root > *").first()).toBeAttached();
  await page.evaluate(() => document.fonts.ready);
}

test("covers every key component", () => {
  for (const title of COMPONENTS) {
    expect(
      stories.some((s) => s.title === title),
      `no stories found for ${title}`,
    ).toBe(true);
  }
});

for (const story of stories) {
  test(`${story.title} / ${story.name}`, async ({ page }) => {
    await openStory(page, story.id);
    const target = FULL_PAGE.has(story.title) ? page : page.locator("#storybook-root");
    await expect(target).toHaveScreenshot(`${story.id}.png`);
  });
}
