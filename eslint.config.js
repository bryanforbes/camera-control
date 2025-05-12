import eslint from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import svelteParser from 'svelte-eslint-parser';
import ts from 'typescript-eslint';
import svelteConfig from './svelte.config.js';

export default ts.config(
  {
    ignores: [
      'src/commands.ts',
      'src-tauri/',
      'dist*/',
      'build/',
      '.svelte-kit/',
      'scripts/',
    ],
  },
  eslint.configs.recommended,
  ...ts.configs.strictTypeChecked,
  ...ts.configs.stylisticTypeChecked,
  ...svelte.configs.recommended,
  prettier,
  ...svelte.configs.prettier,
  {
    languageOptions: {
      ecmaVersion: 2022,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
        extraFileExtensions: ['.svelte'],
      },
    },
    rules: {
      '@typescript-eslint/restrict-template-expressions': 'off',
      '@typescript-eslint/no-confusing-void-expression': 'off',
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/consistent-type-exports': 'error',
      '@typescript-eslint/no-import-type-side-effects': 'error',
    },
  },
  {
    files: ['*.ts', '.*.ts', '*.js', '.*.js'],
    ignores: ['vitest.setup.ts', 'vite.config.ts'],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    files: ['src/**'],
    languageOptions: {
      globals: globals.browser,
    },
  },
  {
    files: ['src/**/*.svelte', 'src/**/*.svelte.ts'],
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        svelteFeatures: {
          experimentalGenerics: true,
        },
        parser: ts.parser,
        svelteConfig,
      },
    },
  },
);
