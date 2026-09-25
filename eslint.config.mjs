// Root ESLint flat config shared by every JS/TS subproject.
// Run with `npm run lint:js`; the pre-commit hook lints staged files only.
import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/node_modules/**",
      "**/dist/**",
      "**/build/**",
      "**/coverage/**",
      "target/**",
      "sdk/generated/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: { ...globals.browser, ...globals.node },
    },
  },
  {
    // CommonJS scripts and Hardhat tests.
    files: ["**/*.cjs", "scripts/*.js", "test/**/*.js", "fix.js"],
    languageOptions: {
      sourceType: "commonjs",
      globals: { ...globals.node, ...globals.mocha },
    },
    rules: {
      "@typescript-eslint/no-require-imports": "off",
    },
  },
  {
    // k6 scripts run in the k6 runtime, not Node or the browser.
    files: ["scripts/k6/**/*.js"],
    languageOptions: {
      globals: { __ENV: "readonly", __VU: "readonly", __ITER: "readonly" },
    },
  },
  // Must stay last: disables stylistic rules that conflict with Prettier.
  prettier,
);
