import { describe, expect, it } from "vitest";
import { buildOptimisticState, buildConfirmedState, buildFailedState } from "./useTxFlow";
import { generateIdempotencyKey, parseApiErrorBody } from "../lib/api";

describe("useTxFlow state transitions", () => {
  const network = "testnet" as const;

  it("applies an optimistic result immediately on submit", () => {
    const state = buildOptimisticState(network, { hash: "optimistic-pending" });
    expect(state.pending).toBe(true);
    expect(state.optimistic).toBe(true);
    expect(state.error).toBeNull();
    expect(state.result).toEqual({ hash: "optimistic-pending", network });
  });

  it("falls back to a plain pending state when no optimistic result is given", () => {
    const state = buildOptimisticState(network);
    expect(state.pending).toBe(true);
    expect(state.optimistic).toBe(false);
    expect(state.result).toBeNull();
  });

  it("replaces the optimistic result with the confirmed one on success", () => {
    const confirmed = buildConfirmedState({ hash: "real-hash", network, ledger: 42 });
    expect(confirmed.pending).toBe(false);
    expect(confirmed.optimistic).toBe(false);
    expect(confirmed.error).toBeNull();
    expect(confirmed.result).toEqual({ hash: "real-hash", network, ledger: 42 });
  });

  it("rolls back the optimistic result and surfaces an error on failure", () => {
    const failed = buildFailedState("Transaction failed");
    expect(failed.pending).toBe(false);
    expect(failed.optimistic).toBe(false);
    expect(failed.result).toBeNull();
    expect(failed.error).toBe("Transaction failed");
  });

  it("generates unique idempotency keys and parses structured API errors", () => {
    const key1 = generateIdempotencyKey();
    const key2 = generateIdempotencyKey();
    expect(key1).toBeTruthy();
    expect(key1).not.toBe(key2);

    const parsed = parseApiErrorBody(
      JSON.stringify({ message: "Bounty already claimed", code: "ALREADY_CLAIMED" }),
      409
    );
    expect(parsed.message).toBe("Bounty already claimed");
    expect(parsed.status).toBe(409);
    expect(parsed.code).toBe("ALREADY_CLAIMED");
  });
});
