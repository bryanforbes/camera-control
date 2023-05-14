import { ESLint } from 'eslint';

/**
 * @param {string[]} files
 */
const removeIgnoredFiles = async (files) => {
  const eslint = new ESLint();
  const isIgnored = await Promise.all(
    files.map((file) => {
      return eslint.isPathIgnored(file);
    })
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
