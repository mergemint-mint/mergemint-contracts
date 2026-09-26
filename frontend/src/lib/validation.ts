// On-chain title/description/metadata fields are stored as Soroban Symbols,
// which cap out at 32 characters.
export const SYMBOL_MAX_LENGTH = 32;

const REWARD_AMOUNT_REGEX = /^\d+(\.\d{1,7})?$/;

export function isValidRewardAmount(value: string): boolean {
  if (!REWARD_AMOUNT_REGEX.test(value.trim())) return false;
  return parseFloat(value) > 0;
}

// Soroban contract addresses are strkey-encoded, start with "C", and are
// always 56 characters long.
const CONTRACT_ADDRESS_REGEX = /^C[A-Z2-7]{55}$/;

export function isValidContractAddress(value: string): boolean {
  return CONTRACT_ADDRESS_REGEX.test(value.trim());
}

// Title/description fields must be non-empty and fit within the on-chain
// Symbol length cap (see SYMBOL_MAX_LENGTH above). This mirrors the rule
// mergemint-backend enforces in `validation::is_valid_description_length`.
export function isValidDescriptionLength(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed.length <= SYMBOL_MAX_LENGTH;
}
