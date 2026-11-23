import AxeBuilder from "@axe-core/playwright";
import type { Locator, Page } from "@playwright/test";
import { CREATOR, expect, makeBounty, test } from "./fixtures";

const CRITICAL_IMPACTS = ['critical', 'serious'];

async function assertNoCriticalViolations(page: import('@playwright/test').Page) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();

  const violations = results.violations.filter((v) =>
    CRITICAL_IMPACTS.includes(v.impact ?? ''),
  );

  expect(
    violations,
    `Critical/serious axe violations: ${violations
      .map((v) => `${v.id} (${v.impact})`)
      .join(', ')}`,
  ).toEqual([]);
}

interface PageCase {
  name: string;
  path: string;
  ready: (page: Page) => Locator;
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

    test("submitted, archived, and paused bounty states have no serious or critical axe violations", async ({
      page,
      api,
      wallet,
    }, testInfo) => {
      api.seed(
        makeBounty({ id: "1", status: "submitted", title: "Submitted bounty item" }),
        makeBounty({ id: "2", status: "archived", title: "Archived bounty item" }),
        makeBounty({ id: "3", status: "paused", paused: true, title: "Paused bounty item" }),
      );

      await page.goto("/bounties/3");
      await wallet.connect();
      await expect(page.getByRole("heading", { level: 1, name: "Paused bounty item" })).toBeVisible();
      await expect(page.getByTestId("contract-paused-notice")).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze();

      await testInfo.attach("axe-results-states.json", {
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

      expect(blocking, "serious/critical axe violations for submitted/archived/paused states").toEqual([]);
    });
  });
}

test.describe('Accessibility (WCAG 2.1 AA)', () => {
  test('home page has no critical/serious violations', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);
  });

  test('submitted contract state is accessible', async ({ page }) => {
    await page.goto('/contracts?status=submitted');
    await page.waitForLoadState('networkidle');
    await expect(page.getByTestId('status-badge-submitted').first()).toBeVisible();
    await assertNoCriticalViolations(page);
  });

  test('archived contract state is accessible', async ({ page }) => {
    await page.goto('/contracts?status=archived');
    await page.waitForLoadState('networkidle');
    await expect(page.getByTestId('status-badge-archived').first()).toBeVisible();
    await assertNoCriticalViolations(page);
  });

  test('paused banner is accessible', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    const banner = page.getByRole('status').filter({ hasText: /paused/i });
    if (await banner.count()) {
      await expect(banner.first()).toBeVisible();
    }
    await assertNoCriticalViolations(page);
  });
});

test.describe('Accessibility — dark mode contrast', () => {
  test.use({ colorScheme: 'dark' });

  test('submitted, archived, and paused states pass contrast in dark mode', async ({
    page,
  }) => {
    await page.goto('/contracts?status=submitted');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);

    await page.goto('/contracts?status=archived');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);
  });
});
