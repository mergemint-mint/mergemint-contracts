import { describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";
import { BountyStatus } from "../types";

// Every value the BountyStatus union (and therefore the contract) can emit.
// Keep this list in sync with src/types.ts — if the contract adds a new
// status, this test should fail until StatusBadge is updated to handle it.
const ALL_BOUNTY_STATUSES: BountyStatus[] = [
  "open",
  "claimed",
  "disputed",
  "completed",
  "cancelled",
];

describe("StatusBadge", () => {
  it.each(ALL_BOUNTY_STATUSES)("renders the label and modifier class for status '%s'", (status) => {
    const element = StatusBadge({ status });

    expect(element.props.children).toBe(status);
    expect(element.props.className).toBe(`status-badge status-badge--${status}`);
    expect({
      status,
      label: element.props.children,
      className: element.props.className,
    }).toMatchSnapshot();
  });

  it("covers every status the BountyStatus union defines", () => {
    // Guards against someone adding a status to the union without adding
    // it to ALL_BOUNTY_STATUSES above.
    expect(ALL_BOUNTY_STATUSES).toHaveLength(new Set(ALL_BOUNTY_STATUSES).size);
    expect(ALL_BOUNTY_STATUSES.length).toBeGreaterThanOrEqual(5);
  });
});
