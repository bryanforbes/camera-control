/**
 * @type {import('lint-staged').Config}
 */
export default {
  '*.{ts,js,cjs}': 'eslint --max-warnings=0 --no-warn-ignored',
  '**/*.css': 'stylelint',
  '*': 'prettier --ignore-unknown --write',
  'src-tauri/**/*.rs': 'rustfmt',
};
