import { describe, expect, it } from 'vitest';
import {
  buildCsp,
  buildHeadersFile,
  connectOrigins,
  cspHeaderName,
  cspMode,
  originOf,
} from './csp';

function directive(csp: string, name: string): string[] {
  const entry = csp.split('; ').find((d) => d.startsWith(`${name} `));
  return entry ? entry.split(' ').slice(1) : [];
}

describe('buildCsp', () => {
  it('only allows the app’s own scripts', () => {
    const scripts = directive(buildCsp({}), 'script-src');
    expect(scripts).toEqual(["'self'"]);
    expect(buildCsp({})).not.toMatch(/script-src[^;]*unsafe-(inline|eval)/);
  });

  it('allows the backend API origin and default testnet RPC endpoints', () => {
    const connect = directive(
      buildCsp({ VITE_API_BASE_URL: 'https://api.mergemint.xyz/api/v1' }),
      'connect-src',
    );
    expect(connect).toEqual([
      "'self'",
      'https://api.mergemint.xyz',
      'https://soroban-testnet.stellar.org',
      'https://horizon-testnet.stellar.org',
    ]);
  });

  it('uses mainnet RPC origins and extra RPC URLs when configured', () => {
    const connect = directive(
      buildCsp({ VITE_NETWORK: 'mainnet', VITE_RPC_URLS: 'https://rpc.example.com/soroban, bad' }),
      'connect-src',
    );
    expect(connect).toEqual(["'self'", 'https://horizon.stellar.org', 'https://rpc.example.com']);
  });

  it('allows the Albedo wallet frame and blocks plugins and framing', () => {
    const csp = buildCsp({});
    expect(directive(csp, 'frame-src')).toEqual(['https://albedo.link']);
    expect(directive(csp, 'object-src')).toEqual(["'none'"]);
    expect(directive(csp, 'frame-ancestors')).toEqual(["'none'"]);
  });

  it('adds report-uri only when configured', () => {
    expect(buildCsp({})).not.toContain('report-uri');
    expect(directive(buildCsp({ VITE_CSP_REPORT_URI: '/csp-report' }), 'report-uri')).toEqual([
      '/csp-report',
    ]);
  });
});

describe('mode and header name', () => {
  it('defaults to report-only', () => {
    expect(cspMode({})).toBe('report-only');
    expect(cspMode({ VITE_CSP_MODE: 'nonsense' })).toBe('report-only');
    expect(cspHeaderName('report-only')).toBe('Content-Security-Policy-Report-Only');
  });

  it('can enforce or be turned off', () => {
    expect(cspMode({ VITE_CSP_MODE: 'ENFORCE' })).toBe('enforce');
    expect(cspHeaderName('enforce')).toBe('Content-Security-Policy');
    expect(cspHeaderName(cspMode({ VITE_CSP_MODE: 'off' }))).toBeNull();
  });
});

describe('helpers', () => {
  it('originOf ignores relative and non-http URLs', () => {
    expect(originOf('/api/v1')).toBeNull();
    expect(originOf('javascript:alert(1)')).toBeNull();
    expect(originOf('http://localhost:8080/api/v1')).toBe('http://localhost:8080');
  });

  it('connectOrigins removes duplicates', () => {
    expect(
      connectOrigins({ VITE_RPC_URLS: 'https://soroban-testnet.stellar.org/rpc' }),
    ).toEqual(['https://soroban-testnet.stellar.org', 'https://horizon-testnet.stellar.org']);
  });

  it('buildHeadersFile writes the header for every path', () => {
    const file = buildHeadersFile({ VITE_CSP_MODE: 'enforce' });
    expect(file.startsWith('/*\n  Content-Security-Policy: default-src')).toBe(true);
    expect(buildHeadersFile({ VITE_CSP_MODE: 'off' })).toBe('');
  });
});
