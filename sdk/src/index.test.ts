import {
  symbolToScVal,
  buildNetworkConfig,
  MergeMintSDK,
  MergeMintSdkError,
  TESTNET,
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

describe("MergeMintSDK contractId validation", () => {
  it("throws a clear error for an empty contractId", () => {
    let caught: unknown;
    try {
      new MergeMintSDK({ ...TESTNET, contractId: "" });
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MergeMintSdkError);
    expect((caught as MergeMintSdkError).code).toBe("MISSING_CONTRACT_ID");
    expect((caught as MergeMintSdkError).message).toMatch(/Invalid contractId/);
  });

  it("throws for a whitespace-only contractId", () => {
    expect(() => new MergeMintSDK({ ...TESTNET, contractId: "   " })).toThrow(
      /Invalid contractId/,
    );
  });

  it("throws for a placeholder contractId", () => {
    expect(
      () => new MergeMintSDK({ ...TESTNET, contractId: "CXXXXXXXXXXXX" }),
    ).toThrow(/Invalid contractId/);
    expect(
      () => new MergeMintSDK({ ...TESTNET, contractId: "CABC..." }),
    ).toThrow(/Invalid contractId/);
  });
});

describe("buildNetworkConfig", () => {
  it("defaults to the testnet rpc url and passphrase", () => {
    expect(buildNetworkConfig()).toEqual({
      rpcUrl: TESTNET.rpcUrl,
      networkPassphrase: TESTNET.networkPassphrase,
      contractId: "",
    });
  });

  it("applies overrides on top of the defaults", () => {
    const config = buildNetworkConfig({
      rpcUrl: "https://your-rpc.example.com",
      contractId: "CABCDEF",
    });
    expect(config.rpcUrl).toBe("https://your-rpc.example.com");
    expect(config.contractId).toBe("CABCDEF");
    expect(config.networkPassphrase).toBe(TESTNET.networkPassphrase);
  });

  it("overrides every field when all are supplied", () => {
    const config = buildNetworkConfig({
      rpcUrl: "https://your-rpc.example.com",
      networkPassphrase: "Custom Stellar Network ; January 2025",
      contractId: "CABCDEF",
    });
    expect(config).toEqual({
      rpcUrl: "https://your-rpc.example.com",
      networkPassphrase: "Custom Stellar Network ; January 2025",
      contractId: "CABCDEF",
    });
  });
});
