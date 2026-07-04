import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import i18next from "eslint-plugin-i18next";
// Defaults internos do plugin: a regra faz spread raso das opções, então quem
// fornece `words` precisa repetir os excludes default (pontuação, ALL_CAPS,
// entidades HTML, emoji) — importamos para estendê-los em vez de copiá-los.
import i18nextDefaults from "eslint-plugin-i18next/lib/options/defaults.js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      globals: globals.browser,
    },
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
    },
  },
  {
    // Texto hardcoded em JSX deve passar por t(); "RetroSync" é marca, não se
    // traduz — fica fora da regra.
    files: ["src/**/*.tsx"],
    plugins: { i18next },
    rules: {
      "i18next/no-literal-string": [
        "error",
        {
          mode: "jsx-text-only",
          words: { exclude: [...i18nextDefaults.words.exclude, "RetroSync"] },
        },
      ],
    },
  },
  {
    files: ["worker/**/*.js"],
    languageOptions: {
      globals: globals.worker,
    },
  },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: globals.node,
    },
  },
  prettier,
);
