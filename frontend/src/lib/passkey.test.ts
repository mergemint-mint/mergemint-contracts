import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PasskeyAuth, PasskeyError } from './passkey';

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function makeChallenge(purpose: string, rpId: string) {
  return {
    challenge: 'Y2hhbGxlbmdl',
    challengeId: `challenge-${purpose}`,
    rpId,
    expiresAt: Date.now() + 60_000,
  };
}

function makeAttestationCredential(): PublicKeyCredential {
  return {
    id: 'cred-1',
    type: 'public-key',
    rawId: new Uint8Array([1, 2, 3]).buffer,
    response: {
      clientDataJSON: new Uint8Array([4, 5]).buffer,
      attestationObject: new Uint8Array([6, 7]).buffer,
    },
  } as unknown as PublicKeyCredential;
}

function makeAssertionCredential(): PublicKeyCredential {
  return {
    id: 'cred-1',
    type: 'public-key',
    rawId: new Uint8Array([1, 2, 3]).buffer,
    response: {
      clientDataJSON: new Uint8Array([4, 5]).buffer,
      authenticatorData: new Uint8Array([8]).buffer,
      signature: new Uint8Array([9]).buffer,
      userHandle: null,
    },
  } as unknown as PublicKeyCredential;
}

describe('PasskeyAuth', () => {
  const hostname = 'app.example.com';

  beforeEach(() => {
    vi.stubGlobal('atob', (value: string) => Buffer.from(value, 'base64').toString('binary'));
    vi.stubGlobal('btoa', (value: string) => Buffer.from(value, 'binary').toString('base64'));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('derives rpId from the hostname', () => {
    const auth = new PasskeyAuth({ hostname });
    expect(auth.rpId).toBe(hostname);
  });

  it('registers using a server-issued challenge and returns the verified wallet address', async () => {
    const create = vi.fn().mockResolvedValue(makeAttestationCredential());
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(makeChallenge('registration', hostname)))
      .mockResolvedValueOnce(jsonResponse({ credentialId: 'cred-1', walletAddress: 'GABC' }));

    const auth = new PasskeyAuth({
      hostname,
      fetchImpl: fetchImpl as unknown as typeof fetch,
      credentials: { create, get: vi.fn() } as unknown as CredentialsContainer,
    });

    const result = await auth.register('alice', 'Alice');

    expect(result).toEqual({ credentialId: 'cred-1', walletAddress: 'GABC' });
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/passkey/challenge',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(create).toHaveBeenCalledTimes(1);
    const createArg = create.mock.calls[0][0] as PublicKeyCredentialCreationOptions;
    expect(createArg.publicKey.rp.id).toBe(hostname);
  });

  it('signs in using a server-issued challenge and returns a session token', async () => {
    const get = vi.fn().mockResolvedValue(makeAssertionCredential());
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(makeChallenge('authentication', hostname)))
      .mockResolvedValueOnce(
        jsonResponse({ credentialId: 'cred-1', walletAddress: 'GABC', sessionToken: 'tok' }),
      );

    const auth = new PasskeyAuth({
      hostname,
      fetchImpl: fetchImpl as unknown as typeof fetch,
      credentials: { create: vi.fn(), get } as unknown as CredentialsContainer,
    });

    const result = await auth.signIn('cred-1');

    expect(result.sessionToken).toBe('tok');
    expect(result.walletAddress).toBe('GABC');
    expect(get).toHaveBeenCalledTimes(1);
  });

  it('rejects a challenge whose rpId does not match the hostname', async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(makeChallenge('registration', 'evil.example.com')));

    const auth = new PasskeyAuth({
      hostname,
      fetchImpl: fetchImpl as unknown as typeof fetch,
      credentials: { create: vi.fn(), get: vi.fn() } as unknown as CredentialsContainer,
    });

    await expect(auth.register('alice', 'Alice')).rejects.toMatchObject({
      code: 'challenge_mismatch',
    });
  });

  it('throws a typed unsupported error when WebAuthn is unavailable', async () => {
    const auth = new PasskeyAuth({
      hostname,
      fetchImpl: vi.fn() as unknown as typeof fetch,
      credentials: undefined,
    });

    vi.stubGlobal('navigator', { credentials: undefined });

    expect(auth.isSupported()).toBe(false);
    await expect(auth.register('alice', 'Alice')).rejects.toBeInstanceOf(PasskeyError);
    await expect(auth.signIn()).rejects.toMatchObject({ code: 'unsupported' });
  });
});
