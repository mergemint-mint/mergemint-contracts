import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { BountyCard } from "./BountyCard";
import { Bounty } from "../lib/types";

const BOUNTY: Bounty = {
  id: "bounty-123456789",
  creator: "GCREATORADDRESS1234567890",
  rewardAmount: 1000n,
  rewardToken: "XLM",
  assignees: [],
  maxAssignees: 1,
  status: "open",
  minReputation: 0,
  deadline: null,
  tags: [],
  approvalThreshold: 1,
  milestones: [],
};

describe("BountyCard", () => {
  it("renders bounty data when not loading", () => {
    const markup = renderToStaticMarkup(<BountyCard bounty={BOUNTY} />);
    expect(markup).toContain("1000 XLM");
    expect(markup).toContain("open");
    expect(markup).not.toContain("bounty-card--loading");
  });

  it("matches the skeleton snapshot when loading is true", () => {
    const markup = renderToStaticMarkup(<BountyCard bounty={BOUNTY} loading />);
    expect(markup).toMatchSnapshot();
  });

  it("renders the skeleton when no bounty is provided yet, even without loading set", () => {
    const markup = renderToStaticMarkup(<BountyCard />);
    expect(markup).toContain("bounty-card--loading");
    expect(markup).toContain('aria-busy="true"');
  });
});
