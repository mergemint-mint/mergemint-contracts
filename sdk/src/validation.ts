// #930: Network config validation
export function validateContractId(id: string): void {
  if (!/^C[A-Z0-9]{55}$/.test(id)) throw new Error(`Invalid contract ID: ${id}`);
}

export function validateRpcUrl(url: string): void {
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:'].includes(parsed.protocol)) throw new Error();
  } catch {
    throw new Error(`Invalid RPC URL: ${url}`);
  }
}
