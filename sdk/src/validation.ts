// #930: Network config validation
import { StrKey } from '@stellar/stellar-sdk';
import { InvalidArgumentError, InvalidContractIdError } from './types';

const HEX_32_BYTE_REGEX = /^[0-9a-fA-F]{64}$/;

/**
 * #1281: Strictly decode a 32-byte hex string into a Uint8Array.
 * Rejects non-hex characters, odd lengths, and any input that is not
 * exactly 64 hex characters (32 bytes).
 */
export function hexToBytesN(hex: string): Uint8Array {
  if (typeof hex !== 'string') {
    throw new InvalidArgumentError('Expected a hex string');
  }
  if (hex.length !== 64) {
    throw new InvalidArgumentError(
      `Expected 64 hex characters (32 bytes), got ${hex.length}`,
    );
  }
  if (!HEX_32_BYTE_REGEX.test(hex)) {
    throw new InvalidArgumentError(`Invalid hex string: ${hex}`);
  }
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/**
 * #1281: Validate a Stellar contract ID (C...) using StrKey CRC16 checksum
 * verification rather than a regex alone.
 */
export function validateContractId(id: string): void {
  if (typeof id !== 'string' || !StrKey.isValidContract(id)) {
    throw new InvalidContractIdError(`Invalid contract ID: ${id}`);
  }
}

/**
 * #1281: Validate a Stellar Ed25519 public key (G...) using StrKey CRC16
 * checksum verification rather than a regex alone.
 */
export function validateAddress(address: string): void {
  if (typeof address !== 'string' || !StrKey.isValidEd25519PublicKey(address)) {
    throw new InvalidArgumentError(`Invalid address: ${address}`);
  }
}

export function validateRpcUrl(url: string): void {
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:'].includes(parsed.protocol)) throw new Error();
  } catch {
    throw new Error(`Invalid RPC URL: ${url}`);
  }
}
