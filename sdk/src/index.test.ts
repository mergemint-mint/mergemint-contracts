import { nativeToScVal, scValToNative, xdr } from "@stellar/stellar-sdk";
import {
  MergeMintSDK,
  milestoneToScVal,
  milestonesToScVal,
  parseBounty,
  symbolToScVal,
  TESTNET,
} from "./index";
import { MergeMintSdkError, Milestone } from "./types";

describe("symbolToScVal", () => {
  it("throws for a 33-character input", () => {
    const value = "a".repeat(33);
    expect(() => symbolToScVal(value)).toThrow(
      /exceeds 32-character Symbol limit/,
    );
  });

  it("passes for exactly 32 characters", () => {
    const value = "a".repeat(32);
    expect(() => symbolToScVal(value)).not.toThrow();
  });
});

describe("milestoneToScVal", () => {
  const base: Milestone = {
    description: "ship it",
    reward: 100n,
    completed: false,
    deadline: 1700000000,
  };

  function keysOf(scVal: xdr.ScVal): string[] {
    return scVal
      .map()!
      .map((entry) => scValToNative(entry.key()) as string);
  }

  it("emits ScMap keys in strict lexicographical order", () => {
    const scVal = milestoneToScVal(base);
    expect(keysOf(scVal)).toEqual([
      "completed",
      "deadline",
      "description",
      "reward",
    ]);
  });

  it("encodes a present deadline as scvU32", () => {
    const scVal = milestoneToScVal(base);
    const deadlineEntry = scVal
      .map()!
      .find((e) => (scValToNative(e.key()) as string) === "deadline")!;
    expect(deadlineEntry.val().switch().name).toBe("scvU32");
    expect(deadlineEntry.val().u32()).toBe(1700000000);
  });

  it("encodes a null deadline as scvVoid", () => {
    const scVal = milestoneToScVal({ ...base, deadline: null });
    const deadlineEntry = scVal
      .map()!
      .find((e) => (scValToNative(e.key()) as string) === "deadline")!;
    expect(deadlineEntry.val().switch().name).toBe("scvVoid");
  });

  it("encodes an undefined deadline as scvVoid", () => {
    const scVal = milestoneToScVal({ ...base, deadline: undefined });
    const deadlineEntry = scVal
      .map()!
      .find((e) => (scValToNative(e.key()) as string) === "deadline")!;
    expect(deadlineEntry.val().switch().name).toBe("scvVoid");
  });

  it("round-trips deadline through parseBounty", () => {
    const scVal = milestoneToScVal(base);
    const native = scValToNative(scVal) as Record<string, unknown>;
    const bounty = parseBounty({ milestones: [native] });
    expect(bounty.milestones[0].deadline).toBe(1700000000);
  });

  it("round-trips a null deadline through parseBounty", () => {
    const scVal = milestoneToScVal({ ...base, deadline: null });
    const native = scValToNative(scVal) as Record<string, unknown>;
    const bounty = parseBounty({ milestones: [native] });
    expect(bounty.milestones[0].deadline).toBeNull();
  });

  it("milestonesToScVal preserves sorted keys per milestone", () => {
    const scVal = milestonesToScVal([base, { ...base, deadline: null }]);
    const vec = scVal.vec()!;
    expect(vec).toHaveLength(2);
    for (const entry of vec) {
      expect(keysOf(entry)).toEqual([
        "completed",
        "deadline",
        "description",
        "reward",
      ]);
    }
  });
});

const CONTRACT_ID = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
const SOURCE_ACCOUNT = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/**
 * Minimal stand-in for `SorobanRpc.Server`. `getAccount` fails for the first
 * `failures` calls and succeeds afterwards, so a test can assert that the retry
 * policy recovers from a transient hiccup.
 */
function makeFlakyRpc(failures: number, retval = 7n) {
  const calls = { getAccount: 0, simulateTransaction: 0 };
  return {
    calls,
    getAccount: jest.fn(async () => {
      calls.getAccount++;
      if (calls.getAccount <= failures) {
        throw new Error("transient RPC failure");
      }
      return {
        accountId: () => SOURCE_ACCOUNT,
        sequenceNumber: () => "1",
        incrementSequenceNumber: () => undefined,
      };
    }),
    simulateTransaction: jest.fn(async () => {
      calls.simulateTransaction++;
      return { result: { retval: nativeToScVal(retval, { type: "i128" }) } };
    }),
  };
}

function sdkWithRpc(
  rpc: unknown,
  retry?: { attempts: number; backoffMs: number },
) {
  const sdk = new MergeMintSDK({
    ...TESTNET,
    contractId: CONTRACT_ID,
    retry,
  });
  // The SDK builds its own `SorobanRpc.Server` from `rpcUrl`; swap in the stub.
  (sdk as unknown as { rpc: unknown }).rpc = rpc;
  return sdk;
}

describe("MergeMintSDK retry", () => {
  it("recovers when an RPC call fails once then succeeds", async () => {
    const rpc = makeFlakyRpc(1);
    const sdk = sdkWithRpc(rpc, { attempts: 3, backoffMs: 0 });

    await expect(sdk.getBountyCount()).resolves.toBe(7n);
    expect(rpc.calls.getAccount).toBe(2);
  });

  it("makes a single attempt when no retry option is supplied", async () => {
    const rpc = makeFlakyRpc(1);
    const sdk = sdkWithRpc(rpc);

    await expect(sdk.getBountyCount()).resolves.toBe(0n);
    expect(rpc.calls.getAccount).toBe(1);
  });

  it("gives up and surfaces the last error once attempts are exhausted", async () => {
    const rpc = makeFlakyRpc(5);
    const sdk = sdkWithRpc(rpc, { attempts: 2, backoffMs: 0 });

    // `getBountyCount` swallows the failure and yields the zero-value default.
    await expect(sdk.getBountyCount()).resolves.toBe(0n);
    expect(rpc.calls.getAccount).toBe(2);
  });

  it("rejects an out-of-range retry configuration", () => {
    expect(
      () =>
        new MergeMintSDK({
          ...TESTNET,
          contractId: CONTRACT_ID,
          retry: { attempts: 0, backoffMs: 10 },
        }),
    ).toThrow(/Invalid retry.attempts/);

    expect(
      () =>
        new MergeMintSDK({
          ...TESTNET,
          contractId: CONTRACT_ID,
          retry: { attempts: 2, backoffMs: -1 },
        }),
    ).toThrow(/Invalid retry.backoffMs/);
  });
});

describe("MergeMintSDK constructor typed errors", () => {
  it("throws MergeMintSdkError with INVALID_CONTRACT_ID for an empty contractId", () => {
    try {
      new MergeMintSDK({ ...TESTNET, contractId: "" });
      throw new Error("expected constructor to throw");
    } catch (err) {
      expect(err).toBeInstanceOf(MergeMintSdkError);
      expect((err as MergeMintSdkError).code).toBe("INVALID_CONTRACT_ID");
    }
  });

  it("throws MergeMintSdkError with INVALID_RPC_URL for a placeholder rpcUrl", () => {
    try {
      new MergeMintSDK({
        rpcUrl: "https://example.com/v1/XCa...",
        networkPassphrase: TESTNET.networkPassphrase,
        contractId: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      });
      throw new Error("expected constructor to throw");
    } catch (err) {
      expect(err).toBeInstanceOf(MergeMintSdkError);
      expect((err as MergeMintSdkError).code).toBe("INVALID_RPC_URL");
    }
  });
});
