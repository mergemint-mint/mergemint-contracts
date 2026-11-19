// #930: Network config validation
// #1278: Strict RPC URL validation (typed errors, HTTPS enforcement, placeholder rejection)

export class InvalidContractIdError extends Error {
  constructor(id: string) {
    super(`Invalid contract ID: ${id}`);
    this.name = 'InvalidContractIdError';
  }
}

export class InvalidRpcUrlError extends Error {
  constructor(url: string, reason?: string) {
    super(reason ? `Invalid RPC URL: ${url} (${reason})` : `Invalid RPC URL: ${url}`);
    this.name = 'InvalidRpcUrlError';
  }
}

export function validateContractId(id: string): void {
  if (!/^C[A-Z0-9]{55}$/.test(id)) throw new InvalidContractIdError(id);
}

const LOCALHOST_HOSTS = new Set(['localhost', '127.0.0.1', '::1', '[::1]']);

function isLocalhost(hostname: string): boolean {
  return LOCALHOST_HOSTS.has(hostname.toLowerCase());
}

export function validateRpcUrl(url: string): void {
  if (typeof url !== 'string' || url.trim() === '') {
    throw new InvalidRpcUrlError(url, 'URL must be a non-empty string');
  }

  // Reject truncated placeholder URLs (e.g. ".../v1/XCa...")
  if (url.includes('...') || url.includes('\u2026')) {
    throw new InvalidRpcUrlError(url, 'placeholder or truncated URL');
  }

  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new InvalidRpcUrlError(url, 'malformed URL');
  }

  if (!['http:', 'https:'].includes(parsed.protocol)) {
    throw new InvalidRpcUrlError(url, `unsupported protocol ${parsed.protocol}`);
  }

  // Require HTTPS for non-localhost endpoints
  if (parsed.protocol === 'http:' && !isLocalhost(parsed.hostname)) {
    throw new InvalidRpcUrlError(url, 'HTTPS is required for non-localhost RPC URLs');
  }
}
