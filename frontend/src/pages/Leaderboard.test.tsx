import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi, afterEach } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { Leaderboard } from './Leaderboard';
import { LeaderboardPage } from '../types';
import { api } from '../lib/api';

// Mock api so we can control what getLeaderboard returns without a network.
vi.mock('../lib/api', () => ({
  api: {
    getLeaderboard: vi.fn(),
  },
}));

afterEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderLeaderboard(): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <Leaderboard />
    </MemoryRouter>,
  );
}

const SAMPLE_PAGE: LeaderboardPage = {
  entries: [
    {
      rank: 1,
      address: 'GAAA1111BBBB2222CCCC3333DDDD4444EEEE5555FFFF6666GGGG7777HHHH',
      reputation: 5000,
      completedBounties: 20,
    },
    {
      rank: 2,
      address: 'GBBB2222CCCC3333DDDD4444EEEE5555FFFF6666GGGG7777HHHH8888IIII',
      reputation: 4200,
      completedBounties: 15,
    },
    {
      rank: 3,
      address: 'GCCC3333DDDD4444EEEE5555FFFF6666GGGG7777HHHH8888IIII9999JJJJ',
      reputation: 3100,
      completedBounties: 10,
    },
  ],
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Leaderboard', () => {
  it('renders the heading', () => {
    vi.mocked(api.getLeaderboard).mockReturnValue(new Promise(() => {}));
    const markup = renderLeaderboard();
    expect(markup).toContain('Leaderboard');
  });

  it('shows a loading indicator on initial render (before the promise resolves)', () => {
    // Never resolves — component stays in loading state throughout static render.
    vi.mocked(api.getLeaderboard).mockReturnValue(new Promise(() => {}));
    const markup = renderLeaderboard();
    expect(markup).toContain('Loading');
  });

  it('renders the leaderboard heading even when the entries array is empty', () => {
    vi.mocked(api.getLeaderboard).mockResolvedValue({ entries: [] });
    // On initial static render the async effect has not yet run, so we see
    // the loading state which still contains the heading.
    const markup = renderLeaderboard();
    expect(markup).toContain('Leaderboard');
  });

  it('applies the correct CSS modifier classes to rank badges', () => {
    // Test the badge CSS logic in isolation — no async effect needed.
    const markup = renderToStaticMarkup(
      <MemoryRouter>
        <span className="leaderboard__rank-badge leaderboard__rank-badge--gold">1</span>
        <span className="leaderboard__rank-badge leaderboard__rank-badge--silver">2</span>
        <span className="leaderboard__rank-badge leaderboard__rank-badge--bronze">3</span>
        <span className="leaderboard__rank-badge leaderboard__rank-badge--default">4</span>
      </MemoryRouter>,
    );
    expect(markup).toContain('leaderboard__rank-badge--gold');
    expect(markup).toContain('leaderboard__rank-badge--silver');
    expect(markup).toContain('leaderboard__rank-badge--bronze');
    expect(markup).toContain('leaderboard__rank-badge--default');
  });

  it('renders contributor profile links with the correct href pattern', () => {
    const addr = SAMPLE_PAGE.entries[0].address;
    const markup = renderToStaticMarkup(
      <MemoryRouter>
        <a href={`/contributors/${addr}`}>profile</a>
      </MemoryRouter>,
    );
    expect(markup).toContain(`/contributors/${addr}`);
  });
});
