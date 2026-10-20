// Content Security Policy for the frontend (#1114).
//
// Single source of truth for the CSP. `vite.config.ts` uses it to:
//   * send the header from `vite preview` (which serves the production build),
//   * write `dist/_headers` at build time for static hosts that read that file
//     (Netlify, Cloudflare Pages). Other hosts should copy the header from it.
//
// The policy is sent with `Content-Security-Policy-Report-Only` by default so
// violations show up in the console (and at `VITE_CSP_REPORT_URI`) without
// breaking anything. Set `VITE_CSP_MODE=enforce` once the reports are clean.
// A `<meta>` tag is not used because it cannot express report-only mode.
//
// Allowed origins are documented in docs/security.md.

export type CspMode = 'report-only' | 'enforce' | 'off';

export interface CspEnv {
  /** Backend API base URL (same variable `lib/api.ts` reads). */
  VITE_API_BASE_URL?: string;
  /** `testnet` (default) or `mainnet`; picks the default RPC/Horizon origins. */
  VITE_NETWORK?: string;
  /** Extra comma-separated RPC/Horizon URLs the app may connect to. */
  VITE_RPC_URLS?: string;
  /** `report-only` (default), `enforce` or `off`. */
  VITE_CSP_MODE?: string;
  /** Optional endpoint that receives violation reports. */
  VITE_CSP_REPORT_URI?: string;
}

/** Default Soroban RPC and Horizon origins per network. */
export const DEFAULT_RPC_ORIGINS: Record<string, string[]> = {
  testnet: ['https://soroban-testnet.stellar.org', 'https://horizon-testnet.stellar.org'],
  mainnet: ['https://horizon.stellar.org'],
};

/**
 * Albedo signs in a popup or an iframe served from albedo.link. Freighter and
 * xBull are browser extensions: their content scripts talk to the page with
 * `window.postMessage`, which CSP does not restrict, so they need no entry.
 */
export const WALLET_FRAME_ORIGINS = ['https://albedo.link'];

export function cspMode(env: CspEnv): CspMode {
  const raw = (env.VITE_CSP_MODE ?? '').trim().toLowerCase();
  if (raw === 'enforce' || raw === 'off') return raw;
  return 'report-only';
}

export function cspHeaderName(mode: CspMode): string | null {
  if (mode === 'off') return null;
  return mode === 'enforce' ? 'Content-Security-Policy' : 'Content-Security-Policy-Report-Only';
}

/** Returns the origin of an absolute http(s) URL, or null for relative/invalid URLs. */
export function originOf(url: string | undefined): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url.trim());
    if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') return null;
    return parsed.origin;
  } catch {
    return null;
  }
}

/** Origins the app may `fetch`/`EventSource` to, besides its own. */
export function connectOrigins(env: CspEnv): string[] {
  const network = (env.VITE_NETWORK ?? 'testnet').trim().toLowerCase();
  const extra = (env.VITE_RPC_URLS ?? '').split(',').map(originOf);
  const all = [
    originOf(env.VITE_API_BASE_URL),
    ...(DEFAULT_RPC_ORIGINS[network] ?? DEFAULT_RPC_ORIGINS.testnet),
    ...extra,
  ].filter((o): o is string => o !== null);
  return [...new Set(all)];
}

export function buildCsp(env: CspEnv): string {
  const directives: Record<string, string[]> = {
    'default-src': ["'self'"],
    // Only the app's own bundled scripts. No 'unsafe-inline' or 'unsafe-eval'.
    'script-src': ["'self'"],
    // A few components render small <style> blocks (skeletons, onboarding
    // tour). Inline styles cannot run code, so this is an accepted trade-off.
    'style-src': ["'self'", "'unsafe-inline'"],
    'img-src': ["'self'", 'data:'],
    'font-src': ["'self'"],
    'connect-src': ["'self'", ...connectOrigins(env)],
    'frame-src': WALLET_FRAME_ORIGINS,
    'worker-src': ["'self'"],
    'manifest-src': ["'self'"],
    'object-src': ["'none'"],
    'base-uri': ["'self'"],
    'form-action': ["'self'"],
    'frame-ancestors': ["'none'"],
  };
  const reportUri = env.VITE_CSP_REPORT_URI?.trim();
  if (reportUri) directives['report-uri'] = [reportUri];

  return Object.entries(directives)
    .map(([name, values]) => `${name} ${values.join(' ')}`)
    .join('; ');
}

/** Contents of a `_headers` file (Netlify / Cloudflare Pages format). */
export function buildHeadersFile(env: CspEnv): string {
  const name = cspHeaderName(cspMode(env));
  if (!name) return '';
  return `/*\n  ${name}: ${buildCsp(env)}\n`;
}
