// #929: Amount formatting utilities using bigint
export function formatAmount(raw: bigint, decimals: number): string {
  const factor = 10n ** BigInt(decimals);
  const major = raw / factor;
  const minor = raw % factor;
  return `${major}.${minor.toString().padStart(decimals, '0')}`;
}

export function parseAmount(display: string, decimals: number): bigint {
  const [major, minor] = display.split('.');
  const factor = 10n ** BigInt(decimals);
  return BigInt(major) * factor + BigInt((minor || '0').padEnd(decimals, '0'));
}
