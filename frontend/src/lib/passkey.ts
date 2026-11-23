/**
 * WebAuthn passkey authentication for MergeMint.
 *
 * Security model:
 * - Challenges are issued by the backend (single-use, server-tracked) and never
 *   generated client-side. This prevents replay attacks.
 * - `rp.id` is derived from the current hostname so the same build works across
 *   environments (localhost, staging, production).
 * - Attestation (registration) and assertion (sign-in) responses are verified
 *   server-side; the client never trusts its own output.
 * - Stellar/Soroban smart-wallet addresses are derived from the server-verified
 *   credential, never fabricated locally.
 *
 * See docs/passkey-auth.md for the protocol contract.
 */

export interface PasskeyChallenge {
  /** Base64url-encoded challenge bytes issued by the backend. */
  challenge: string;
  /** Opaque server-side identifier used to look up the challenge on verify. */
  challengeId: string;
  /** Relying party id the challenge was issued for. */
  rpId: string;
  /** Unix epoch milliseconds after which the challenge is invalid. */
  expiresAt: number;
}

export interface PasskeyRegistrationResult {
  credentialId: string;
  /** Server-derived Stellar/Soroban smart-wallet address (G.../C...). */
  walletAddress: string;
}

export interface PasskeySignInResult {
  credentialId: string;
  walletAddress: string;
  /** Server-issued session token after successful assertion verification. */
  sessionToken: string;
}

export type PasskeyErrorCode =
  | 'unsupported'
  | 'challenge_fetch_failed'
  | 'challenge_mismatch'
  | 'registration_failed'
  | 'verification_failed'
  | 'sign_in_failed'
  | 'network_error';

export class PasskeyError extends Error {
  readonly code: PasskeyErrorCode;

  constructor(code: PasskeyErrorCode, message: string, options?: { cause?: unknown }) {
    super(message);
    this.name = 'PasskeyError';
    this.code = code;
    if (options?.cause !== undefined) {
      (this as { cause?: unknown }).cause = options.cause;
    }
  }
}

export interface PasskeyAuthOptions {
  /** Base URL of the backend auth API. Defaults to same-origin `/api`. */
  apiBaseUrl?: string;
  /** Injectable fetch for testing. Defaults to global fetch. */
  fetchImpl?: typeof fetch;
  /** Injectable credentials container for testing. */
  credentials?: CredentialsContainer;
  /** Injectable location hostname for testing. */
  hostname?: string;
}

const DEFAULT_API_BASE = '/api';

function base64UrlToBuffer(value: string): ArrayBuffer {
  const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
  const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), '=');
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

function bufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function isWebAuthnSupported(credentials?: CredentialsContainer): boolean {
  const container = credentials ?? (typeof navigator !== 'undefined' ? navigator.credentials : undefined);
  return typeof container !== 'undefined' && typeof container.create === 'function';
}

export class PasskeyAuth {
  private readonly apiBaseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private readonly credentials?: CredentialsContainer;
  private readonly hostname: string;

  constructor(options: PasskeyAuthOptions = {}) {
    this.apiBaseUrl = options.apiBaseUrl ?? DEFAULT_API_BASE;
    this.fetchImpl = options.fetchImpl ?? ((...args) => fetch(...args));
    this.credentials = options.credentials;
    this.hostname =
      options.hostname ?? (typeof window !== 'undefined' ? window.location.hostname : '');
  }

  /** Relying party id derived from the current hostname. */
  get rpId(): string {
    return this.hostname;
  }

  isSupported(): boolean {
    return isWebAuthnSupported(this.credentials);
  }

  private getCredentials(): CredentialsContainer {
    const container = this.credentials ?? (typeof navigator !== 'undefined' ? navigator.credentials : undefined);
    if (!container) {
      throw new PasskeyError('unsupported', 'WebAuthn is not supported in this browser.');
    }
    return container;
  }

  private async fetchChallenge(purpose: 'registration' | 'authentication'): Promise<PasskeyChallenge> {
    let response: Response;
    try {
      response = await this.fetchImpl(`${this.apiBaseUrl}/passkey/challenge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ purpose, rpId: this.rpId }),
      });
    } catch (cause) {
      throw new PasskeyError('network_error', 'Failed to reach the challenge endpoint.', { cause });
    }

    if (!response.ok) {
      throw new PasskeyError(
        'challenge_fetch_failed',
        `Challenge endpoint returned ${response.status}.`,
      );
    }

    const data = (await response.json()) as Partial<PasskeyChallenge>;
    if (!data.challenge || !data.challengeId || !data.rpId) {
      throw new PasskeyError('challenge_fetch_failed', 'Malformed challenge response.');
    }
    if (data.rpId !== this.rpId) {
      throw new PasskeyError(
        'challenge_mismatch',
        `Challenge rpId ${data.rpId} does not match ${this.rpId}.`,
      );
    }

    return {
      challenge: data.challenge,
      challengeId: data.challengeId,
      rpId: data.rpId,
      expiresAt: data.expiresAt ?? 0,
    };
  }

  async register(userName: string, displayName: string): Promise<PasskeyRegistrationResult> {
    if (!this.isSupported()) {
      throw new PasskeyError('unsupported', 'WebAuthn is not supported in this browser.');
    }

    const challenge = await this.fetchChallenge('registration');
    const credentials = this.getCredentials();

    let credential: Credential | null;
    try {
      credential = await credentials.create({
        publicKey: {
          challenge: base64UrlToBuffer(challenge.challenge),
          rp: { id: challenge.rpId, name: 'MergeMint' },
          user: {
            id: new TextEncoder().encode(userName),
            name: userName,
            displayName,
          },
          pubKeyCredParams: [
            { type: 'public-key', alg: -7 },
            { type: 'public-key', alg: -257 },
          ],
          authenticatorSelection: { residentKey: 'preferred', userVerification: 'preferred' },
          timeout: 60_000,
          attestation: 'none',
        },
      });
    } catch (cause) {
      throw new PasskeyError('registration_failed', 'Passkey registration was cancelled or failed.', { cause });
    }

    if (!credential || credential.type !== 'public-key') {
      throw new PasskeyError('registration_failed', 'No public-key credential was created.');
    }

    const attestation = credential as PublicKeyCredential;
    const response = attestation.response as AuthenticatorAttestationResponse;

    let verifyResponse: Response;
    try {
      verifyResponse = await this.fetchImpl(`${this.apiBaseUrl}/passkey/register/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          challengeId: challenge.challengeId,
          rpId: challenge.rpId,
          credentialId: bufferToBase64Url(attestation.rawId),
          clientDataJSON: bufferToBase64Url(response.clientDataJSON),
          attestationObject: bufferToBase64Url(response.attestationObject),
        }),
      });
    } catch (cause) {
      throw new PasskeyError('network_error', 'Failed to reach the registration verify endpoint.', { cause });
    }

    if (!verifyResponse.ok) {
      throw new PasskeyError(
        'verification_failed',
        `Registration verification failed with ${verifyResponse.status}.`,
      );
    }

    const verified = (await verifyResponse.json()) as Partial<PasskeyRegistrationResult>;
    if (!verified.credentialId || !verified.walletAddress) {
      throw new PasskeyError('verification_failed', 'Server did not return a verified wallet address.');
    }

    return { credentialId: verified.credentialId, walletAddress: verified.walletAddress };
  }

  async signIn(credentialId?: string): Promise<PasskeySignInResult> {
    if (!this.isSupported()) {
      throw new PasskeyError('unsupported', 'WebAuthn is not supported in this browser.');
    }

    const challenge = await this.fetchChallenge('authentication');
    const credentials = this.getCredentials();

    let credential: Credential | null;
    try {
      credential = await credentials.get({
        publicKey: {
          challenge: base64UrlToBuffer(challenge.challenge),
          rpId: challenge.rpId,
          timeout: 60_000,
          userVerification: 'preferred',
          ...(credentialId
            ? { allowCredentials: [{ type: 'public-key', id: base64UrlToBuffer(credentialId) }] }
            : {}),
        },
      });
    } catch (cause) {
      throw new PasskeyError('sign_in_failed', 'Passkey sign-in was cancelled or failed.', { cause });
    }

    if (!credential || credential.type !== 'public-key') {
      throw new PasskeyError('sign_in_failed', 'No public-key assertion was returned.');
    }

    const assertion = credential as PublicKeyCredential;
    const response = assertion.response as AuthenticatorAssertionResponse;

    let verifyResponse: Response;
    try {
      verifyResponse = await this.fetchImpl(`${this.apiBaseUrl}/passkey/authenticate/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          challengeId: challenge.challengeId,
          rpId: challenge.rpId,
          credentialId: bufferToBase64Url(assertion.rawId),
          clientDataJSON: bufferToBase64Url(response.clientDataJSON),
          authenticatorData: bufferToBase64Url(response.authenticatorData),
          signature: bufferToBase64Url(response.signature),
          userHandle: response.userHandle ? bufferToBase64Url(response.userHandle) : null,
        }),
      });
    } catch (cause) {
      throw new PasskeyError('network_error', 'Failed to reach the authentication verify endpoint.', { cause });
    }

    if (!verifyResponse.ok) {
      throw new PasskeyError(
        'verification_failed',
        `Authentication verification failed with ${verifyResponse.status}.`,
      );
    }

    const verified = (await verifyResponse.json()) as Partial<PasskeySignInResult>;
    if (!verified.credentialId || !verified.walletAddress || !verified.sessionToken) {
      throw new PasskeyError('verification_failed', 'Server did not return a verified session.');
    }

    return {
      credentialId: verified.credentialId,
      walletAddress: verified.walletAddress,
      sessionToken: verified.sessionToken,
    };
  }
}
