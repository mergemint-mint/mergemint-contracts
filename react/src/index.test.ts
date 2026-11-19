import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useBounty, useMergeMintTx } from "./index";

const watchBounty = vi.fn();
const unwatchBounty = vi.fn();
const getBounty = vi.fn();
const execute = vi.fn();

vi.mock("@mergemint/sdk", () => ({
  MergeMintClient: vi.fn().mockImplementation(() => ({
    watchBounty,
    unwatchBounty,
    getBounty,
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
