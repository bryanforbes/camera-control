/**
 * @type {import('lint-staged').Configuration}
 */
export default {
  '*.{ts,js,cjs,svelte}': 'eslint --max-warnings=0 --no-warn-ignored',
  '**/*.css': 'stylelint',
  '*': 'prettier --ignore-unknown --write',
  'src-tauri/**/*.rs': 'rustfmt',
};
