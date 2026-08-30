import { describe, expect, it, vi, afterEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { isFreighterInstalled, requestAccess, signTransaction } from "./wallet";
import { WalletProvider, useWallet } from "./WalletContext";

vi.mock("./format", () => ({
  mapErrorMessage: (message: string) => message,
}));

type FreighterMock = {
  requestAccess?: () => Promise<string>;
  signTransaction?: (xdr: string) => Promise<string>;
};

function setFreighter(freighter?: FreighterMock) {
  if (freighter) {
    (window as unknown as { freighter: FreighterMock }).freighter = freighter;
  } else {
    Reflect.deleteProperty(window, "freighter");
  }
}

afterEach(() => {
  setFreighter();
  vi.clearAllMocks();
});

describe("wallet.ts", () => {
  describe("isFreighterInstalled", () => {
    it("returns false when the Freighter extension is absent", async () => {
      setFreighter();
      await expect(isFreighterInstalled()).resolves.toBe(false);
    });

    it("returns true when window.freighter is present", async () => {
      setFreighter({ requestAccess: vi.fn() });
      await expect(isFreighterInstalled()).resolves.toBe(true);
    });
  });

  describe("requestAccess (connect)", () => {
    it("returns the public key on successful connect", async () => {
      const requestAccessMock = vi.fn().mockResolvedValue("GCONNECT1234567890");
      setFreighter({ requestAccess: requestAccessMock });

      await expect(requestAccess()).resolves.toBe("GCONNECT1234567890");
      expect(requestAccessMock).toHaveBeenCalledOnce();
    });

    it("throws when the wallet extension is not installed", async () => {
      setFreighter();
      await expect(requestAccess()).rejects.toThrow(
        "Freighter extension is not installed"
      );
    });
  });

  describe("signTransaction", () => {
    it("returns the signed XDR on success", async () => {
      const signMock = vi.fn().mockResolvedValue("signed-xdr");
      setFreighter({ signTransaction: signMock });

      await expect(signTransaction("unsigned-xdr")).resolves.toBe("signed-xdr");
      expect(signMock).toHaveBeenCalledWith("unsigned-xdr");
    });

    it("throws when the wallet extension is not installed", async () => {
      setFreighter();
      await expect(signTransaction("unsigned-xdr")).rejects.toThrow(
        "Freighter extension is not installed"
      );
    });
  });
});

describe("WalletContext connect/disconnect flow", () => {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <WalletProvider>{children}</WalletProvider>
  );

  it("connect stores the wallet address on success", async () => {
    setFreighter({
      requestAccess: vi.fn().mockResolvedValue("GCONNECTEDWALLET"),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    await act(async () => {
      await result.current.connect();
    });

    expect(result.current.address).toBe("GCONNECTEDWALLET");
    expect(result.current.error).toBeNull();
    expect(result.current.connecting).toBe(false);
  });

  it("disconnect clears the connected address", async () => {
    setFreighter({
      requestAccess: vi.fn().mockResolvedValue("GCONNECTEDWALLET"),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    await act(async () => {
      await result.current.connect();
    });
    expect(result.current.address).toBe("GCONNECTEDWALLET");

    act(() => {
      result.current.disconnect();
    });

    expect(result.current.address).toBeNull();
  });

  it("surfaces wallet-not-found when Freighter is missing", async () => {
    setFreighter();

    const { result } = renderHook(() => useWallet(), { wrapper });

    await act(async () => {
      await result.current.connect();
    });

    expect(result.current.address).toBeNull();
    expect(result.current.error).toBe("Freighter extension is not installed");
    expect(result.current.connecting).toBe(false);
  });
});
