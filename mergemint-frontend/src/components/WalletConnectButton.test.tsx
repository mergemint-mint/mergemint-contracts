import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import WalletConnectButton from "./WalletConnectButton";
import * as wallet from "../lib/wallet";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("WalletConnectButton", () => {
  it("renders a Connect wallet button when no wallet is connected", () => {
    render(<WalletConnectButton />);
    expect(screen.getByRole("button", { name: "Connect wallet" })).toBeInTheDocument();
  });

  it("shows a pending state while connecting", async () => {
    let resolveAccess!: (address: string) => void;
    vi.spyOn(wallet, "isFreighterInstalled").mockResolvedValue(true);
    vi.spyOn(wallet, "requestAccess").mockReturnValue(
      new Promise((resolve) => {
        resolveAccess = resolve;
      })
    );

    render(<WalletConnectButton />);
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));

    expect(await screen.findByRole("button", { name: "Connecting…" })).toBeDisabled();

    resolveAccess("GCONNECTEDADDRESS1234567890");
    expect(await screen.findByTitle("GCONNECTEDADDRESS1234567890")).toBeInTheDocument();
  });

  it("shows the shortened address and a Disconnect control once connected", async () => {
    const onConnect = vi.fn();
    vi.spyOn(wallet, "isFreighterInstalled").mockResolvedValue(true);
    vi.spyOn(wallet, "requestAccess").mockResolvedValue("GCONNECTEDADDRESS1234567890");

    render(<WalletConnectButton onConnect={onConnect} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));

    await screen.findByRole("button", { name: "Disconnect" });
    expect(screen.getByTitle("GCONNECTEDADDRESS1234567890")).toHaveTextContent("GCON…7890");
    expect(onConnect).toHaveBeenCalledWith("GCONNECTEDADDRESS1234567890");
  });

  it("returns to the disconnected state when Disconnect is clicked", async () => {
    const onDisconnect = vi.fn();
    vi.spyOn(wallet, "isFreighterInstalled").mockResolvedValue(true);
    vi.spyOn(wallet, "requestAccess").mockResolvedValue("GCONNECTEDADDRESS1234567890");

    render(<WalletConnectButton onDisconnect={onDisconnect} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));
    fireEvent.click(await screen.findByRole("button", { name: "Disconnect" }));

    expect(screen.getByRole("button", { name: "Connect wallet" })).toBeInTheDocument();
    expect(onDisconnect).toHaveBeenCalledOnce();
  });

  it("shows an error message when the wallet extension is not installed", async () => {
    vi.spyOn(wallet, "isFreighterInstalled").mockResolvedValue(false);

    render(<WalletConnectButton />);
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));

    expect(await screen.findByText("Freighter extension is not installed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect wallet" })).toBeInTheDocument();
  });
});
