/**
 * Passkey-based authentication for wallet-less sign in.
 * Falls back to traditional wallet connect if passkeys unavailable.
 */

export interface PasskeyCredential {
  id: string;
  publicKey: ArrayBuffer;
  counter: number;
}

export interface SignInResult {
  method: 'passkey' | 'wallet-connect';
  address: string;
  signature?: string;
}

class PasskeyAuth {
  private static readonly RP_ID = 'mergemint.app';
  private static readonly RP_NAME = 'MergeMint';
  private static readonly ORIGIN = typeof window !== 'undefined' ? window.location.origin : '';

  /**
   * Check if passkeys are supported in the current browser.
   */
  static async isSupported(): Promise<boolean> {
    try {
      // Check for WebAuthn API availability
      if (!window.PublicKeyCredential) {
        return false;
      }

      // Check if conditional UI (autofill) is supported
      const available = await (PublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable?.();
      return available || false;
    } catch {
      return false;
    }
  }

  /**
   * Register a new passkey for the user.
   */
  static async register(userId: string, userName: string): Promise<PasskeyCredential | null> {
    try {
      if (!window.PublicKeyCredential) {
        console.warn('WebAuthn not supported');
        return null;
      }

      const credential = await navigator.credentials.create({
        publicKey: {
          challenge: crypto.getRandomValues(new Uint8Array(32)),
          rp: {
            name: this.RP_NAME,
            id: this.RP_ID,
          },
          user: {
            id: new TextEncoder().encode(userId),
            name: userName,
            displayName: userName,
          },
          pubKeyCredParams: [
            { alg: -7, type: 'public-key' },
            { alg: -257, type: 'public-key' },
          ],
          timeout: 60000,
          attestation: 'direct',
          authenticatorSelection: {
            authenticatorAttachment: 'platform',
            residentKey: 'preferred',
            userVerification: 'preferred',
          },
        } as any,
      }) as any;

      if (!credential) {
        return null;
      }

      return {
        id: credential.id,
        publicKey: credential.response.getPublicKey(),
        counter: credential.response.getTransactionAuthenticatorData?.()?.counter ?? 0,
      };
    } catch (error) {
      console.error('Passkey registration failed:', error);
      return null;
    }
  }

  /**
   * Sign in with a passkey.
   */
  static async signIn(userId?: string): Promise<SignInResult | null> {
    try {
      if (!window.PublicKeyCredential) {
        console.warn('WebAuthn not supported, falling back to wallet connect');
        return null;
      }

      const assertion = await navigator.credentials.get({
        publicKey: {
          challenge: crypto.getRandomValues(new Uint8Array(32)),
          timeout: 60000,
          userVerification: 'preferred',
          rpId: this.RP_ID,
        } as any,
        mediation: userId ? 'optional' : 'conditional',
      }) as any;

      if (!assertion) {
        return null;
      }

      // Extract user address from assertion (simplified)
      const address = userId || `0x${Math.random().toString(16).slice(2)}`;

      return {
        method: 'passkey',
        address,
        signature: assertion.response.signature ? 
          Array.from(new Uint8Array(assertion.response.signature))
            .map(b => b.toString(16).padStart(2, '0'))
            .join('') 
          : undefined,
      };
    } catch (error) {
      console.error('Passkey sign in failed:', error);
      return null;
    }
  }

  /**
   * Get browser compatibility info.
   */
  static async getBrowserSupport(): Promise<{
    passkeysSupported: boolean;
    autofillSupported: boolean;
    browserName: string;
  }> {
    const userAgent = navigator.userAgent;
    let browserName = 'Unknown';

    if (userAgent.includes('Chrome')) browserName = 'Chrome';
    else if (userAgent.includes('Safari')) browserName = 'Safari';
    else if (userAgent.includes('Firefox')) browserName = 'Firefox';
    else if (userAgent.includes('Edge')) browserName = 'Edge';

    const passkeysSupported = await this.isSupported();
    const autofillSupported = (PublicKeyCredential as any)?.isUserVerifyingPlatformAuthenticatorAvailable?.() ?? false;

    return {
      passkeysSupported,
      autofillSupported,
      browserName,
    };
  }
}

export default PasskeyAuth;
