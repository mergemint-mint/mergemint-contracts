const TOKEN_DECIMALS = 7;

/**
 * Converts a raw on-chain integer amount (stroops-style, 7 decimal places)
 * into a human-readable decimal string.
 */
export function formatTokenAmount(raw: string): string {
  const negative = raw.startsWith("-");
  const digits = negative ? raw.slice(1) : raw;
  const padded = digits.padStart(TOKEN_DECIMALS + 1, "0");
  const whole = padded.slice(0, -TOKEN_DECIMALS).replace(/^0+(?=\d)/, "");
  const fraction = padded.slice(-TOKEN_DECIMALS).replace(/0+$/, "");
  const result = fraction ? `${whole}.${fraction}` : whole;
  return negative ? `-${result}` : result;
}

/**
 * Converts a human-readable decimal string into a raw on-chain integer
 * amount at 7 decimal places. Inverse of formatTokenAmount.
 */
export function toRawTokenAmount(value: string): string {
  const negative = value.startsWith("-");
  const trimmed = negative ? value.slice(1) : value;
  const [wholePart, fractionPart = ""] = trimmed.split(".");
  const fraction = fractionPart.slice(0, TOKEN_DECIMALS).padEnd(TOKEN_DECIMALS, "0");
  const raw = `${wholePart}${fraction}`.replace(/^0+(?=\d)/, "");
  return negative ? `-${raw}` : raw;
}

/**
 * Shortens a wallet/contract address for display, e.g. "GABC…4567".
 * Addresses shorter than lead + trail are returned unchanged.
 */
export function shortenAddress(address: string, lead = 4, trail = 4): string {
  if (address.length <= lead + trail) return address;
  return `${address.slice(0, lead)}…${address.slice(-trail)}`;
}

// Known contract/backend error substrings mapped to user-friendly copy.
// Falls back to the raw message when nothing matches (see issue #57).
const ERROR_MAP: Array<[RegExp, string]> = [
  [/contributor already has an active claim/i, "You already have an active claim on this bounty."],
  [/bounty (is )?not found/i, "This bounty no longer exists."],
  [/bounty (is )?already claimed/i, "This bounty has already been claimed by someone else."],
  [/insufficient (funds|balance)/i, "You don't have enough balance to complete this action."],
  [/unauthorized|not (the )?creator/i, "You don't have permission to perform this action."],
  [/dispute window (has )?closed/i, "The dispute window for this bounty has closed."],
  [/internal server error/i, "Something went wrong on our end. Please try again shortly."],
  [/network ?error|failed to fetch/i, "Unable to reach the server. Check your connection and try again."],
];

/**
 * Maps a raw contract/backend error message to user-friendly copy,
 * falling back to the raw message when nothing matches.
 */
export function mapErrorMessage(raw: string): string {
  if (!raw) return "Something went wrong. Please try again.";
  for (const [pattern, friendly] of ERROR_MAP) {
    if (pattern.test(raw)) return friendly;
  }
  return raw;
}

/**
 * Formats a Unix-epoch timestamp (seconds) as a relative time string using
 * Intl.RelativeTimeFormat, e.g. "in 3 days", "2 hours ago".
 * Falls back to the ISO date string when the timestamp is null/undefined.
 */
export function formatRelative(
  epochSeconds: number | null | undefined,
  now: Date = new Date()
): string {
  if (epochSeconds == null) return '—';
  const diffMs = epochSeconds * 1000 - now.getTime();
  const diffSec = Math.round(diffMs / 1000);
  const absMs = Math.abs(diffMs);

  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

  if (absMs < 60_000) return rtf.format(diffSec, 'second');
  if (absMs < 3_600_000) return rtf.format(Math.round(diffSec / 60), 'minute');
  if (absMs < 86_400_000) return rtf.format(Math.round(diffSec / 3600), 'hour');
  if (absMs < 2_592_000_000) return rtf.format(Math.round(diffSec / 86400), 'day');
  if (absMs < 31_536_000_000) return rtf.format(Math.round(diffSec / 2_592_000), 'month');
  return rtf.format(Math.round(diffSec / 31_536_000), 'year');
}

/**
 * Returns true when a deadline (Unix epoch seconds) is within 24 hours from now.
 */
export function isDeadlineUrgent(
  epochSeconds: number | null | undefined,
  now: Date = new Date()
): boolean {
  if (epochSeconds == null) return false;
  const diffMs = epochSeconds * 1000 - now.getTime();
  return diffMs > 0 && diffMs < 86_400_000;
}
