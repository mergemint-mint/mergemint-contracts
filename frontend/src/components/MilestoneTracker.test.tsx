import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { MilestoneTracker } from './MilestoneTracker';

describe('MilestoneTracker', () => {
  it('renders nothing when milestones array is empty', () => {
    const markup = renderToStaticMarkup(<MilestoneTracker milestones={[]} />);
    expect(markup).toBe('');
  });

  it('renders nothing when milestones is undefined', () => {
    const markup = renderToStaticMarkup(<MilestoneTracker milestones={undefined as any} />);
    expect(markup).toBe('');
  });

  it('renders 0% completion for no completed milestones', () => {
    const milestones = [
      { description: 'Design', reward: '100 XLM', completed: false },
      { description: 'Implementation', reward: '200 XLM', completed: false },
      { description: 'Testing', reward: '150 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('0 of 3 completed');
    expect(markup).toContain('Design');
    expect(markup).toContain('Implementation');
    expect(markup).toContain('Testing');
    expect(markup).toContain('milestone-item--pending');
    expect(markup).toContain('aria-valuenow="0"');
    expect(markup).toContain('aria-valuemax="3"');
  });

  it('renders partial completion correctly', () => {
    const milestones = [
      { description: 'Design', reward: '100 XLM', completed: true },
      { description: 'Implementation', reward: '200 XLM', completed: true },
      { description: 'Testing', reward: '150 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('2 of 3 completed');
    expect(markup).toContain('milestone-item--completed');
    expect(markup).toContain('milestone-item--pending');
    expect(markup).toContain('aria-valuenow="2"');
    expect(markup).toContain('aria-valuemax="3"');
  });

  it('renders 100% completion when all milestones are done', () => {
    const milestones = [
      { description: 'Design', reward: '100 XLM', completed: true },
      { description: 'Implementation', reward: '200 XLM', completed: true },
      { description: 'Testing', reward: '150 XLM', completed: true },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('3 of 3 completed');
    expect(markup).not.toContain('milestone-item--pending');
    expect(markup).toContain('aria-valuenow="3"');
    expect(markup).toContain('aria-valuemax="3"');
  });

  it('displays per-milestone rewards', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
      { description: 'Phase 2', reward: '250 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('100 XLM');
    expect(markup).toContain('250 XLM');
  });

  it('displays total reward when provided', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
      { description: 'Phase 2', reward: '250 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(
      <MilestoneTracker milestones={milestones} totalReward="350 XLM" />
    );

    expect(markup).toContain('Total Reward:');
    expect(markup).toContain('350 XLM');
  });

  it('does not display total reward section when totalReward is not provided', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).not.toContain('Total Reward:');
  });

  it('renders checkboxes as disabled for read-only display', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
      { description: 'Phase 2', reward: '250 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    const disabledCheckboxes = (markup.match(/disabled/g) || []).length;
    expect(disabledCheckboxes).toBeGreaterThanOrEqual(2);
  });

  it('includes proper accessibility labels and roles', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('role="region"');
    expect(markup).toContain('aria-label="Milestone progress"');
    expect(markup).toContain('role="progressbar"');
    expect(markup).toContain('aria-live="polite"');
    expect(markup).toContain('aria-atomic="true"');
    expect(markup).toContain('aria-valuenow=');
    expect(markup).toContain('aria-valuemin=');
    expect(markup).toContain('aria-valuemax=');
  });

  it('calculates progress percentage correctly', () => {
    const milestones = [
      { description: 'Phase 1', reward: '100 XLM', completed: true },
      { description: 'Phase 2', reward: '250 XLM', completed: true },
      { description: 'Phase 3', reward: '150 XLM', completed: false },
      { description: 'Phase 4', reward: '200 XLM', completed: false },
    ];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    // 2 out of 4 = 50%
    expect(markup).toContain('width: 50%');
    expect(markup).toContain('2 of 4 completed');
  });

  it('handles single milestone at 100%', () => {
    const milestones = [{ description: 'Only Phase', reward: '500 XLM', completed: true }];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('1 of 1 completed');
    expect(markup).toContain('width: 100%');
    expect(markup).toContain('milestone-item--completed');
    expect(markup).not.toContain('milestone-item--pending');
  });

  it('handles single milestone at 0%', () => {
    const milestones = [{ description: 'Only Phase', reward: '500 XLM', completed: false }];

    const markup = renderToStaticMarkup(<MilestoneTracker milestones={milestones} />);

    expect(markup).toContain('0 of 1 completed');
    expect(markup).toContain('width: 0%');
    expect(markup).toContain('milestone-item--pending');
  });
});
