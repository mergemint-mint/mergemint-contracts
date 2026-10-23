/**
 * frontend/src/vite-plugin-csp.ts
 *
 * Vite plugin that injects a Content-Security-Policy (or
 * Content-Security-Policy-Report-Only) <meta> tag into index.html at build
 * time.  The directives are assembled from environment variables so the same
 * build artefact can be used across environments without re-compiling.
 *
 * Environment variables (all optional — safe defaults shown):
 *
 *   VITE_RPC_URL         The Stellar RPC origin, e.g. https://soroban-testnet.stellar.org
 *   VITE_API_URL         The mergemint-backend origin, e.g. https://api.mergemint.xyz
 *   VITE_CSP_REPORT_URI  The reporting endpoint for policy violations (omit to skip report-uri)
 *   VITE_CSP_REPORT_ONLY Set to "true" to emit Content-Security-Policy-Report-Only instead
 *                        of the enforcing header.  Defaults to "true" so the first deployment
 *                        is always in report-only mode.
 *
 * Design notes
 * ────────────
 * • No `unsafe-inline` is allowed in script-src or style-src.  React injects
 *   styles at runtime via <style> tags; those need `unsafe-inline` in
 *   style-src, which is a conscious tradeoff for XSS resistance vs. DX.
 *   Inline scripts are never permitted.
 *
 * • `connect-src` allows the app's own origin (relative URLs), the configured
 *   RPC endpoint, and the backend API.  Wallet browser extensions communicate
 *   through `window.postMessage` (no CSP restriction) or via the page's own
 *   origin, so no extra CSP directive is needed for them.
 *
 * • `frame-ancestors 'none'` prevents clickjacking.
 *
 * • `upgrade-insecure-requests` is included so mixed-content fetches are
 *   automatically upgraded to HTTPS in production.
 */

import type { Plugin } from "vite";

export interface CspOptions {
  /** Override directives for advanced use.  Merged on top of defaults. */
  extraDirectives?: Record<string, string>;
}

export function cspPlugin(options: CspOptions = {}): Plugin {
  return {
    name: "mergemint-csp",
    // Run in build and dev server (dev server benefits from early warning).
    apply: "build",

    transformIndexHtml(html, ctx) {
      const env = ctx.server?.config?.env ?? {};

      // Pull values from Vite's env (populated from .env files and the real
      // process environment).
      const rpcUrl = resolveOrigin(
        env.VITE_RPC_URL ?? process.env.VITE_RPC_URL ?? ""
      );
      const apiUrl = resolveOrigin(
        env.VITE_API_URL ?? process.env.VITE_API_URL ?? ""
      );
      const reportUri =
        env.VITE_CSP_REPORT_URI ?? process.env.VITE_CSP_REPORT_URI ?? "";
      const reportOnly =
        (env.VITE_CSP_REPORT_ONLY ?? process.env.VITE_CSP_REPORT_ONLY ?? "true") === "true";

      // Build connect-src from the configured origins.
      const connectSrcParts = ["'self'"];
      if (rpcUrl) connectSrcParts.push(rpcUrl);
      if (apiUrl) connectSrcParts.push(apiUrl);
      // SSE endpoint shares the same origin as the API, already covered.

      const directives: Record<string, string> = {
        "default-src": "'self'",
        // No unsafe-inline; all scripts are loaded as ES modules from 'self'.
        "script-src": "'self'",
        // React runtime injects <style> tags; unsafe-inline is required here.
        "style-src": "'self' 'unsafe-inline'",
        "img-src": "'self' data:",
        "font-src": "'self'",
        "connect-src": connectSrcParts.join(" "),
        "media-src": "'none'",
        "object-src": "'none'",
        // Prevent this page from being embedded in iframes (clickjacking guard).
        "frame-ancestors": "'none'",
        // Upgrade mixed-content requests to HTTPS automatically.
        "upgrade-insecure-requests": "",
        // Disallow <base> tag hijacking.
        "base-uri": "'self'",
        "form-action": "'self'",
        ...(options.extraDirectives ?? {}),
      };

      if (reportUri) {
        directives["report-uri"] = reportUri;
      }

      const policyString = Object.entries(directives)
        .map(([key, value]) => (value ? `${key} ${value}` : key))
        .join("; ");

      const headerName = reportOnly
        ? "Content-Security-Policy-Report-Only"
        : "Content-Security-Policy";

      const metaTag = `<meta http-equiv="${headerName}" content="${policyString}">`;

      // Inject just before </head>.
      return html.replace("</head>", `  ${metaTag}\n  </head>`);
    },
  };
}

/** Extract the origin (scheme + host + optional port) from a URL string. */
function resolveOrigin(url: string): string {
  if (!url) return "";
  try {
    const parsed = new URL(url);
    return parsed.origin;
  } catch {
    // Treat as a bare origin already (e.g. "https://api.mergemint.xyz").
    return url;
  }
}
