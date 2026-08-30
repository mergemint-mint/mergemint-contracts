import { expect, test } from "@playwright/test";

/**
 * End-to-end dispute flow: a contributor claims a bounty, either party
 * raises a dispute, and the creator (acting as arbitrator, per
 * mergemint-backend's `resolve_dispute`) resolves it. Asserts the UI
 * reflects each status transition (open -> claimed -> disputed -> completed).
 *
 * This complements happy-path.spec.ts (issue #522), which only covers the
 * create/claim/complete flow and never exercises `raise_dispute` /
 * `resolve_dispute`.
 *
 * Requires a local dev server (or mocked backend) reachable at baseURL, and
 * two connectable test wallets (creator/arbitrator and contributor).
 */
test("claim -> raise dispute -> resolve dispute flow", async ({ page, context }) => {
  await page.goto("/");

  // 1. Creator posts a bounty.
  await page.getByRole("button", { name: "Connect Wallet" }).click();
  await page.getByRole("button", { name: "Create Bounty" }).click();
  await page.getByLabel("Title").fill("E2E Dispute Test Bounty");
  await page.getByLabel("Reward Amount").fill("10");
  await page.getByRole("button", { name: "Submit" }).click();

  await expect(page.getByText("Status: open")).toBeVisible();

  // 2. A second wallet (contributor) claims the bounty.
  const contributorPage = await context.newPage();
  await contributorPage.goto("/");
  await contributorPage.getByRole("button", { name: "Connect Wallet" }).click();
  await contributorPage.getByRole("button", { name: "Claim" }).click();

  await expect(contributorPage.getByText("Status: claimed")).toBeVisible();
  await expect(page.getByText("Status: claimed")).toBeVisible();

  // 3. The contributor raises a dispute (e.g. the creator went unresponsive).
  await contributorPage.getByRole("button", { name: "Raise Dispute" }).click();

  await expect(contributorPage.getByText("Status: disputed")).toBeVisible();
  await expect(page.getByText("Status: disputed")).toBeVisible();

  // 4. The creator, acting as arbitrator, resolves the dispute in favor of
  // the contributor. mergemint-backend's resolve_dispute only allows the
  // bounty creator to act as arbitrator.
  await page.getByRole("button", { name: "Resolve Dispute" }).click();
  await page.getByLabel("Winner").fill("contributor");
  await page.getByRole("button", { name: "Confirm Resolution" }).click();

  await expect(page.getByText("Status: completed")).toBeVisible();
  await expect(contributorPage.getByText("Status: completed")).toBeVisible();
});
