/** @typedef {typeof import('eslint').ESLint} ESLintCtor */
import _eslint from 'eslint/use-at-your-own-risk';
/** @type {typeof import('eslint').ESLint} */
// @ts-expect-error FlatESLint doesn't exist in the eslint types
// eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
const FlatESLint = _eslint.FlatESLint;

/**
 * @param {string[]} files
 */
const removeIgnoredFiles = async (files) => {
  const eslint = new FlatESLint();
  const isIgnored = await Promise.all(
    files.map((file) => {
      return eslint.isPathIgnored(file);
    }),
  );
  const filteredFiles = files.filter((_, i) => !isIgnored[i]);
  return filteredFiles.join(' ');
};

export default {
  /**
   * @param {string[]} files
   */
  '**/*.{ts,js,cjs}': async (files) => {
    const filesToLint = await removeIgnoredFiles(files);
    return [`eslint --max-warnings=0 ${filesToLint}`];
  },
  '**/*.css': 'stylelint',
  '*': 'prettier --ignore-unknown --write',
};
