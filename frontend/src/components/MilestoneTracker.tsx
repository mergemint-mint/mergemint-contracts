import React from 'react';

interface Milestone {
  description: string;
  reward: string;
  completed: boolean;
}

interface MilestoneTrackerProps {
  milestones: Milestone[];
  totalReward?: string;
}

export function MilestoneTracker({ milestones, totalReward }: MilestoneTrackerProps) {
  if (!milestones || milestones.length === 0) {
    return null;
  }

  const completedCount = milestones.filter((m) => m.completed).length;
  const totalCount = milestones.length;
  const progressPercent = (completedCount / totalCount) * 100;

  return (
    <div className="milestone-tracker" role="region" aria-label="Milestone progress">
      <div className="milestone-tracker__header">
        <h2 className="milestone-tracker__title">Milestones</h2>
        <div
          className="milestone-tracker__progress-summary"
          aria-live="polite"
          aria-atomic="true"
        >
          <span className="milestone-tracker__count">
            {completedCount} of {totalCount} completed
          </span>
        </div>
      </div>

      <div
        className="milestone-tracker__progress-bar"
        role="progressbar"
        aria-valuenow={completedCount}
        aria-valuemin={0}
        aria-valuemax={totalCount}
        aria-label={`Milestone progress: ${completedCount} of ${totalCount} completed`}
      >
        <div
          className="milestone-tracker__progress-fill"
          style={{ width: `${progressPercent}%` }}
        />
      </div>

      <ul className="milestone-tracker__list">
        {milestones.map((milestone, index) => (
          <li
            key={index}
            className={`milestone-item ${
              milestone.completed ? 'milestone-item--completed' : 'milestone-item--pending'
            }`}
          >
            <input
              type="checkbox"
              className="milestone-item__checkbox"
              checked={milestone.completed}
              disabled
              aria-label={`Milestone ${index + 1}: ${milestone.description}`}
            />
            <div className="milestone-item__content">
              <span className="milestone-item__description">{milestone.description}</span>
              <span className="milestone-item__reward">{milestone.reward}</span>
            </div>
          </li>
        ))}
      </ul>

      {totalReward && (
        <div className="milestone-tracker__total">
          <span className="milestone-tracker__total-label">Total Reward:</span>
          <span className="milestone-tracker__total-value">{totalReward}</span>
        </div>
      )}
    </div>
  );
}
