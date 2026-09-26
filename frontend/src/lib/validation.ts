export const SYMBOL_MAX_LENGTH = 32;

export const REWARD_AMOUNT_REGEX = /^\d+(\.\d{1,7})?$/;

export const CONTRACT_ADDRESS_REGEX = /^C[A-Z2-7]{55}$/;

/**
 * Validates that a reward amount is a positive decimal string with up to 7 decimal places.
 * @param value The reward string input
 * @returns True if valid, false otherwise
 */
export function isValidRewardAmount(value: string): boolean {
  if (!REWARD_AMOUNT_REGEX.test(value.trim())) {
    return false;
  }
  return parseFloat(value) > 0;
}

/**
 * Validates that a string is a valid Soroban contract address.
 * @param value The address string input
 * @returns True if valid, false otherwise
 */
export function isValidContractAddress(value: string): boolean {
  return CONTRACT_ADDRESS_REGEX.test(value.trim());
}

/**
 * Validates that a description fits within the on-chain Symbol length cap.
 * @param value The description string input
 * @returns True if non-empty and within cap, false otherwise
 */
export function isValidDescriptionLength(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed.length <= SYMBOL_MAX_LENGTH;
}

/**
 * Validates that a title fits within the on-chain Symbol length cap.
 * @param value The title string input
 * @returns True if non-empty and within cap, false otherwise
 */
export function isValidTitle(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed.length <= SYMBOL_MAX_LENGTH;
}

/**
 * Validates that max assignees is a positive integer greater than or equal to 1.
 * @param value Number of assignees
 * @returns True if integer >= 1, false otherwise
 */
export function isValidMaxAssignees(value: number): boolean {
  return Number.isInteger(value) && value >= 1;
}

export interface FormMilestone {
  description: string;
  reward: string;
}

export interface DetailsStepErrors {
  title?: string;
  description?: string;
}

/**
 * Validates inputs for the Details step.
 * @param data Object containing title and description
 * @returns Validation outcome with field-level errors
 */
export function validateDetailsStep(data: { title: string; description: string }): {
  valid: boolean;
  errors: DetailsStepErrors;
} {
  const errors: DetailsStepErrors = {};
  const trimmedTitle = data.title.trim();
  if (!trimmedTitle) {
    errors.title = "Title is required.";
  } else if (trimmedTitle.length > SYMBOL_MAX_LENGTH) {
    errors.title = `Title must be ${SYMBOL_MAX_LENGTH} characters or less.`;
  }

  const trimmedDescription = data.description.trim();
  if (!trimmedDescription) {
    errors.description = "Description is required.";
  } else if (trimmedDescription.length > SYMBOL_MAX_LENGTH) {
    errors.description = `Description must be ${SYMBOL_MAX_LENGTH} characters or less.`;
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  };
}

export interface RewardStepErrors {
  reward?: string;
  maxAssignees?: string;
}

/**
 * Validates inputs for the Reward step.
 * @param data Object containing reward amount and optional maxAssignees
 * @returns Validation outcome with field-level errors
 */
export function validateRewardStep(data: { reward: string; maxAssignees?: number }): {
  valid: boolean;
  errors: RewardStepErrors;
} {
  const errors: RewardStepErrors = {};
  const trimmedReward = data.reward.trim();

  if (!trimmedReward) {
    errors.reward = "Reward amount is required.";
  } else if (!isValidRewardAmount(trimmedReward)) {
    errors.reward = "Enter a positive number with up to 7 decimal places.";
  }

  if (data.maxAssignees !== undefined && !isValidMaxAssignees(data.maxAssignees)) {
    errors.maxAssignees = "Max assignees must be at least 1.";
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  };
}

export interface MilestonesStepErrors {
  milestones?: string;
  verifiers?: string;
}

/**
 * Validates inputs for the Milestones step.
 * @param data Object containing milestones, optional totalReward, verifiers, and threshold
 * @returns Validation outcome with field-level errors
 */
export function validateMilestonesStep(data: {
  milestones?: FormMilestone[];
  totalReward?: string;
  verifiers?: string[];
  threshold?: number;
}): {
  valid: boolean;
  errors: MilestonesStepErrors;
} {
  const errors: MilestonesStepErrors = {};

  if (data.milestones && data.milestones.length > 0) {
    let milestoneSum = 0;
    let hasInvalidMilestone = false;

    for (let i = 0; i < data.milestones.length; i++) {
      const milestone = data.milestones[i];
      if (!milestone.description.trim()) {
        errors.milestones = `Milestone #${i + 1} requires a description.`;
        hasInvalidMilestone = true;
        break;
      }
      if (!isValidRewardAmount(milestone.reward)) {
        errors.milestones = `Milestone #${i + 1} has an invalid reward amount.`;
        hasInvalidMilestone = true;
        break;
      }
      milestoneSum += parseFloat(milestone.reward);
    }

    if (!hasInvalidMilestone && data.totalReward && isValidRewardAmount(data.totalReward)) {
      const expectedTotal = parseFloat(data.totalReward);
      if (Math.abs(milestoneSum - expectedTotal) > 0.0000001) {
        errors.milestones = `Sum of milestone rewards must equal the total bounty reward (${expectedTotal}).`;
      }
    }
  }

  if (data.verifiers && data.verifiers.length > 0) {
    const activeVerifiers = data.verifiers.filter((verifier) => verifier.trim() !== "");
    if (activeVerifiers.length > 0) {
      const threshold = data.threshold ?? 1;
      if (threshold < 1 || threshold > activeVerifiers.length) {
        errors.verifiers = `Approval threshold must be between 1 and ${activeVerifiers.length}.`;
      }
    }
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  };
}
