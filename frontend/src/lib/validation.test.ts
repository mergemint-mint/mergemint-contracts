/**
 * Asserts frontend/src/lib/validation.ts agrees with mergemint-backend's
 * validation.rs on the reward-amount and description-length rules, using the
 * shared fixture at fixtures/validation-parity.json (repo root).
 *
 * Migrated from app/src/lib/validation.test.ts (#916). Runs under Vitest.
 */
import { describe, expect, it } from 'vitest';
import { isValidRewardAmount, isValidDescriptionLength } from './validation';

// Vitest (with Vite) supports JSON imports natively.
import fixture from '../../../fixtures/validation-parity.json';

interface Case {
  value: string;
  valid: boolean;
  reason: string;
}

interface Fixture {
  rewardAmount: Case[];
  descriptionLength: Case[];
}

const { rewardAmount, descriptionLength } = fixture as unknown as Fixture;

describe('isValidRewardAmount — backend parity', () => {
  it.each(rewardAmount.map((c) => [c.value, c.valid, c.reason] as const))(
    'value=%j -> valid=%s (%s)',
    (value, valid) => {
      expect(isValidRewardAmount(value)).toBe(valid);
    },
  );
});

describe('isValidDescriptionLength — backend parity', () => {
  it.each(descriptionLength.map((c) => [c.value, c.valid, c.reason] as const))(
    'value=%j -> valid=%s (%s)',
    (value, valid) => {
      expect(isValidDescriptionLength(value)).toBe(valid);
    },
  );
});
