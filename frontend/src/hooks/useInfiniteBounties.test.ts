import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import { Bounty, BountyPage } from "../types";
import { useInfiniteBounties } from "./useInfiniteBounties";

const MOCK_BOUNTY_1: Bounty = {
  id: "bounty-1",
  title: "Implement feature A",
  description: "First test bounty",
  reward: "100 XLM",
  status: "open",
  creator: "GCREATOR111111111111111111111111111111111111111111111111",
  createdAt: "2026-01-01T00:00:00Z",
  maxAssignees: 1,
  tags: ["soroban"],
  milestones: [],
};

const MOCK_BOUNTY_2: Bounty = {
  id: "bounty-2",
  title: "Implement feature B",
  description: "Second test bounty",
  reward: "200 XLM",
  status: "open",
  creator: "GCREATOR222222222222222222222222222222222222222222222222",
  createdAt: "2026-01-02T00:00:00Z",
  maxAssignees: 1,
  tags: ["stellar"],
  milestones: [],
};

const MOCK_BOUNTY_3: Bounty = {
  id: "bounty-3",
  title: "Implement feature C",
  description: "Third test bounty",
  reward: "300 XLM",
  status: "open",
  creator: "GCREATOR333333333333333333333333333333333333333333333333",
  createdAt: "2026-01-03T00:00:00Z",
  maxAssignees: 1,
  tags: ["rust"],
  milestones: [],
};

describe("useInfiniteBounties", () => {
  let originalIntersectionObserver: typeof IntersectionObserver;
  let observerCallback: IntersectionObserverCallback | null = null;
  let observeSpy: ReturnType<typeof vi.fn>;
  let disconnectSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.restoreAllMocks();
    originalIntersectionObserver = globalThis.IntersectionObserver;

    observeSpy = vi.fn();
    disconnectSpy = vi.fn();
    observerCallback = null;

    globalThis.IntersectionObserver = vi.fn((callback) => {
      observerCallback = callback;
      return {
        observe: observeSpy,
        disconnect: disconnectSpy,
        unobserve: vi.fn(),
        takeRecords: vi.fn().mockReturnValue([]),
        root: null,
        rootMargin: "",
        thresholds: [],
      };
    }) as unknown as typeof IntersectionObserver;
  });

  afterEach(() => {
    globalThis.IntersectionObserver = originalIntersectionObserver;
    vi.restoreAllMocks();
  });

  it("fetches and returns the initial page of bounties", async () => {
    const pageResponse: BountyPage = {
      bounties: [MOCK_BOUNTY_1, MOCK_BOUNTY_2],
      nextCursor: "cursor-token-1",
    };

    vi.spyOn(api, "getBounties").mockResolvedValueOnce(pageResponse);

    const { result } = renderHook(() => useInfiniteBounties());

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.bounties).toEqual([MOCK_BOUNTY_1, MOCK_BOUNTY_2]);
    expect(result.current.hasMore).toBe(true);
    expect(result.current.nextCursor).toBe("cursor-token-1");
    expect(result.current.error).toBeNull();
  });

  it("loads subsequent pages and appends bounties when loadMore is triggered", async () => {
    const firstPage: BountyPage = {
      bounties: [MOCK_BOUNTY_1],
      nextCursor: "cursor-token-1",
    };

    const secondPage: BountyPage = {
      bounties: [MOCK_BOUNTY_2],
      nextCursor: null,
    };

    const getBountiesSpy = vi
      .spyOn(api, "getBounties")
      .mockResolvedValueOnce(firstPage)
      .mockResolvedValueOnce(secondPage);

    const { result } = renderHook(() => useInfiniteBounties());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.bounties).toHaveLength(1);
    expect(result.current.hasMore).toBe(true);

    await act(async () => {
      await result.current.loadMore();
    });

    expect(getBountiesSpy).toHaveBeenCalledTimes(2);
    expect(getBountiesSpy).toHaveBeenLastCalledWith({
      status: undefined,
      cursor: "cursor-token-1",
      limit: undefined,
    });
    expect(result.current.bounties).toEqual([MOCK_BOUNTY_1, MOCK_BOUNTY_2]);
    expect(result.current.hasMore).toBe(false);
    expect(result.current.nextCursor).toBeNull();
  });

  it("handles empty page responses", async () => {
    vi.spyOn(api, "getBounties").mockResolvedValueOnce({
      bounties: [],
      nextCursor: null,
    });

    const { result } = renderHook(() => useInfiniteBounties());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.bounties).toEqual([]);
    expect(result.current.hasMore).toBe(false);
    expect(result.current.nextCursor).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("captures and surfaces mapped error messages on fetch failure", async () => {
    vi.spyOn(api, "getBounties").mockRejectedValueOnce(new Error("Network failure"));

    const { result } = renderHook(() => useInfiniteBounties());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.error).toBeTruthy();
    expect(result.current.bounties).toEqual([]);
  });

  it("routes query to getBountiesByCreator when ownership filter is created", async () => {
    const creatorSpy = vi.spyOn(api, "getBountiesByCreator").mockResolvedValueOnce({
      bounties: [MOCK_BOUNTY_1],
      nextCursor: null,
    });

    renderHook(() =>
      useInfiniteBounties({
        ownership: "created",
        address: "GCREATOR111111111111111111111111111111111111111111111111",
      }),
    );

    await waitFor(() => {
      expect(creatorSpy).toHaveBeenCalledWith(
        "GCREATOR111111111111111111111111111111111111111111111111",
        { status: undefined, cursor: undefined, limit: undefined },
      );
    });
  });

  it("routes query to getBountiesByAssignee when ownership filter is assigned", async () => {
    const assigneeSpy = vi.spyOn(api, "getBountiesByAssignee").mockResolvedValueOnce({
      bounties: [MOCK_BOUNTY_2],
      nextCursor: null,
    });

    renderHook(() =>
      useInfiniteBounties({
        ownership: "assigned",
        address: "GASSIGNEE11111111111111111111111111111111111111111111111",
      }),
    );

    await waitFor(() => {
      expect(assigneeSpy).toHaveBeenCalledWith(
        "GASSIGNEE11111111111111111111111111111111111111111111111",
        { status: undefined, cursor: undefined, limit: undefined },
      );
    });
  });

  it("triggers loadMore when IntersectionObserver reports sentinel intersection", async () => {
    const firstPage: BountyPage = {
      bounties: [MOCK_BOUNTY_1],
      nextCursor: "cursor-token-1",
    };
    const secondPage: BountyPage = {
      bounties: [MOCK_BOUNTY_2],
      nextCursor: null,
    };

    const getBountiesSpy = vi
      .spyOn(api, "getBounties")
      .mockResolvedValueOnce(firstPage)
      .mockResolvedValueOnce(secondPage);

    const { result } = renderHook(() => useInfiniteBounties());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    const sentinelElement = document.createElement("div");

    act(() => {
      result.current.sentinelRef(sentinelElement);
    });

    expect(observeSpy).toHaveBeenCalledWith(sentinelElement);
    expect(observerCallback).not.toBeNull();

    await act(async () => {
      if (observerCallback) {
        observerCallback(
          [
            {
              isIntersecting: true,
              target: sentinelElement,
              intersectionRatio: 1,
              boundingClientRect: sentinelElement.getBoundingClientRect(),
              intersectionRect: sentinelElement.getBoundingClientRect(),
              rootBounds: null,
              time: Date.now(),
            } as IntersectionObserverEntry,
          ],
          {} as IntersectionObserver,
        );
      }
    });

    await waitFor(() => {
      expect(getBountiesSpy).toHaveBeenCalledTimes(2);
    });

    expect(result.current.bounties).toEqual([MOCK_BOUNTY_1, MOCK_BOUNTY_2]);
  });

  it("does not trigger loadMore when nextCursor is null or already loading", async () => {
    const getBountiesSpy = vi.spyOn(api, "getBounties").mockResolvedValueOnce({
      bounties: [MOCK_BOUNTY_1],
      nextCursor: null,
    });

    const { result } = renderHook(() => useInfiniteBounties());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.loadMore();
    });

    expect(getBountiesSpy).toHaveBeenCalledTimes(1);
  });

  it("resets list and re-fetches from first page when filters change", async () => {
    const allPage: BountyPage = {
      bounties: [MOCK_BOUNTY_1, MOCK_BOUNTY_2],
      nextCursor: "cursor-all",
    };
    const openPage: BountyPage = {
      bounties: [MOCK_BOUNTY_3],
      nextCursor: null,
    };

    const getBountiesSpy = vi
      .spyOn(api, "getBounties")
      .mockResolvedValueOnce(allPage)
      .mockResolvedValueOnce(openPage);

    const { result, rerender } = renderHook(
      (props: { status: "all" | "open" }) => useInfiniteBounties({ status: props.status }),
      { initialProps: { status: "all" } },
    );

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.bounties).toHaveLength(2);

    rerender({ status: "open" });

    await waitFor(() => {
      expect(result.current.bounties).toEqual([MOCK_BOUNTY_3]);
    });

    expect(getBountiesSpy).toHaveBeenCalledTimes(2);
    expect(result.current.nextCursor).toBeNull();
  });
});
