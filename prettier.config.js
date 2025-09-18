/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config}
 */
export default {
  singleQuote: true,
  experimentalTernaries: true,
  plugins: [
    'prettier-plugin-svelte',
    'prettier-plugin-organize-imports',
    'prettier-plugin-tailwindcss',
  ],
  overrides: [{ files: '*.svelte', options: { parser: 'svelte' } }],
};
