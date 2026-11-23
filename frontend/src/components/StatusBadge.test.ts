import { describe, expect, it } from "vitest";
import { PausedContractNotice, STATUS_LABELS, StatusBadge } from "./StatusBadge";
import { BountyStatus } from "../types";

// Every value the BountyStatus union (and therefore the contract) can emit.
// Keep this list in sync with src/types.ts — if the contract adds a new
// status, this test should fail until StatusBadge is updated to handle it.
const ALL_BOUNTY_STATUSES: BountyStatus[] = [
  "open",
  "claimed",
  "submitted",
  "disputed",
  "completed",
  "cancelled",
  "archived",
  "paused",
];

describe("StatusBadge", () => {
  it.each(ALL_BOUNTY_STATUSES)("renders the label and modifier class for status '%s'", (status) => {
    const element = StatusBadge({ status });

    expect(element.props.children).toBe(STATUS_LABELS[status]);
    expect(element.props.className).toBe(`status-badge status-badge--${status}`);
    expect({
      status,
      label: element.props.children,
      className: element.props.className,
    }).toMatchSnapshot();
  });

  it("covers every status the BountyStatus union defines", () => {
    expect(ALL_BOUNTY_STATUSES).toHaveLength(new Set(ALL_BOUNTY_STATUSES).size);
    expect(ALL_BOUNTY_STATUSES.length).toBeGreaterThanOrEqual(8);
  });

  it("renders PausedContractNotice only when paused is true", () => {
    expect(PausedContractNotice({ paused: false })).toBeNull();
    const banner = PausedContractNotice({ paused: true });
    expect(banner).not.toBeNull();
    expect(banner?.props.role).toBe("status");
    expect(banner?.props.className).toBe("contract-paused-banner");
  });
});
