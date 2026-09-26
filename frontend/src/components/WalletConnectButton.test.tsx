/**
 * Tests for the frontend WalletConnectButton controlled component.
 *
 * Ported and adapted from mergemint-frontend as part of #916 (consolidate
 * frontend directories). The frontend version is a controlled component that
 * receives `address` and `onConnect` as props, unlike the mergemint-frontend
 * standalone version that managed its own Freighter state internally.
 */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { WalletConnectButton } from "./WalletConnectButton";

describe("WalletConnectButton", () => {
  it("renders a Connect wallet button when no address is provided", () => {
    render(<WalletConnectButton address={null} onConnect={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Connect wallet" })).toBeInTheDocument();
  });

  it("calls onConnect when the Connect wallet button is clicked", () => {
    const onConnect = vi.fn();
    render(<WalletConnectButton address={null} onConnect={onConnect} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));
    expect(onConnect).toHaveBeenCalledTimes(1);
  });

  it("shows the shortened address once connected", () => {
    const { container } = render(
      <WalletConnectButton address="GCONNECTEDADDRESS1234567890" onConnect={vi.fn()} />
    );
    // The connected span wraps the shortened form and carries the full address as title.
    const addressSpan = container.querySelector("span[title='GCONNECTEDADDRESS1234567890']");
    expect(addressSpan).toBeTruthy();
    // shortenAddress("GCONNECTEDADDRESS1234567890", 4, 4) → "GCON…7890"
    expect(addressSpan!.textContent).toBe("GCON\u20267890");
  });

  it("renders a copy button alongside the address when connected", () => {
    render(
      <WalletConnectButton address="GCONNECTEDADDRESS1234567890" onConnect={vi.fn()} />
    );
    expect(screen.getByRole("button", { name: "Copy to clipboard" })).toBeInTheDocument();
  });

  it("does not render a disconnect button — address control is lifted to the parent", () => {
    render(
      <WalletConnectButton address="GCONNECTEDADDRESS1234567890" onConnect={vi.fn()} />
    );
    expect(screen.queryByRole("button", { name: /disconnect/i })).toBeNull();
  });

  it("shows the Connect wallet button again when address is reset to null", () => {
    const { rerender } = render(
      <WalletConnectButton address="GCONNECTEDADDRESS1234567890" onConnect={vi.fn()} />
    );
    rerender(<WalletConnectButton address={null} onConnect={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Connect wallet" })).toBeInTheDocument();
  });

  it("applies the connected CSS modifier when an address is provided", () => {
    const { container } = render(
      <WalletConnectButton address="GCONNECTEDADDRESS1234567890" onConnect={vi.fn()} />
    );
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain("wallet-connect--connected");
  });

  it("does not apply the connected modifier when no address is provided", () => {
    const { container } = render(<WalletConnectButton address={null} onConnect={vi.fn()} />);
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).not.toContain("wallet-connect--connected");
  });
});
