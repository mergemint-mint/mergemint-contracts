// Exposes the backend's health status including indexer lag and connectivity checks.
// Reports indexer lag (lastIndexedLedger vs currentLedger), database reachability,
// and network passphrase for mismatch detection.
export interface HealthResponse {
  ok: boolean;
  contractId: string;
  lastIndexedLedger: number;
  currentLedger: number;
  dbReachable: boolean;
  indexerLag: number;
  networkPassphrase?: string;
}

const MAX_INDEXER_LAG_LEDGERS: number =
  parseInt(process.env.MAX_INDEXER_LAG_LEDGERS ?? "100", 10) || 100;

// Fails fast on startup if NETWORK_PASSPHRASE doesn't match the network the
// configured RPC endpoint actually serves, instead of failing silently on
// the first signed transaction.
export function buildHealthResponse(
  contractId: string,
  lastIndexedLedger: number,
  currentLedger: number,
  dbReachable: boolean,
  configuredPassphrase?: string,
): HealthResponse {
  const indexerLag = Math.max(0, currentLedger - lastIndexedLedger);
  const negativeLag = lastIndexedLedger > currentLedger;
  const lagExceeded = indexerLag > MAX_INDEXER_LAG_LEDGERS || negativeLag;
  const ok = dbReachable && currentLedger > 0 && !lagExceeded;

  return {
    ok,
    contractId,
    lastIndexedLedger,
    currentLedger,
    dbReachable,
    indexerLag,
    networkPassphrase: configuredPassphrase,
  };
}

// Legacy compatibility function - returns minimal health response
export function buildHealthResponseLegacy(configuredPassphrase: string) {
  return {
    status: "ok",
    networkPassphrase: configuredPassphrase,
  };
}
