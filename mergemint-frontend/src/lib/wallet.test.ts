import { afterEach, describe, expect, it } from "vitest";

import { isFreighterInstalled, requestAccess, signTransaction } from "./wallet";
import type { FreighterApi } from "./wallet";

afterEach(() => {
  delete window.freighter;
});

describe("isFreighterInstalled", () => {
  it("returns false when the extension has not injected window.freighter", async () => {
    expect(await isFreighterInstalled()).toBe(false);
  });

  it("returns true once window.freighter is present", async () => {
    window.freighter = {
      requestAccess: async () => "GABC",
      signTransaction: async (xdr) => xdr,
    };
    expect(await isFreighterInstalled()).toBe(true);
  });
});

describe("requestAccess", () => {
  it("resolves with the public key on success", async () => {
    const mock: FreighterApi = {
      requestAccess: async () => "GCONNECTEDADDRESS",
      signTransaction: async (xdr) => xdr,
    };
    window.freighter = mock;

    await expect(requestAccess()).resolves.toBe("GCONNECTEDADDRESS");
  });

  it("rejects with a clear error when the extension is not installed", async () => {
    await expect(requestAccess()).rejects.toThrow("Freighter extension is not installed");
  });

  it("propagates a rejection from the extension (e.g. the user declining)", async () => {
    window.freighter = {
      requestAccess: async () => {
        throw new Error("User declined access");
      },
      signTransaction: async (xdr) => xdr,
    };

    await expect(requestAccess()).rejects.toThrow("User declined access");
  });
});

describe("signTransaction", () => {
  it("delegates to the extension and returns the signed XDR", async () => {
    window.freighter = {
      requestAccess: async () => "GABC",
      signTransaction: async (xdr) => `signed:${xdr}`,
    };

    await expect(signTransaction("raw-xdr")).resolves.toBe("signed:raw-xdr");
  });

  it("rejects with a clear error when the extension is not installed", async () => {
    await expect(signTransaction("raw-xdr")).rejects.toThrow(
      "Freighter extension is not installed"
    );
  });
});
