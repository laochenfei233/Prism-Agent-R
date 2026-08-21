// @ts-check
import eslint from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';
import eslintConfigPrettier from 'eslint-config-prettier';
import globals from 'globals';

export default tseslint.config(
  {
    ignores: ['build/', '.svelte-kit/', 'node_modules/', 'static/', 'src-tauri/', 'docs/'],
  },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
    rules: {
      // {@html} is used for trusted static content (SVG provider icons,
      // markdown-it rendered HTML); keep the rule off for the whole svelte tree.
      'svelte/no-at-html-tags': 'off',
    },
  },
  {
    // Svelte 5 runes modules (.svelte.ts) are plain TS — route them through the TS parser
    files: ['**/*.svelte.ts'],
    languageOptions: {
      parser: tseslint.parser,
    },
  },
  {
    rules: {
      // SvelteKit 2.x: goto() remains a valid navigation API; resolve() migration
      // is a suggestion, not a defect — leaving the rule off avoids churn in
      // PrimaryNav/+layout/+page navigation calls.
      'svelte/no-navigation-without-resolve': 'off',
    },
  },
  eslintConfigPrettier,
);
