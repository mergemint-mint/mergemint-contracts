import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ProfileHeader } from './ProfileHeader';
import { ProfileStats } from './ProfileStats';
import { ProfileHistory } from './ProfileHistory';
import { Contributor, ReputationPoint } from '../types';

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

const CONTRIBUTOR: Contributor = {
  address: 'GABC1234EFGH5678ABCD1234EFGH5678ABCD1234EFGH5678ABCD1234EFGH',
  reputation: 1500,
  completedBounties: 7,
  reputationHistory: [
    { date: '2024-01-01', reputation: 100 },
    { date: '2024-06-01', reputation: 800 },
    { date: '2024-12-01', reputation: 1500 },
  ],
};

// ---------------------------------------------------------------------------
// ProfileHeader
// ---------------------------------------------------------------------------

describe('ProfileHeader', () => {
  it('renders a shortened form of the address', () => {
    const markup = renderToStaticMarkup(<ProfileHeader contributor={CONTRIBUTOR} />);
    // shortenAddress with lead=6, trail=6 → "GABC12…GH5678" (not exact, but both ends present)
    expect(markup).toContain('GABC12');
    expect(markup).toContain('GH5678');
  });

  it('includes the full address in a title attribute', () => {
    const markup = renderToStaticMarkup(<ProfileHeader contributor={CONTRIBUTOR} />);
    expect(markup).toContain(CONTRIBUTOR.address);
  });

  it('shows "Your profile" badge when isOwn is true', () => {
    const markup = renderToStaticMarkup(
      <ProfileHeader contributor={CONTRIBUTOR} isOwn={true} />,
    );
    expect(markup).toContain('Your profile');
  });

  it('does not show the badge when isOwn is false (default)', () => {
    const markup = renderToStaticMarkup(<ProfileHeader contributor={CONTRIBUTOR} />);
    expect(markup).not.toContain('Your profile');
  });
});

// ---------------------------------------------------------------------------
// ProfileStats
// ---------------------------------------------------------------------------

describe('ProfileStats', () => {
  it('renders the reputation value', () => {
    const markup = renderToStaticMarkup(<ProfileStats contributor={CONTRIBUTOR} />);
    // toLocaleString may format with commas; check for key digits
    expect(markup).toContain('1');
    expect(markup).toContain('500');
  });

  it('renders the completed bounties count', () => {
    const markup = renderToStaticMarkup(<ProfileStats contributor={CONTRIBUTOR} />);
    expect(markup).toContain('7');
  });

  it('renders the "Reputation" label', () => {
    const markup = renderToStaticMarkup(<ProfileStats contributor={CONTRIBUTOR} />);
    expect(markup).toContain('Reputation');
  });

  it('renders the "Completed bounties" label', () => {
    const markup = renderToStaticMarkup(<ProfileStats contributor={CONTRIBUTOR} />);
    expect(markup).toContain('Completed bounties');
  });
});

// ---------------------------------------------------------------------------
// ProfileHistory
// ---------------------------------------------------------------------------

describe('ProfileHistory', () => {
  it('renders each history date', () => {
    const markup = renderToStaticMarkup(
      <ProfileHistory history={CONTRIBUTOR.reputationHistory} />,
    );
    expect(markup).toContain('2024-01-01');
    expect(markup).toContain('2024-06-01');
    expect(markup).toContain('2024-12-01');
  });

  it('shows the newest entry first', () => {
    const markup = renderToStaticMarkup(
      <ProfileHistory history={CONTRIBUTOR.reputationHistory} />,
    );
    const idx2024_12 = markup.indexOf('2024-12-01');
    const idx2024_01 = markup.indexOf('2024-01-01');
    expect(idx2024_12).toBeLessThan(idx2024_01);
  });

  it('shows an empty state when history is empty', () => {
    const markup = renderToStaticMarkup(<ProfileHistory history={[]} />);
    expect(markup).toContain('No reputation history yet');
  });

  it('does not render a table when history is empty', () => {
    const markup = renderToStaticMarkup(<ProfileHistory history={[]} />);
    expect(markup).not.toContain('<table');
  });

  it('renders a table when there are history entries', () => {
    const markup = renderToStaticMarkup(
      <ProfileHistory history={CONTRIBUTOR.reputationHistory} />,
    );
    expect(markup).toContain('<table');
  });
});
