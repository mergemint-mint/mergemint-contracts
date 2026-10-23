import { describe, expect, it, vi, afterEach, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { isFreighterInstalled, requestAccess, signTransaction } from "./wallet";
import { WalletProvider, useWallet, WALLET_TYPE_STORAGE_KEY } from "./WalletContext";

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
  localStorage.clear();
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

  it("connect persists the wallet type to localStorage", async () => {
    setFreighter({
      requestAccess: vi.fn().mockResolvedValue("GCONNECTEDWALLET"),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    await act(async () => {
      await result.current.connect();
    });

    expect(localStorage.getItem(WALLET_TYPE_STORAGE_KEY)).toBe("freighter");
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

  it("disconnect removes the wallet type from localStorage", async () => {
    setFreighter({
      requestAccess: vi.fn().mockResolvedValue("GCONNECTEDWALLET"),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    await act(async () => {
      await result.current.connect();
    });
    expect(localStorage.getItem(WALLET_TYPE_STORAGE_KEY)).toBe("freighter");

    act(() => {
      result.current.disconnect();
    });

    expect(localStorage.getItem(WALLET_TYPE_STORAGE_KEY)).toBeNull();
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

describe("WalletContext session restore", () => {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <WalletProvider>{children}</WalletProvider>
  );

  beforeEach(() => {
    localStorage.clear();
  });

  it("silently restores the session when a stored wallet type is found and the wallet authorises", async () => {
    // Pre-seed localStorage as if the user had connected before.
    localStorage.setItem(WALLET_TYPE_STORAGE_KEY, "freighter");
    setFreighter({
      requestAccess: vi.fn().mockResolvedValue("GRESTOREDADDRESS"),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    // Wait for the async restore effect to complete.
    await waitFor(() => {
      expect(result.current.restoring).toBe(false);
    });

    expect(result.current.address).toBe("GRESTOREDADDRESS");
    expect(result.current.walletType).toBe("freighter");
    expect(result.current.error).toBeNull();
  });

  it("falls back to disconnected state when restore fails (wallet denies access)", async () => {
    localStorage.setItem(WALLET_TYPE_STORAGE_KEY, "freighter");
    setFreighter({
      requestAccess: vi.fn().mockRejectedValue(new Error("User rejected")),
    });

    const { result } = renderHook(() => useWallet(), { wrapper });

    await waitFor(() => {
      expect(result.current.restoring).toBe(false);
    });

    // Address should be null and no error surfaced to the user.
    expect(result.current.address).toBeNull();
    expect(result.current.walletType).toBeNull();
    expect(result.current.error).toBeNull();
    // Stale storage key should be cleaned up.
    expect(localStorage.getItem(WALLET_TYPE_STORAGE_KEY)).toBeNull();
  });

  it("falls back to disconnected state when Freighter extension is missing on restore", async () => {
    localStorage.setItem(WALLET_TYPE_STORAGE_KEY, "freighter");
    // No Freighter extension present.
    setFreighter();

    const { result } = renderHook(() => useWallet(), { wrapper });

    await waitFor(() => {
      expect(result.current.restoring).toBe(false);
    });

    expect(result.current.address).toBeNull();
    expect(result.current.walletType).toBeNull();
    expect(result.current.error).toBeNull();
    expect(localStorage.getItem(WALLET_TYPE_STORAGE_KEY)).toBeNull();
  });

  it("skips restore when no wallet type is stored", async () => {
    // Nothing in localStorage — restoring should never flip to true.
    const { result } = renderHook(() => useWallet(), { wrapper });

    // Give React a tick to run effects.
    await act(async () => {});

    expect(result.current.restoring).toBe(false);
    expect(result.current.address).toBeNull();
  });
});
