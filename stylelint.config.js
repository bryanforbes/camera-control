/** @type {import('stylelint').Config} */
export default {
  extends: 'stylelint-config-standard',
  plugins: ['stylelint-order'],
  rules: {
    'order/properties-alphabetical-order': true,
    'import-notation': null,
    'at-rule-no-unknown': [
      true,
      {
        ignoreAtRules: [
          'theme',
          'source',
          'utility',
          'variant',
          'custom-variant',
          'plugin',
        ],
      },
    ],
  },
};
