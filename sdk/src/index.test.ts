import {
  symbolToScVal,
  MergeMintSDK,
  MergeMintSdkError,
  MAINNET,
} from "./index";

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

describe("MergeMintSdkError", () => {
  it("tags an oversized Symbol with the SYMBOL_TOO_LONG code", () => {
    let caught: unknown;
    try {
      symbolToScVal("a".repeat(33));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MergeMintSdkError);
    expect((caught as MergeMintSdkError).code).toBe("SYMBOL_TOO_LONG");
    expect((caught as MergeMintSdkError).name).toBe("MergeMintSdkError");
  });

  it("tags a placeholder RPC URL with the INVALID_RPC_URL code", () => {
    let caught: unknown;
    try {
      new MergeMintSDK({ ...MAINNET, contractId: "C".repeat(56) });
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MergeMintSdkError);
    expect((caught as MergeMintSdkError).code).toBe("INVALID_RPC_URL");
  });
});
