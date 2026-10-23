/**
 * frontend/src/vite-plugin-csp.test.ts
 *
 * Unit tests for the CSP Vite plugin (issue #1114).
 * Tests verify that the plugin injects the correct <meta> tag into HTML and
 * respects the VITE_CSP_REPORT_ONLY environment variable.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { cspPlugin } from "./vite-plugin-csp";

/** Minimal HTML that mimics index.html */
const BASE_HTML = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>MergeMint</title>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>`;

/** Helper: run transformIndexHtml with fake env vars */
function transform(
  html: string,
  env: Record<string, string> = {}
): string {
  const plugin = cspPlugin();
  // Plugin applies only in build mode; ctx.server is undefined during build.
  type TransformHook = (html: string, ctx: Record<string, unknown>) => string;
  const fn = plugin.transformIndexHtml as TransformHook;
  return fn(html, { server: { config: { env } } });
}

describe("cspPlugin", () => {
  const origEnv: Record<string, string | undefined> = {};

  beforeEach(() => {
    // Snapshot env vars we may mutate.
    for (const key of [
      "VITE_RPC_URL",
      "VITE_API_URL",
      "VITE_CSP_REPORT_ONLY",
      "VITE_CSP_REPORT_URI",
    ]) {
      origEnv[key] = process.env[key];
      delete process.env[key];
    }
  });

  afterEach(() => {
    // Restore.
    for (const [key, value] of Object.entries(origEnv)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  });

  it("injects a <meta> CSP tag before </head>", () => {
    const result = transform(BASE_HTML);
    expect(result).toContain('<meta http-equiv="Content-Security-Policy');
    expect(result.indexOf("Content-Security-Policy")).toBeLessThan(
      result.indexOf("</head>")
    );
  });

  it("defaults to report-only mode", () => {
    const result = transform(BASE_HTML);
    expect(result).toContain(
      'http-equiv="Content-Security-Policy-Report-Only"'
    );
  });

  it("enforces CSP when VITE_CSP_REPORT_ONLY=false", () => {
    const result = transform(BASE_HTML, { VITE_CSP_REPORT_ONLY: "false" });
    expect(result).toContain('http-equiv="Content-Security-Policy"');
    expect(result).not.toContain("Report-Only");
  });

  it("includes 'self' in default-src", () => {
    const result = transform(BASE_HTML);
    expect(result).toContain("default-src 'self'");
  });

  it("does not include unsafe-inline in script-src", () => {
    const result = transform(BASE_HTML);
    // Extract content of the CSP meta tag
    const match = result.match(/content="([^"]+)"/);
    expect(match).not.toBeNull();
    const policy = match![1];
    const scriptSrc = policy
      .split(";")
      .find((d) => d.trim().startsWith("script-src"));
    expect(scriptSrc).toBeDefined();
    expect(scriptSrc).not.toContain("unsafe-inline");
  });

  it("includes frame-ancestors 'none'", () => {
    const result = transform(BASE_HTML);
    expect(result).toContain("frame-ancestors 'none'");
  });

  it("adds RPC origin to connect-src when VITE_RPC_URL is set", () => {
    const result = transform(BASE_HTML, {
      VITE_RPC_URL: "https://soroban-testnet.stellar.org",
    });
    expect(result).toContain("https://soroban-testnet.stellar.org");
    const match = result.match(/content="([^"]+)"/);
    const policy = match![1];
    const connectSrc = policy
      .split(";")
      .find((d) => d.trim().startsWith("connect-src"));
    expect(connectSrc).toContain("https://soroban-testnet.stellar.org");
  });

  it("adds backend API origin to connect-src when VITE_API_URL is set", () => {
    const result = transform(BASE_HTML, {
      VITE_API_URL: "https://api.mergemint.xyz",
    });
    const match = result.match(/content="([^"]+)"/);
    const policy = match![1];
    const connectSrc = policy
      .split(";")
      .find((d) => d.trim().startsWith("connect-src"));
    expect(connectSrc).toContain("https://api.mergemint.xyz");
  });

  it("adds report-uri directive when VITE_CSP_REPORT_URI is set", () => {
    const result = transform(BASE_HTML, {
      VITE_CSP_REPORT_URI: "https://csp.mergemint.xyz/report",
    });
    expect(result).toContain(
      "report-uri https://csp.mergemint.xyz/report"
    );
  });

  it("does not add report-uri when VITE_CSP_REPORT_URI is absent", () => {
    const result = transform(BASE_HTML);
    expect(result).not.toContain("report-uri");
  });
});
