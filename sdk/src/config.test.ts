import { MergeMintSDK, TESTNET, MAINNET, createNetworkConfig } from "./index";
import { validateRpcUrl } from "./validation";

describe("MergeMintSDK constructor & configuration", () => {
  it("should throw error if contractId is missing or empty", () => {
    expect(
      () =>
        new MergeMintSDK({
          rpcUrl: "https://soroban-testnet.stellar.org",
          networkPassphrase: "Test SDF Network ; September 2015",
          contractId: "",
        }),
    ).toThrow("Invalid contractId");
  });

  it("should throw error if RPC URL contains placeholder", () => {
    expect(
      () =>
        new MergeMintSDK({
          rpcUrl: "https://mainnet.stellar.validationcloud.io/v1/XCa...",
          networkPassphrase: MAINNET.networkPassphrase,
          contractId: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
        }),
    ).toThrow("placeholder detected");
  });

  it("should successfully instantiate with valid config", () => {
    const config = createNetworkConfig(
      TESTNET,
      "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
    );
    const sdk = new MergeMintSDK(config);
    expect(sdk).toBeDefined();
  });
});

describe("validateRpcUrl", () => {
  it("should reject placeholder/ellipsis URLs", () => {
    expect(() =>
      validateRpcUrl("https://mainnet.stellar.validationcloud.io/v1/XCa..."),
    ).toThrow(/placeholder detected/);
    expect(() => validateRpcUrl("https://example.com/...")).toThrow(
      /placeholder detected/,
    );
  });

  it("should reject non-HTTPS schemes for non-localhost URLs", () => {
    expect(() => validateRpcUrl("http://soroban-testnet.stellar.org")).toThrow(
      /HTTPS/,
    );
    expect(() => validateRpcUrl("ftp://soroban-testnet.stellar.org")).toThrow(
      /HTTPS/,
    );
  });

  it("should allow HTTP for localhost", () => {
    expect(() => validateRpcUrl("http://localhost:8000")).not.toThrow();
    expect(() => validateRpcUrl("http://127.0.0.1:8000")).not.toThrow();
  });

  it("should accept valid testnet and mainnet URLs", () => {
    expect(() => validateRpcUrl(TESTNET.rpcUrl)).not.toThrow();
    expect(() => validateRpcUrl(MAINNET.rpcUrl)).not.toThrow();
  });
});
