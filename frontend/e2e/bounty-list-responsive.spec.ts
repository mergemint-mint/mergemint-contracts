import { expect, test } from "@playwright/test";

/**
 * Confirms the bounty card grid on BountyList reflows to a single column at
 * mobile widths, rather than staying in a multi-column layout.
 *
 * Requires a local dev server (or mocked backend) reachable at baseURL with
 * at least two bounties returned, matching the setup used by happy-path.spec.ts.
 */
test.use({ viewport: { width: 375, height: 812 } });

test("bounty card grid stacks to a single column on mobile", async ({ page }) => {
  await page.goto("/");

  const cards = page.locator(".bounty-grid > *");
  const count = await cards.count();
  test.skip(count < 2, "Needs at least two bounties to assert stacking.");

  const boxes = await Promise.all(
    Array.from({ length: count }, (_, i) => cards.nth(i).boundingBox())
  );

  for (let i = 1; i < boxes.length; i++) {
    expect(boxes[i]!.x).toBeCloseTo(boxes[0]!.x, 0);
    expect(boxes[i]!.y).toBeGreaterThan(boxes[i - 1]!.y);
  }
});
