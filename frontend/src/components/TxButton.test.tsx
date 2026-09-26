/**
 * Tests for TxButton — the shared button used for every on-chain action
 * (claim, complete, create, etc.) driven by useTxFlow.ts.
 *
 * Covers: idle, pending, success, and error states, as well as the
 * requirement that the button is disabled while a transaction is in flight.
 *
 * Resolves #917.
 */
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { fireEvent, render, screen } from "@testing-library/react";

import { TxButton } from "./TxButton";

// ---------------------------------------------------------------------------
// Idle state
// ---------------------------------------------------------------------------
describe("TxButton — idle state", () => {
  it("renders the children label when not pending", () => {
    const markup = renderToStaticMarkup(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(markup).toContain("Claim bounty");
    expect(markup).not.toContain("Submitting…");
  });

  it("is not disabled by default", () => {
    render(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button", { name: "Claim bounty" })).not.toBeDisabled();
  });

  it("does not carry aria-busy when not pending", () => {
    render(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button")).toHaveAttribute("aria-busy", "false");
  });

  it("does not apply the pending CSS modifier when idle", () => {
    render(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    const btn = screen.getByRole("button");
    expect(btn.className).not.toContain("tx-button--pending");
  });

  it("forwards extra props (e.g. data-testid) to the underlying button", () => {
    render(
      <TxButton pending={false} pendingLabel="Submitting…" data-testid="submit-btn">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByTestId("submit-btn")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Pending / in-flight state
// ---------------------------------------------------------------------------
describe("TxButton — pending state (transaction in flight)", () => {
  it("renders the pendingLabel instead of children while pending", () => {
    render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.queryByText("Claim bounty")).toBeNull();
    expect(screen.getByRole("button", { name: "Submitting…" })).toBeInTheDocument();
  });

  it("is disabled while a transaction is in flight", () => {
    render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("sets aria-busy=true while pending so screen readers announce the state", () => {
    render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button")).toHaveAttribute("aria-busy", "true");
  });

  it("applies the pending CSS class while in flight", () => {
    render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button").className).toContain("tx-button--pending");
  });

  it("does not fire the onClick handler when clicked while pending", () => {
    const onClick = vi.fn();
    render(
      <TxButton pending pendingLabel="Submitting…" onClick={onClick}>
        Claim bounty
      </TxButton>
    );
    // Clicking a disabled button should not call the handler.
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).not.toHaveBeenCalled();
  });

  it("stays disabled when both pending and disabled props are set", () => {
    render(
      <TxButton pending disabled pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button")).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Success state — callers typically hide the button and show a TxResultBanner,
// but TxButton itself is re-rendered idle once the tx hash is returned. We
// verify it resets cleanly.
// ---------------------------------------------------------------------------
describe("TxButton — success state (pending resolved)", () => {
  it("returns to the idle label once pending resolves to false", () => {
    const { rerender } = render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    rerender(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button", { name: "Claim bounty" })).not.toBeDisabled();
  });

  it("removes aria-busy and the pending class after the tx completes", () => {
    const { rerender } = render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    rerender(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    const btn = screen.getByRole("button");
    expect(btn).toHaveAttribute("aria-busy", "false");
    expect(btn.className).not.toContain("tx-button--pending");
  });
});

// ---------------------------------------------------------------------------
// Error state — on failure the caller typically shows a TxResultBanner.
// TxButton itself goes back to idle (not pending) so the user can retry.
// ---------------------------------------------------------------------------
describe("TxButton — error / retry state", () => {
  it("is re-enabled after a tx error (pending resets to false)", () => {
    const { rerender } = render(
      <TxButton pending pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    // Simulate the error path: useTxFlow sets pending = false on failure.
    rerender(
      <TxButton pending={false} pendingLabel="Submitting…">
        Claim bounty
      </TxButton>
    );
    expect(screen.getByRole("button", { name: "Claim bounty" })).not.toBeDisabled();
  });

  it("fires onClick when clicked after recovering from an error", () => {
    const onClick = vi.fn();
    const { rerender } = render(
      <TxButton pending pendingLabel="Submitting…" onClick={onClick}>
        Claim bounty
      </TxButton>
    );
    rerender(
      <TxButton pending={false} pendingLabel="Submitting…" onClick={onClick}>
        Claim bounty
      </TxButton>
    );
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// Style / className merging
// ---------------------------------------------------------------------------
describe("TxButton — className and style forwarding", () => {
  it("merges a custom className with the base tx-button class", () => {
    render(
      <TxButton pending={false} pendingLabel="Submitting…" className="my-custom">
        Claim bounty
      </TxButton>
    );
    const btn = screen.getByRole("button");
    expect(btn.className).toContain("tx-button");
    expect(btn.className).toContain("my-custom");
  });

  it("merges custom inline styles with the pending opacity override", () => {
    const markup = renderToStaticMarkup(
      <TxButton pending pendingLabel="Submitting…" style={{ color: "red" }}>
        Claim bounty
      </TxButton>
    );
    // Both the caller's color and the pending opacity should be present.
    expect(markup).toContain("color:red");
    expect(markup).toContain("opacity:0.6");
  });
});
