import { MergeMintSDK, TESTNET, MAINNET, createNetworkConfig } from "./index";

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
          rpcUrl: MAINNET.rpcUrl,
          networkPassphrase: MAINNET.networkPassphrase,
          contractId: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        }),
    ).toThrow("placeholder detected");
  });

  it("should successfully instantiate with valid config", () => {
    const config = createNetworkConfig(
      TESTNET,
      "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    const sdk = new MergeMintSDK(config);
    expect(sdk).toBeDefined();
  });
});
