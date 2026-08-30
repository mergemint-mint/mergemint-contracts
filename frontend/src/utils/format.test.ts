import { describe, expect, it } from "vitest";
import { formatTokenAmount, toRawTokenAmount, shortenAddress, mapErrorMessage } from "./format";

describe("formatTokenAmount / toRawTokenAmount round-trip", () => {
  const cases: Array<{ raw: string; formatted: string }> = [
    { raw: "0", formatted: "0" }, // edge value: zero
    { raw: "10000000", formatted: "1" }, // whole number
    { raw: "1000000", formatted: "0.1" }, // trailing-zero fraction
    { raw: "12345670", formatted: "1.234567" }, // max-precision fraction (7 dp)
    { raw: "100", formatted: "0.00001" }, // small fraction, leading zero padding
    { raw: "123456789000000000", formatted: "12345678900" }, // very large amount
  ];

  it.each(cases)("formats raw $raw as $formatted", ({ raw, formatted }) => {
    expect(formatTokenAmount(raw)).toBe(formatted);
  });

  it.each(cases)("parses $formatted back to raw $raw", ({ raw, formatted }) => {
    expect(toRawTokenAmount(formatted)).toBe(raw);
  });

  it("round-trips arbitrary raw integers through format -> parse", () => {
    const rawValues = ["0", "1", "9999999", "10000001", "999999999999999"];
    for (const raw of rawValues) {
      expect(toRawTokenAmount(formatTokenAmount(raw))).toBe(raw);
    }
  });
});

describe("shortenAddress", () => {
  it("returns the address unchanged when it fits within lead + trail", () => {
    expect(shortenAddress("GABCDEFGH")).toBe("GABCDEFGH");
  });

  it("shortens a long address using the default 4/4 split", () => {
    expect(shortenAddress("GABCDEFGHIJKLMNOPQRSTUVWXYZ1234")).toBe("GABC…1234");
  });

  it("honors custom lead/trail lengths", () => {
    expect(shortenAddress("GABCDEFGHIJKLMNOPQRSTUVWXYZ1234", 6, 2)).toBe("GABCDE…34");
  });
});

describe("mapErrorMessage", () => {
  it("falls back to a generic message for empty input", () => {
    expect(mapErrorMessage("")).toBe("Something went wrong. Please try again.");
  });

  it("maps known contract error substrings to friendly copy", () => {
    expect(mapErrorMessage("Error: bounty already claimed by another user")).toBe(
      "This bounty has already been claimed by someone else."
    );
    expect(mapErrorMessage("insufficient balance for transfer")).toBe(
      "You don't have enough balance to complete this action."
    );
  });

  it("returns the raw message unchanged when nothing matches", () => {
    expect(mapErrorMessage("some totally unrecognized error")).toBe(
      "some totally unrecognized error"
    );
  });
});
