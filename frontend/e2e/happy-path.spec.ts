import { expect, test } from "@playwright/test";

/**
 * End-to-end happy path: creator posts a bounty, a second wallet claims it,
 * and a verifier completes it. Asserts the UI reflects each status
 * transition (open -> in_progress -> completed).
 *
 * Requires a local dev server (or mocked backend) reachable at baseURL, and
 * two connectable test wallets (creator/verifier and contributor).
 */
test("create bounty -> claim -> complete happy path", async ({ page, context }) => {
  await page.goto("/");

  // 1. Creator posts a bounty.
  await page.getByRole("button", { name: "Connect Wallet" }).click();
  await page.getByRole("button", { name: "Create Bounty" }).click();
  await page.getByLabel("Title").fill("E2E Test Bounty");
  await page.getByLabel("Description").fill("E2E Test Bounty Description");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByLabel("Reward Amount").fill("10");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Submit" }).click();

  await expect(page.getByText("Status: open")).toBeVisible();

  // 2. A second wallet (contributor) claims the bounty.
  const contributorPage = await context.newPage();
  await contributorPage.goto("/");
  await contributorPage.getByRole("button", { name: "Connect Wallet" }).click();
  await contributorPage.getByRole("button", { name: "Claim" }).click();

  await expect(contributorPage.getByText("Status: in_progress")).toBeVisible();
  await expect(page.getByText("Status: in_progress")).toBeVisible();

  // 3. Verifier (creator, acting as verifier) completes the bounty.
  await page.getByRole("button", { name: "Complete Bounty" }).click();

  await expect(page.getByText("Status: completed")).toBeVisible();
  await expect(contributorPage.getByText("Status: completed")).toBeVisible();
});
