/**
 * Tests for NetworkMismatchBanner — the dismissible alert that warns users
 * when their wallet is connected to a different network than the app expects.
 *
 * Strategy: mock both useWallet (to control the connected address) and
 * useNetworkMismatch (to inject the mismatch state) so the tests exercise the
 * banner's rendering and dismiss logic without touching real wallet APIs or
 * network polling.
 *
 * Covers: matching network, mismatched network, disconnected wallet, dismiss
 * interaction, and banner re-arm behaviour.
 *
 * Resolves #918.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// ---- module mocks (hoisted so imports see the mocked versions) -------------
vi.mock("../lib/WalletContext", () => ({
  useWallet: vi.fn(),
}));

vi.mock("../hooks/useNetworkMismatch", () => ({
  useNetworkMismatch: vi.fn(),
}));

import { useWallet } from "../lib/WalletContext";
import { useNetworkMismatch } from "../hooks/useNetworkMismatch";
import { NetworkMismatchBanner } from "./NetworkMismatchBanner";

const mockUseWallet = useWallet as ReturnType<typeof vi.fn>;
const mockUseNetworkMismatch = useNetworkMismatch as ReturnType<typeof vi.fn>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function setWallet(address: string | null) {
  mockUseWallet.mockReturnValue({
    address,
    connecting: false,
    error: null,
    connect: vi.fn(),
    disconnect: vi.fn(),
    clearError: vi.fn(),
  });
}

function setNetworkState(opts: {
  mismatched: boolean;
  walletNetwork: string | null;
  configuredNetwork?: string;
}) {
  mockUseNetworkMismatch.mockReturnValue({
    mismatched: opts.mismatched,
    walletNetwork: opts.walletNetwork,
    configuredNetwork: opts.configuredNetwork ?? "testnet",
  });
}

afterEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Disconnected wallet
// ---------------------------------------------------------------------------
describe("NetworkMismatchBanner — disconnected wallet", () => {
  it("renders nothing when no wallet is connected and networks match", () => {
    setWallet(null);
    setNetworkState({ mismatched: false, walletNetwork: null });
    const { container } = render(<NetworkMismatchBanner />);
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when no wallet address is present even if mismatch flag is false", () => {
    setWallet(null);
    setNetworkState({ mismatched: false, walletNetwork: null });
    const { container } = render(<NetworkMismatchBanner />);
    expect(container.firstChild).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Matching network
// ---------------------------------------------------------------------------
describe("NetworkMismatchBanner — matching network", () => {
  it("renders nothing when wallet and app are on the same network", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: false, walletNetwork: "testnet", configuredNetwork: "testnet" });
    const { container } = render(<NetworkMismatchBanner />);
    expect(container.firstChild).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Mismatched network
// ---------------------------------------------------------------------------
describe("NetworkMismatchBanner — mismatched network", () => {
  it("renders an alert banner when the wallet is on the wrong network", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });
    render(<NetworkMismatchBanner />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("names both the wallet network and the configured network in the warning", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });
    render(<NetworkMismatchBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent(/mainnet/i);
    expect(screen.getByRole("alert")).toHaveTextContent(/testnet/i);
  });

  it("includes a dismiss button inside the alert", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });
    render(<NetworkMismatchBanner />);
    expect(
      screen.getByRole("button", { name: /dismiss network mismatch warning/i })
    ).toBeInTheDocument();
  });

  it("hides the banner when the dismiss button is clicked", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });
    render(<NetworkMismatchBanner />);

    fireEvent.click(screen.getByRole("button", { name: /dismiss network mismatch warning/i }));

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("also warns in the testnet → mainnet direction", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "testnet", configuredNetwork: "mainnet" });
    render(<NetworkMismatchBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent(/testnet/i);
    expect(screen.getByRole("alert")).toHaveTextContent(/mainnet/i);
  });
});

// ---------------------------------------------------------------------------
// Banner re-arm behaviour
// ---------------------------------------------------------------------------
describe("NetworkMismatchBanner — re-arm after mismatch resolves", () => {
  it("re-shows the banner if a mismatch reoccurs after being dismissed", () => {
    setWallet("GWALLETADDRESS1234567890");
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });

    const { rerender } = render(<NetworkMismatchBanner />);

    // Dismiss the banner.
    fireEvent.click(screen.getByRole("button", { name: /dismiss network mismatch warning/i }));
    expect(screen.queryByRole("alert")).toBeNull();

    // Networks come back into alignment — the hook now returns mismatched: false.
    setNetworkState({ mismatched: false, walletNetwork: "testnet", configuredNetwork: "testnet" });
    rerender(<NetworkMismatchBanner />);
    expect(screen.queryByRole("alert")).toBeNull();

    // User switches wallet network again — mismatch reoccurs.
    setNetworkState({ mismatched: true, walletNetwork: "mainnet", configuredNetwork: "testnet" });
    rerender(<NetworkMismatchBanner />);

    // Banner must be visible again (dismissed state was cleared when mismatch resolved).
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });
});
