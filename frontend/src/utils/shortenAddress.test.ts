import { describe, expect, it } from "vitest";
import { shortenAddress } from "./format";

describe("shortenAddress boundary cases", () => {
  it("returns addresses shorter than 12 characters unchanged", () => {
    const address = "GABCDE1234"; // 10 chars
    expect(shortenAddress(address)).toBe(address);
  });

  it("returns addresses exactly 12 characters unchanged", () => {
    const address = "GABCDE123456"; // 12 chars
    expect(address).toHaveLength(12);
    expect(shortenAddress(address)).toBe(address);
  });

  it("shortens addresses longer than 12 characters", () => {
    const address = "GABCDE1234567"; // 13 chars
    expect(shortenAddress(address)).toBe("GABCDE…4567");
  });
});

describe("shortenAddress Stellar address formats", () => {
  it("shortens a full Stellar account address (G..., 56 chars)", () => {
    const address = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";
    expect(address).toHaveLength(56);
    expect(shortenAddress(address)).toBe("GBZXN7…MADI");
  });

  it("shortens a full Stellar/Soroban contract address (C..., 56 chars)", () => {
    const address = "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJNDDXV3CHKICUQIJQ3M";
    expect(address).toHaveLength(56);
    expect(shortenAddress(address)).toBe(`${address.slice(0, 6)}…${address.slice(-4)}`);
  });
});
