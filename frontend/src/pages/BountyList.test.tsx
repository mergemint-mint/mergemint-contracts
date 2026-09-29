import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import { WalletProvider } from "../lib/WalletContext";
import { Bounty, BountyPage } from "../types";
import { BountyList } from "./BountyList";

const MOCK_BOUNTIES: Bounty[] = [
  {
    id: "bounty-1",
    title: "First Test Bounty",
    description: "First description",
    reward: "100 XLM",
    status: "open",
    creator: "GCREATOR111111111111111111111111111111111111111111111111",
    createdAt: "2026-01-01T00:00:00Z",
    maxAssignees: 1,
    tags: ["stellar"],
    milestones: [],
  },
  {
    id: "bounty-2",
    title: "Second Test Bounty",
    description: "Second description",
    reward: "200 XLM",
    status: "claimed",
    creator: "GCREATOR222222222222222222222222222222222222222222222222",
    createdAt: "2026-01-02T00:00:00Z",
    maxAssignees: 1,
    tags: ["soroban"],
    milestones: [],
  },
];

describe("BountyList component", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders loading skeleton cards while initial page is loading", async () => {
    let resolvePromise: (value: BountyPage) => void;
    const promise = new Promise<BountyPage>((resolve) => {
      resolvePromise = resolve;
    });

    vi.spyOn(api, "getBounties").mockReturnValue(promise);

    render(
      <WalletProvider>
        <BountyList />
      </WalletProvider>,
    );

    const skeletonCards = screen.getAllByLabelText("Loading bounty");
    expect(skeletonCards.length).toBeGreaterThan(0);

    resolvePromise!({
      bounties: [MOCK_BOUNTIES[0]!],
      nextCursor: null,
    });

    await waitFor(() => {
      expect(screen.queryByLabelText("Loading bounty")).toBeNull();
    });

    expect(screen.getByText("100 XLM")).toBeTruthy();
  });

  it("renders fallback load more button when next cursor is available", async () => {
    vi.spyOn(api, "getBounties").mockResolvedValueOnce({
      bounties: [MOCK_BOUNTIES[0]!],
      nextCursor: "next-page-cursor",
    });

    render(
      <WalletProvider>
        <BountyList />
      </WalletProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("100 XLM")).toBeTruthy();
    });

    const loadMoreBtn = screen.getByRole("button", { name: "Load more" });
    expect(loadMoreBtn).toBeTruthy();

    const sentinel = screen.getByTestId("infinite-scroll-sentinel");
    expect(sentinel).toBeTruthy();
  });

  it("clicks load more button to fetch and append subsequent bounties", async () => {
    vi.spyOn(api, "getBounties")
      .mockResolvedValueOnce({
        bounties: [MOCK_BOUNTIES[0]!],
        nextCursor: "cursor-2",
      })
      .mockResolvedValueOnce({
        bounties: [MOCK_BOUNTIES[1]!],
        nextCursor: null,
      });

    render(
      <WalletProvider>
        <BountyList />
      </WalletProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("100 XLM")).toBeTruthy();
    });

    const loadMoreBtn = screen.getByRole("button", { name: "Load more" });
    fireEvent.click(loadMoreBtn);

    await waitFor(() => {
      expect(screen.getByText("200 XLM")).toBeTruthy();
    });

    expect(screen.getByText("100 XLM")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });
});
