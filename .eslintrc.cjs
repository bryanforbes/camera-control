module.exports = {
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/strict-type-checked',
    'plugin:@typescript-eslint/stylistic-type-checked',
  ],
  parser: '@typescript-eslint/parser',
  plugins: ['@typescript-eslint'],
  parserOptions: {
    project: true,
    tsconfigRootDir: __dirname,
  },
  root: true,
  ignorePatterns: ['src/commands.ts', 'src-tauri/**', 'dist*/**'],
  rules: {
    '@typescript-eslint/restrict-template-expressions': 'off',
    '@typescript-eslint/no-confusing-void-expression': 'off',
    '@typescript-eslint/consistent-type-imports': 'error',
    '@typescript-eslint/consistent-type-exports': 'error',
    '@typescript-eslint/no-import-type-side-effects': 'error',
  },
  overrides: [
    {
      files: [
        '.eslintrc.cjs',
        '.lintstagedrc.js',
        'postcss.config.js',
        'vite.config.ts',
      ],
      env: {
        node: true,
        es2022: true,
      },
    },
    {
      files: ['src/**'],
      env: {
        browser: true,
        es2022: true,
      },
    },
  ],
};
