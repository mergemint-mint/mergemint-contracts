import { describe, expect, it, vi } from "vitest";
import { TxResultBanner } from "./TxResultBanner";

// TxResultBanner is a plain function component with no hooks, so we can call
// it directly and walk the returned React element tree instead of pulling in
// a DOM rendering library just for these assertions.
function findByType(node: unknown, type: string): any {
  if (!node || typeof node !== "object") return null;
  const el = node as { type?: unknown; props?: { children?: unknown } };
  if (el.type === type) return el;
  const children = el.props?.children;
  if (Array.isArray(children)) {
    for (const child of children) {
      const found = findByType(child, type);
      if (found) return found;
    }
  } else if (children) {
    return findByType(children, type);
  }
  return null;
}

describe("TxResultBanner", () => {
  it("renders nothing when there is no result or error", () => {
    expect(TxResultBanner({ result: null })).toBeNull();
  });

  it("shows the success link when a result is present", () => {
    const element = TxResultBanner({ result: { hash: "abc123", network: "testnet" } });
    const link = findByType(element, "a");
    expect(link).toBeTruthy();
    expect(link.props.href).toContain("abc123");
  });

  it("shows a retry button on failure that re-invokes the retry callback when clicked", () => {
    const onRetry = vi.fn();
    const element = TxResultBanner({ result: null, error: "Transaction failed", onRetry });

    const button = findByType(element, "button");
    expect(button).toBeTruthy();
    expect(button.props.disabled).toBeFalsy();

    button.props.onClick();
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("disables the retry button while a retry is in flight", () => {
    const element = TxResultBanner({
      result: null,
      error: "Transaction failed",
      onRetry: vi.fn(),
      retrying: true,
    });
    const button = findByType(element, "button");
    expect(button.props.disabled).toBe(true);
  });

  it("does not render a retry button when no error is present", () => {
    const element = TxResultBanner({
      result: { hash: "abc123", network: "testnet" },
      onRetry: vi.fn(),
    });
    expect(findByType(element, "button")).toBeNull();
  });
});
