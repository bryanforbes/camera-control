import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import ts from 'typescript-eslint';

export default ts.config(
  {
    ignores: ['src/commands.ts', 'src-tauri/**', 'dist*/**'],
  },
  js.configs.recommended,
  ...ts.configs.strictTypeChecked,
  ...ts.configs.stylisticTypeChecked,
  prettier,
  {
    languageOptions: {
      ecmaVersion: 2022,
      parserOptions: {
        project: true,
        tsconfigRootDir: import.meta.dirname,
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
);
