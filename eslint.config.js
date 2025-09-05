import eslint from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import unicorn from 'eslint-plugin-unicorn';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';
import svelteConfig from './svelte.config.js';

export default defineConfig(
  {
    ignores: [
      'src-tauri/',
      'dist-isolation/',
      'build/',
      '.svelte-kit/',
      'scripts/',
      'src/lib/bindings.ts',
    ],
  },
  eslint.configs.recommended,
  unicorn.configs.recommended,
  ts.configs.strictTypeChecked,
  ts.configs.stylisticTypeChecked,
  svelte.configs.recommended,
  prettier,
  svelte.configs.prettier,
  {
    languageOptions: {
      ecmaVersion: 2022,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
        extraFileExtensions: ['.svelte'],
        svelteFeatures: {
          experimentalGenerics: true,
        },
        parser: ts.parser,
        svelteConfig,
      },
    },
    rules: {
      '@typescript-eslint/restrict-template-expressions': [
        'error',
        {
          allowNumber: true,
        },
      ],
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/consistent-type-exports': 'error',
      '@typescript-eslint/no-import-type-side-effects': 'error',
      '@typescript-eslint/no-invalid-void-type': [
        'error',
        {
          allowAsThisParameter: true,
          allowInGenericTypeArguments: true,
        },
      ],
      'unicorn/explicit-length-check': 'off',
      'unicorn/prevent-abbreviations': [
        'error',
        {
          allowList: {
            Props: true,
          },
        },
      ],
      'unicorn/no-null': 'off',
      'unicorn/prefer-switch': 'off',
      'unicorn/no-empty-file': 'off',
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
    rules: {
      '@typescript-eslint/naming-convention': 'error',
    },
  },
  {
    files: ['src/**/*.svelte', 'src/**/*.svelte.ts'],
    rules: {
      // Svelte's @render syntax triggers this error
      '@typescript-eslint/no-confusing-void-expression': 'off',
    },
  },
);
