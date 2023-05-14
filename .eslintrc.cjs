module.exports = {
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:@typescript-eslint/recommended-requiring-type-checking',
  ],
  parser: '@typescript-eslint/parser',
  plugins: ['@typescript-eslint'],
  parserOptions: {
    project: true,
    tsconfigRootDir: __dirname,
  },
  root: true,
  ignorePatterns: ['src-tauri/**', 'dist*/**'],
  rules: {
    '@typescript-eslint/restrict-template-expressions': 'off',
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
