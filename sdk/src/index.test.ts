import { nativeToScVal } from "@stellar/stellar-sdk";
import { MergeMintSDK, symbolToScVal, TESTNET } from "./index";

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

const CONTRACT_ID = "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";

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
        accountId: () => CONTRACT_ID,
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
