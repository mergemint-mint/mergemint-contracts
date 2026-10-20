import { CONTRIBUTOR, expect, makeBounty, test } from "./fixtures";
import type { Page } from "@playwright/test";

/**
 * End-to-end coverage for the creator-only "Top Up Bounty" and "Extend
 * deadline" flows on the bounty detail page (#1121). Runs against the mocked
 * backend and Freighter wallet in ./fixtures, so no real network is needed.
 *
 * The seeded bounty has a 100 XLM reward and a deadline of 2030-01-01 (UTC).
 */

async function openBounty(page: Page) {
  await page.goto("/bounties/1");
  await expect(page.getByRole("heading", { level: 1, name: "Fix wallet reconnect bug" })).toBeVisible();
}

function reward(page: Page) {
  return page.getByTestId("bounty-reward");
}

test.describe("top up bounty", () => {
  test.beforeEach(async ({ page, api, wallet }) => {
    void api;
    await openBounty(page);
    await wallet.connect();
  });

  test("creator tops up the reward", async ({ page, api }) => {
    await expect(reward(page)).toHaveText("Reward: 100 XLM");

    await page.getByRole("button", { name: "Top Up Bounty" }).click();
    const dialog = page.getByRole("dialog", { name: "Top Up Bounty Reward" });
    await expect(dialog).toBeVisible();

    await dialog.getByLabel("Additional Amount (XLM)").fill("50");
    await expect(dialog.getByTestId("new-total-amount")).toHaveText("150 XLM");
    await dialog.getByRole("button", { name: "Confirm Top Up (150 XLM)" }).click();

    await expect(dialog).toBeHidden();
    await expect(reward(page)).toHaveText("Reward: 150 XLM");
    expect(api.txCalls).toEqual([
      { path: "/tx/top-up-bounty", body: { bountyId: "1", amount: "50", tokenAddress: "XLM" } },
    ]);
  });

  test("wallet rejection keeps the modal open and the reward unchanged", async ({ page, api }) => {
    api.failNextTx("top-up-bounty", "User declined to sign the transaction");

    await page.getByRole("button", { name: "Top Up Bounty" }).click();
    const dialog = page.getByRole("dialog", { name: "Top Up Bounty Reward" });
    await dialog.getByLabel("Additional Amount (XLM)").fill("25");
    await dialog.getByRole("button", { name: /Confirm Top Up/ }).click();

    await expect(dialog.getByRole("alert")).toHaveText("The request was rejected in your wallet.");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Confirm Top Up (125 XLM)" })).toBeEnabled();
    expect(api.bounty().reward).toBe("100");

    // Closing and reopening clears the error; the reward on the page is untouched.
    await dialog.getByRole("button", { name: "Cancel" }).click();
    await expect(dialog).toBeHidden();
    await expect(reward(page)).toHaveText("Reward: 100 XLM");
  });

  test("validates the amount before submitting", async ({ page, api }) => {
    await page.getByRole("button", { name: "Top Up Bounty" }).click();
    const dialog = page.getByRole("dialog", { name: "Top Up Bounty Reward" });
    const amount = dialog.getByLabel("Additional Amount (XLM)");
    const confirm = dialog.getByRole("button", { name: /Confirm Top Up/ });

    await expect(confirm).toBeDisabled();

    await amount.fill("0");
    await expect(dialog.getByRole("alert")).toHaveText("Amount must be greater than 0.");
    await expect(confirm).toBeDisabled();

    await amount.fill("-5");
    await expect(dialog.getByRole("alert")).toHaveText("Amount must be greater than 0.");
    await expect(confirm).toBeDisabled();

    await amount.fill("");
    await expect(dialog.getByRole("alert")).toHaveText("Amount is required.");
    await expect(confirm).toBeDisabled();

    expect(api.txCalls).toHaveLength(0);
  });
});

test.describe("extend deadline", () => {
  test.beforeEach(async ({ page, api, wallet }) => {
    void api;
    await openBounty(page);
    await wallet.connect();
  });

  test("creator extends the deadline", async ({ page, api }) => {
    const form = page.getByRole("form", { name: "Extend deadline" });
    await expect(form.getByText("Current deadline: 1/1/2030")).toBeVisible();

    await form.getByLabel("New deadline").fill("2030-02-01");
    await form.getByRole("button", { name: "Extend deadline" }).click();

    await expect(form.getByText("Transaction submitted")).toBeVisible();
    await expect(form.getByText("Current deadline: 2/1/2030")).toBeVisible();
    expect(api.txCalls).toEqual([
      { path: "/tx/extend-deadline", body: { bountyId: "1", newDeadline: 1_896_134_400 } },
    ]);
  });

  test("wallet rejection shows an error and keeps the old deadline", async ({ page, api }) => {
    api.failNextTx("extend-deadline", "User declined to sign the transaction");
    const form = page.getByRole("form", { name: "Extend deadline" });

    await form.getByLabel("New deadline").fill("2030-03-15");
    await form.getByRole("button", { name: "Extend deadline" }).click();

    await expect(form.getByRole("alert")).toHaveText("The request was rejected in your wallet.");
    await expect(form.getByText("Current deadline: 1/1/2030")).toBeVisible();
    await expect(form.getByRole("button", { name: "Extend deadline" })).toBeEnabled();
    expect(api.bounty().deadline).toBe(1_893_456_000);
  });

  test("rejects a date that is not after the current deadline", async ({ page, api }) => {
    const form = page.getByRole("form", { name: "Extend deadline" });
    const submit = form.getByRole("button", { name: "Extend deadline" });

    await expect(submit).toBeDisabled();

    for (const date of ["2029-12-01", "2030-01-01"]) {
      await form.getByLabel("New deadline").fill(date);
      await expect(form.getByRole("alert")).toHaveText(
        "New deadline must be after the current deadline (2030-01-01).",
      );
      await expect(submit).toBeDisabled();
    }

    expect(api.txCalls).toHaveLength(0);
  });
});

test.describe("wallet and role gating", () => {
  test("rejected wallet connection hides creator controls", async ({ page, api, wallet }) => {
    void api;
    await wallet.rejectConnection();
    await openBounty(page);

    await page.getByRole("button", { name: /connect wallet/i }).click();

    await expect(page.getByRole("alert")).toHaveText("The request was rejected in your wallet.");
    await expect(page.getByRole("button", { name: "Top Up Bounty" })).toHaveCount(0);
    await expect(page.getByRole("form", { name: "Extend deadline" })).toHaveCount(0);
  });

  test("non-creators never see top-up or extend controls", async ({ page, api, wallet }) => {
    api.seed(makeBounty({ creator: CONTRIBUTOR }));
    await openBounty(page);
    await wallet.connect();

    await expect(page.getByRole("button", { name: "Sponsor Bounty" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Top Up Bounty" })).toHaveCount(0);
    await expect(page.getByRole("form", { name: "Extend deadline" })).toHaveCount(0);
  });
});
