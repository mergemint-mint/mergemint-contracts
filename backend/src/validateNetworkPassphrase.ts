import { SorobanRpc } from "@stellar/stellar-sdk";

export interface NetworkPassphraseCheckConfig {
  rpcUrl: string;
  configuredPassphrase: string;
  /** Maximum time in ms to wait for a single RPC attempt. Default 15000. */
  timeoutMs?: number;
  /** Maximum number of retry attempts for transient failures. Default 3. */
  maxRetries?: number;
  /** Base delay in ms between retries (exponential backoff). Default 500. */
  retryBaseDelayMs?: number;
}

const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_MAX_RETRIES = 3;
const DEFAULT_RETRY_BASE_DELAY_MS = 500;

const RETRYABLE_STATUS_CODES = new Set([500, 502, 503, 504]);

function isRetryableError(error: unknown): boolean {
  if (error instanceof Error) {
    // Network/DNS/timeout errors are retryable.
    if (
      error.message.includes("fetch") ||
      error.message.includes("ECONNREFUSED") ||
      error.message.includes("ETIMEDOUT") ||
      error.message.includes("timeout")
    ) {
      return true;
    }
  }
  if (error && typeof error === "object" && "status" in error) {
    const status = (error as { status?: number }).status;
    if (typeof status === "number" && RETRYABLE_STATUS_CODES.has(status)) {
      return true;
    }
  }
  return false;
}

async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// Fails fast on startup if NETWORK_PASSPHRASE doesn't match the network the
// configured RPC endpoint actually serves, instead of failing silently on
// the first signed transaction.  Transient RPC hiccups (network errors,
// 5xx responses) are retried with bounded exponential backoff; real
// passphrase mismatches fail fast with actionable diagnostics.
export async function validateNetworkPassphrase(
  config: NetworkPassphraseCheckConfig,
): Promise<void> {
  const timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const maxRetries = config.maxRetries ?? DEFAULT_MAX_RETRIES;
  const retryBaseDelayMs = config.retryBaseDelayMs ?? DEFAULT_RETRY_BASE_DELAY_MS;

  let lastError: unknown = null;
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const rpc = new SorobanRpc.Server(config.rpcUrl);
      // AbortController enforces a per-attempt timeout cap.
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const { passphrase: actualPassphrase } = await rpc.getNetwork();
        clearTimeout(timeoutId);

        if (actualPassphrase !== config.configuredPassphrase) {
          throw new Error(
            `NETWORK_PASSPHRASE mismatch: configured "${config.configuredPassphrase}" ` +
              `but RPC endpoint ${config.rpcUrl} reports "${actualPassphrase}".\n` +
              `Please update NETWORK_PASSPHRASE to match the network the contract was deployed to.`,
          );
        }
        return;
      } finally {
        clearTimeout(timeoutId);
      }
    } catch (error) {
      lastError = error;
      // Passphrase mismatches are not retryable — fail fast.
      if (error instanceof Error && error.message.includes("NETWORK_PASSPHRASE mismatch")) {
        throw error;
      }
      if (attempt === maxRetries || !isRetryableError(error)) {
        throw error;
      }
      const delay = retryBaseDelayMs * Math.pow(2, attempt);
      await sleep(delay);
    }
  }
  throw lastError;
}
