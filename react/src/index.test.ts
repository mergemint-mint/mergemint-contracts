import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useBounty, useBounties, useMergeMintTx } from "./index";

const watchBounty = vi.fn();
const unwatchBounty = vi.fn();
const getBounty = vi.fn();
const getBountiesByCreator = vi.fn();
const execute = vi.fn();

vi.mock("@mergemint/sdk", () => ({
  MergeMintClient: vi.fn().mockImplementation(() => ({
    watchBounty,
    unwatchBounty,
    getBounty,
    getBountiesByCreator,
    execute,
  })),
}));

describe("useBounty", () => {
  beforeEach(() => {
    watchBounty.mockReset();
    unwatchBounty.mockReset();
    getBounty.mockReset();
    getBounty.mockResolvedValue({ id: "1" });
    watchBounty.mockReturnValue(() => unwatchBounty());
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("does not resubscribe when inline options object is passed on rerender", async () => {
    const client = {} as never;
    const { rerender } = renderHook(
      ({ id }: { id: string }) =>
        useBounty(client, id, { pollIntervalMs: 1000, sseUrl: "http://x" }),
      { initialProps: { id: "1" } }
    );

    await waitFor(() => expect(watchBounty).toHaveBeenCalledTimes(1));

    rerender({ id: "1" });
    rerender({ id: "1" });

    await waitFor(() => expect(watchBounty).toHaveBeenCalledTimes(1));
    expect(unwatchBounty).not.toHaveBeenCalled();
  });
});

describe("useBounties", () => {
  beforeEach(() => {
    getBountiesByCreator.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetches the initial page with a null cursor", async () => {
    getBountiesByCreator.mockResolvedValue({
      items: [{ id: "1" }, { id: "2" }],
      nextCursor: "c1",
    });
    const client = {} as never;

    const { result } = renderHook(() => useBounties(client, "creator"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(getBountiesByCreator).toHaveBeenCalledWith("creator", null, undefined);
    expect(result.current.bounties).toEqual([{ id: "1" }, { id: "2" }]);
    expect(result.current.nextCursor).toBe("c1");
    expect(result.current.hasMore).toBe(true);
  });

  it("appends the next page on loadMore using the tracked cursor", async () => {
    getBountiesByCreator
      .mockResolvedValueOnce({ items: [{ id: "1" }], nextCursor: "c1" })
      .mockResolvedValueOnce({ items: [{ id: "2" }], nextCursor: null });
    const client = {} as never;

    const { result } = renderHook(() => useBounties(client, "creator"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadMore();
    });

    expect(getBountiesByCreator).toHaveBeenLastCalledWith("creator", "c1", undefined);
    expect(result.current.bounties).toEqual([{ id: "1" }, { id: "2" }]);
    expect(result.current.nextCursor).toBeNull();
    expect(result.current.hasMore).toBe(false);
  });

  it("stops paginating when nextCursor is null", async () => {
    getBountiesByCreator.mockResolvedValue({ items: [{ id: "1" }], nextCursor: null });
    const client = {} as never;

    const { result } = renderHook(() => useBounties(client, "creator"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.loadMore();
    });

    expect(getBountiesByCreator).toHaveBeenCalledTimes(1);
    expect(result.current.hasMore).toBe(false);
  });

  it("recovers from errors and can refetch", async () => {
    getBountiesByCreator
      .mockRejectedValueOnce(new Error("boom"))
      .mockResolvedValueOnce({ items: [{ id: "1" }], nextCursor: null });
    const client = {} as never;

    const { result } = renderHook(() => useBounties(client, "creator"));

    await waitFor(() => expect(result.current.error).toBeInstanceOf(Error));
    expect(result.current.bounties).toEqual([]);

    await act(async () => {
      await result.current.refetch();
    });

    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.bounties).toEqual([{ id: "1" }]);
  });
});

describe("useMergeMintTx", () => {
  beforeEach(() => {
    execute.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("stores the transaction hash on success", async () => {
    execute.mockResolvedValue({ hash: "0xabc" });
    const client = {} as never;

    const { result } = renderHook(() => useMergeMintTx(client));

    expect(result.current.hash).toBeNull();

    await act(async () => {
      await result.current.execute({} as never);
    });

    await waitFor(() => expect(result.current.hash).toBe("0xabc"));
  });
});
