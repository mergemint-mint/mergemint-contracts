import react from "@vitejs/plugin-react";
import { loadEnv, type Plugin } from "vite";
import { defineConfig } from "vitest/config";
import { buildCsp, buildHeadersFile, cspHeaderName, cspMode, type CspEnv } from "./src/security/csp";

// Content Security Policy (#1114). See src/security/csp.ts and docs/security.md.
// The dev server is left without a CSP because Vite injects an inline
// React-refresh script there; test the policy with `vite build && vite preview`.
function contentSecurityPolicy(env: CspEnv): Plugin {
  const header = cspHeaderName(cspMode(env));
  const policy = buildCsp(env);
  return {
    name: "mergemint-csp",
    configurePreviewServer(server) {
      if (!header) return;
      server.middlewares.use((_req, res, next) => {
        res.setHeader(header, policy);
        next();
      });
    },
    generateBundle() {
      const source = buildHeadersFile(env);
      if (source) this.emitFile({ type: "asset", fileName: "_headers", source });
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [react(), contentSecurityPolicy(loadEnv(mode, process.cwd(), "VITE_"))],
  build: {
    target: 'esnext',
    minify: 'terser',
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['react', 'react-dom'],
          router: ['react-router-dom'],
        },
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test-setup.ts'],
    // Playwright specs in e2e/ run under `npx playwright test`, not Vitest.
    exclude: ['e2e/**', 'node_modules/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text-summary', 'lcov'],
      include: ['src/**'],
      reportOnFailure: true,
    },
  },
}) as any);
