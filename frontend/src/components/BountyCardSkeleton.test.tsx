import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { BountyCardSkeleton } from "./BountyCardSkeleton";

describe("BountyCardSkeleton", () => {
  it("renders a single skeleton card with required accessibility attributes", () => {
    const markup = renderToStaticMarkup(<BountyCardSkeleton />);
    expect(markup).toContain("bounty-card");
    expect(markup).toContain("bounty-card--loading");
    expect(markup).toContain('aria-busy="true"');
    expect(markup).toContain('aria-label="Loading bounty"');
  });

  it("renders all four skeleton placeholder lines matching card layout", () => {
    const markup = renderToStaticMarkup(<BountyCardSkeleton />);
    expect(markup).toContain("bounty-card__id bounty-card__skeleton-line");
    expect(markup).toContain("bounty-card__creator bounty-card__skeleton-line");
    expect(markup).toContain("bounty-card__reward bounty-card__skeleton-line");
    expect(markup).toContain("bounty-card__status bounty-card__skeleton-line");
  });

  it("renders specified count of skeleton cards", () => {
    const markup = renderToStaticMarkup(<BountyCardSkeleton count={3} />);
    const matches = markup.match(/bounty-card--loading/g);
    expect(matches).toHaveLength(3);
  });

  it("applies custom class names to each card", () => {
    const markup = renderToStaticMarkup(<BountyCardSkeleton count={2} className="custom-skeleton" />);
    expect(markup).toContain("custom-skeleton");
    const matches = markup.match(/custom-skeleton/g);
    expect(matches).toHaveLength(2);
  });
});
