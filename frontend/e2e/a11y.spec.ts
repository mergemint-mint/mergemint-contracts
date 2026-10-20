import AxeBuilder from "@axe-core/playwright";
import type { Locator, Page } from "@playwright/test";
import { CREATOR, expect, test } from "./fixtures";

/**
 * Automated accessibility checks (#1123). Runs axe-core against each main
 * page in both the light and dark color schemes and fails on any "serious"
 * or "critical" violation. Uses the mocked backend and wallet from
 * ./fixtures so every page renders with realistic content.
 *
 * To check a new page, add it to PAGES.
 */

interface PageCase {
  name: string;
  path: string;
  /** Text that proves the page finished loading its data. */
  ready: (page: Page) => Locator;
  /** Connect the mock wallet first (pages that are gated on a wallet). */
  connect?: boolean;
}

const PAGES: PageCase[] = [
  {
    name: "bounty list",
    path: "/",
    ready: (page) => page.getByTestId("bounty-card").first(),
  },
  {
    name: "bounty detail",
    path: "/bounties/1",
    ready: (page) => page.getByRole("heading", { level: 1, name: "Fix wallet reconnect bug" }),
    connect: true,
  },
  {
    name: "create bounty",
    path: "/create",
    ready: (page) => page.getByRole("heading", { level: 1 }),
  },
  {
    name: "profile",
    path: `/contributors/${CREATOR}`,
    ready: (page) => page.getByRole("heading", { level: 1 }),
    connect: true,
  },
  {
    name: "leaderboard",
    path: "/leaderboard",
    ready: (page) => page.getByRole("table"),
  },
];

const BLOCKING_IMPACTS = new Set(["serious", "critical"]);

for (const colorScheme of ["light", "dark"] as const) {
  test.describe(`${colorScheme} theme`, () => {
    test.use({ colorScheme });

    for (const pageCase of PAGES) {
      test(`${pageCase.name} has no serious or critical axe violations`, async ({
        page,
        api,
        wallet,
      }, testInfo) => {
        void api;
        await page.goto(pageCase.path);
        if (pageCase.connect) await wallet.connect();
        await expect(pageCase.ready(page)).toBeVisible();

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();

        await testInfo.attach("axe-results.json", {
          body: JSON.stringify(results.violations, null, 2),
          contentType: "application/json",
        });

        const blocking = results.violations
          .filter((v) => BLOCKING_IMPACTS.has(v.impact ?? ""))
          .map((v) => ({
            rule: v.id,
            impact: v.impact,
            help: v.help,
            targets: v.nodes.map((n) => n.target.join(" ")),
          }));

        expect(blocking, "serious/critical axe violations").toEqual([]);
      });
    }
  });
}
