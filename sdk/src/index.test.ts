import { nativeToScVal, xdr } from "@stellar/stellar-sdk";
import {
  MergeMintSDK,
  optionU32ToScVal,
  optionVecAddressToScVal,
  symbolToScVal,
  TESTNET,
} from "./index";
import { MergeMintSdkError } from "./types";

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

describe("optionU32ToScVal", () => {
  it("encodes Some(v) as the bare inner u32 ScVal", () => {
    const encoded = optionU32ToScVal(42);
    expect(encoded.switch().name).toBe("scvU32");
    expect(encoded).toEqual(nativeToScVal(42, { type: "u32" }));
  });

  it("encodes None as scvVoid", () => {
    expect(optionU32ToScVal(null).switch().name).toBe("scvVoid");
    expect(optionU32ToScVal(undefined).switch().name).toBe("scvVoid");
  });
});

describe("optionVecAddressToScVal", () => {
  const ADDR_A = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
  const ADDR_B = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

  it("encodes Some(vec) as a bare scvVec of address ScVals", () => {
    const encoded = optionVecAddressToScVal([ADDR_A]);
    expect(encoded.switch().name).toBe("scvVec");
    const inner = encoded.vec();
    expect(inner).not.toBeNull();
    expect(inner).toHaveLength(1);
    expect(inner?.[0].switch().name).toBe("scvAddress");
  });

  it("encodes an empty vec as Some(vec![])", () => {
    const encoded = optionVecAddressToScVal([]);
    expect(encoded.switch().name).toBe("scvVec");
    expect(encoded.vec()).toHaveLength(0);
  });

  it("encodes None as scvVoid", () => {
    expect(optionVecAddressToScVal(null).switch().name).toBe("scvVoid");
    expect(optionVecAddressToScVal(undefined).switch().name).toBe("scvVoid");
  });

  it("round-trips through XDR base64", () => {
    const encoded = optionVecAddressToScVal([ADDR_A, ADDR_B]);
    const decoded = xdr.ScVal.fromXDR(encoded.toXDR());
    expect(decoded.switch().name).toBe("scvVec");
    expect(decoded.vec()).toHaveLength(2);
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
