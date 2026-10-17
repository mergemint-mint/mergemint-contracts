// Root ESLint flat config shared by every JS/TS subproject.
// Run with `npm run lint:js`; the pre-commit hook lints staged files only.
import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import globals from "globals";
import tseslint from "typescript-eslint";

const noHardcodedJsxStringsRule = {
  meta: {
    type: "problem",
    docs: {
      description: "Disallow hardcoded text strings in JSX elements. Use i18n translation functions instead.",
    },
    schema: [],
    messages: {
      noHardcodedJsx: "Hardcoded text '{{text}}' in JSX is not allowed. Extract it to i18n and use t(...).",
    },
  },
  create(context) {
    return {
      JSXText(node) {
        const raw = node.value;
        const trimmed = raw.replace(/\s+/g, " ").trim();
        // Ignore whitespace, numbers, standalone symbols/punctuation (&times;, /, -, |, etc.)
        if (!trimmed || !/[A-Za-z]/.test(trimmed)) {
          return;
        }
        context.report({
          node,
          messageId: "noHardcodedJsx",
          data: {
            text: trimmed.length > 30 ? trimmed.slice(0, 27) + "..." : trimmed,
          },
        });
      },
    };
  },
};

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
  {
    // Dummy stub for react-hooks to satisfy legacy inline disable comments.
    plugins: {
      "react-hooks": {
        rules: {
          "exhaustive-deps": {
            create() {
              return {};
            },
          },
        },
      },
    },
  },
  {
    // Issue #1106: Detect hardcoded JSX text strings; ignore tests and Storybook stories.
    files: ["frontend/src/**/*.{jsx,tsx}"],
    ignores: [
      "**/*.test.{js,ts,jsx,tsx}",
      "**/*.spec.{js,ts,jsx,tsx}",
      "**/*.stories.{js,ts,jsx,tsx}",
      "**/stories/**",
      "**/test/**",
      "**/tests/**",
      "**/__tests__/**",
      "frontend/src/test-setup.ts",
    ],
    plugins: {
      i18n: {
        rules: {
          "no-hardcoded-jsx-strings": noHardcodedJsxStringsRule,
        },
      },
    },
    rules: {
      "i18n/no-hardcoded-jsx-strings": "warn",
    },
  },
  // Must stay last: disables stylistic rules that conflict with Prettier.
  prettier,
);
