import { test as base, expect, type Page, type Route } from "@playwright/test";

/**
 * Shared Playwright fixtures for specs that run without a real backend or
 * wallet extension:
 *
 * - `api`: an in-memory mock of the `/api/v1` backend. Bounties live in a map,
 *   and the `/tx/*` endpoints update it, so a successful transaction is
 *   visible after the page refetches. Use `failNextTx` to simulate a
 *   transaction that is rejected (for example, declined in the wallet).
 * - `wallet`: a stand-in for the Freighter extension (`window.freighter`).
 *   It approves connection requests unless `rejectConnection()` is called
 *   before navigating.
 *
 * Specs import `test` and `expect` from here instead of `@playwright/test`.
 */

export const CREATOR = "GCREATORAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
export const CONTRIBUTOR = "GCONTRIBUTORBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

/** 2030-01-01T00:00:00Z. Fixed so date-based assertions never drift. */
export const DEFAULT_DEADLINE = 1_893_456_000;

export interface MockBounty {
  id: string;
  title: string;
  description: string;
  reward: string;
  rewardToken?: string;
  status: "open" | "claimed" | "disputed" | "completed" | "cancelled";
  creator: string;
  assignee?: string;
  createdAt: string;
  maxAssignees: number;
  tags: string[];
  deadline?: number;
  milestones: Array<{ description: string; reward: string; completed: boolean }>;
}

export function makeBounty(overrides: Partial<MockBounty> = {}): MockBounty {
  return {
    id: "1",
    title: "Fix wallet reconnect bug",
    description: "Reconnecting Freighter after a reload drops the session.",
    reward: "100",
    rewardToken: "XLM",
    status: "open",
    creator: CREATOR,
    createdAt: "2026-10-01T00:00:00Z",
    maxAssignees: 1,
    tags: ["bug"],
    deadline: DEFAULT_DEADLINE,
    milestones: [],
    ...overrides,
  };
}

interface TxCall {
  path: string;
  body: Record<string, unknown>;
}

interface TxFailure {
  status: number;
  message: string;
}

export class MockApi {
  readonly bounties = new Map<string, MockBounty>();
  /** Every `/tx/*` request the page made, in order. */
  readonly txCalls: TxCall[] = [];
  private failures = new Map<string, TxFailure>();
  private txCounter = 0;

  constructor(initial: MockBounty[] = [makeBounty()]) {
    for (const b of initial) this.bounties.set(b.id, b);
  }

  /** Replace the seeded bounties. */
  seed(...bounties: MockBounty[]) {
    this.bounties.clear();
    for (const b of bounties) this.bounties.set(b.id, b);
  }

  bounty(id = "1"): MockBounty {
    const b = this.bounties.get(id);
    if (!b) throw new Error(`no mock bounty ${id}`);
    return b;
  }

  /** Make the next call to `/tx/<name>` fail with `message`. */
  failNextTx(name: string, message: string, status = 400) {
    this.failures.set(`/tx/${name}`, { status, message });
  }

  async install(page: Page) {
    await page.route("**/api/v1/**", (route) => this.handle(route));
  }

  private async handle(route: Route) {
    const req = route.request();
    const path = new URL(req.url()).pathname.replace(/^.*\/api\/v1/, "");
    const method = req.method();

    // Server-sent event streams: 204 tells EventSource to stop reconnecting.
    if (path.endsWith("/stream")) return route.fulfill({ status: 204 });

    if (method === "POST" && path.startsWith("/tx/")) return this.handleTx(route, path);

    if (path === "/bounties") {
      const url = new URL(req.url());
      const creator = url.searchParams.get("creator");
      const bounties = [...this.bounties.values()].filter(
        (b) => !creator || b.creator === creator,
      );
      return route.fulfill({ json: { bounties, nextCursor: null } });
    }
    let m = path.match(/^\/bounties\/assignee\/([^/]+)$/);
    if (m) {
      const bounties = [...this.bounties.values()].filter((b) => b.assignee === m![1]);
      return route.fulfill({ json: { bounties, nextCursor: null } });
    }
    m = path.match(/^\/bounties\/([^/]+)\/events$/);
    if (m) return route.fulfill({ json: [] });
    m = path.match(/^\/bounties\/([^/]+)$/);
    if (m) {
      const b = this.bounties.get(decodeURIComponent(m[1]));
      return b
        ? route.fulfill({ json: b })
        : route.fulfill({ status: 404, body: "bounty not found" });
    }
    if (path.startsWith("/contributors/leaderboard")) {
      return route.fulfill({
        json: {
          entries: [
            { rank: 1, address: CONTRIBUTOR, reputation: 120, completedBounties: 9 },
            { rank: 2, address: CREATOR, reputation: 80, completedBounties: 4 },
          ],
        },
      });
    }
    if (path.endsWith("/reputation-history")) return route.fulfill({ json: { entries: [] } });
    m = path.match(/^\/contributors\/([^/]+)$/);
    if (m) {
      return route.fulfill({
        json: {
          address: decodeURIComponent(m[1]),
          reputation: 80,
          completedBounties: 4,
          reputationHistory: [],
        },
      });
    }
    if (path === "/tokens/allowlist") {
      return route.fulfill({ json: [{ address: "XLM", symbol: "XLM", decimals: 7 }] });
    }
    if (path.startsWith("/tokens/")) return route.fulfill({ json: { balance: "1000" } });

    return route.fulfill({ status: 404, body: `no mock for ${method} ${path}` });
  }

  private async handleTx(route: Route, path: string) {
    const body = (route.request().postDataJSON() ?? {}) as Record<string, unknown>;
    this.txCalls.push({ path, body });

    const failure = this.failures.get(path);
    if (failure) {
      this.failures.delete(path);
      return route.fulfill({ status: failure.status, body: failure.message });
    }

    const bounty = this.bounties.get(String(body.bountyId));
    if (!bounty) return route.fulfill({ status: 404, body: "bounty not found" });

    if (path === "/tx/top-up-bounty") {
      bounty.reward = String(Number(bounty.reward) + Number(body.amount));
    } else if (path === "/tx/extend-deadline") {
      bounty.deadline = Number(body.newDeadline);
    } else if (path === "/tx/cancel-bounty") {
      bounty.status = "cancelled";
    }

    this.txCounter += 1;
    return route.fulfill({ json: { hash: `mocktx${String(this.txCounter).padStart(4, "0")}` } });
  }
}

export class MockWallet {
  constructor(
    private readonly page: Page,
    readonly address: string,
  ) {}

  /** Install a Freighter stand-in that approves access for `address`. */
  async install() {
    await this.page.addInitScript((address: string) => {
      (window as unknown as { freighter: unknown }).freighter = {
        requestAccess: async () => address,
        getNetwork: async () => ({ network: "TESTNET" }),
        signTransaction: async (xdr: string) => xdr,
      };
    }, this.address);
  }

  /** Make Freighter refuse the connection request, as if the user clicked "Reject". */
  async rejectConnection() {
    await this.page.addInitScript(() => {
      const w = window as unknown as { freighter: { requestAccess: () => Promise<string> } };
      w.freighter.requestAccess = async () => {
        throw new Error("User declined access");
      };
    });
  }

  /** Click the header's Connect button and wait for the address to appear. */
  async connect() {
    const button = this.page.getByRole("button", { name: /connect wallet/i });
    await button.click();
    await expect(button).toBeHidden();
  }
}

export const test = base.extend<{ api: MockApi; wallet: MockWallet }>({
  api: async ({ page }, use) => {
    const api = new MockApi();
    await api.install(page);
    await use(api);
  },
  wallet: async ({ page }, use) => {
    const wallet = new MockWallet(page, CREATOR);
    await wallet.install();
    await use(wallet);
  },
});

export { expect };
