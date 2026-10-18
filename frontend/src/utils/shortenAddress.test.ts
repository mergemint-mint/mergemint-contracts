import { describe, expect, it } from "vitest";
import { shortenAddress } from "./format";

// Default split is 4 leading + 4 trailing characters (see
// README-format-consolidation.md). Shortening only happens when it actually
// saves space, i.e. the address is longer than lead + trail + 1 (ellipsis).
describe("shortenAddress boundary cases", () => {
  it("returns addresses of 9 characters or fewer unchanged", () => {
    expect(shortenAddress("GABC1")).toBe("GABC1");
    expect(shortenAddress("GABCD1234")).toBe("GABCD1234"); // 9 chars
  });

  it("shortens addresses of 10 characters or more", () => {
    expect(shortenAddress("GABCDE1234")).toBe("GABC…1234"); // 10 chars
  });

  it("applies the same boundary to custom lead/trail lengths", () => {
    expect(shortenAddress("GABCDE1234567", 6, 6)).toBe("GABCDE1234567"); // 13 chars
    expect(shortenAddress("GABCDE12345678", 6, 6)).toBe("GABCDE…345678"); // 14 chars
  });
});

describe("shortenAddress Stellar address formats", () => {
  it("shortens a full Stellar account address (G..., 56 chars)", () => {
    const address = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";
    expect(address).toHaveLength(56);
    expect(shortenAddress(address)).toBe("GBZX…MADI");
  });

  it("shortens a full Stellar/Soroban contract address (C..., 56 chars)", () => {
    const address = "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJNDDXV3CHKICUQIJQ3M";
    expect(address).toHaveLength(56);
    expect(shortenAddress(address)).toBe(`${address.slice(0, 4)}…${address.slice(-4)}`);
  });
});
