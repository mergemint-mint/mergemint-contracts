import React from 'react';
import { BountyStatus } from '../types';
import { BountySortField, SortOrder } from '../lib/api';

/**
 * Filter and sort state values for the bounty listing.
 */
export interface BountyFilterValues {
  status: BountyStatus | 'all';
  tag: string;
  sort: BountySortField;
  order: SortOrder;
}

/**
 * Props accepted by the BountyFilters component.
 */
export interface BountyFiltersProps {
  status: BountyStatus | 'all';
  tag: string;
  sort: BountySortField;
  order: SortOrder;
  onStatusChange: (status: BountyStatus | 'all') => void;
  onTagChange: (tag: string) => void;
  onSortChange: (sort: BountySortField) => void;
  onOrderChange: (order: SortOrder) => void;
  onReset: () => void;
}

const STATUS_OPTIONS: Array<BountyStatus | 'all'> = [
  'all',
  'open',
  'claimed',
  'disputed',
  'completed',
  'cancelled',
];

const SORT_OPTIONS: Array<{ value: BountySortField; label: string }> = [
  { value: 'created', label: 'Date created' },
  { value: 'reward', label: 'Reward' },
  { value: 'deadline', label: 'Deadline' },
];

const ORDER_OPTIONS: Array<{ value: SortOrder; label: string }> = [
  { value: 'desc', label: 'Descending' },
  { value: 'asc', label: 'Ascending' },
];

/**
 * Renders filter controls for bounty status, tag search, and sort order.
 *
 * @param props Configuration and callbacks for filter controls.
 * @returns JSX element containing the interactive filter controls.
 */
export function BountyFilters({
  status,
  tag,
  sort,
  order,
  onStatusChange,
  onTagChange,
  onSortChange,
  onOrderChange,
  onReset,
}: BountyFiltersProps): React.JSX.Element {
  return (
    <div className="bounty-filters" role="region" aria-label="Bounty filters and sorting">
      <div className="bounty-filters__group bounty-filters__status">
        <span className="bounty-filters__label" id="status-filter-label">
          Status
        </span>
        <div
          className="bounty-filters__status-options"
          role="group"
          aria-labelledby="status-filter-label"
        >
          {STATUS_OPTIONS.map((option) => (
            <button
              key={option}
              type="button"
              className={`bounty-filters__button ${
                status === option ? 'bounty-filters__button--active' : ''
              }`}
              aria-pressed={status === option}
              onClick={() => onStatusChange(option)}
            >
              {option.charAt(0).toUpperCase() + option.slice(1)}
            </button>
          ))}
        </div>
      </div>

      <div className="bounty-filters__group bounty-filters__tag">
        <label className="bounty-filters__label" htmlFor="tag-filter-input">
          Tag
        </label>
        <input
          id="tag-filter-input"
          type="text"
          className="bounty-filters__input"
          placeholder="Filter by tag..."
          value={tag}
          onChange={(event) => onTagChange(event.target.value)}
        />
      </div>

      <div className="bounty-filters__group bounty-filters__sort">
        <label className="bounty-filters__label" htmlFor="sort-field-select">
          Sort by
        </label>
        <select
          id="sort-field-select"
          className="bounty-filters__select"
          value={sort}
          onChange={(event) => onSortChange(event.target.value as BountySortField)}
        >
          {SORT_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="bounty-filters__group bounty-filters__order">
        <label className="bounty-filters__label" htmlFor="sort-order-select">
          Order
        </label>
        <select
          id="sort-order-select"
          className="bounty-filters__select"
          value={order}
          onChange={(event) => onOrderChange(event.target.value as SortOrder)}
        >
          {ORDER_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="bounty-filters__group bounty-filters__actions">
        <button
          type="button"
          className="bounty-filters__reset-button"
          onClick={onReset}
        >
          Reset filters
        </button>
      </div>
    </div>
  );
}
