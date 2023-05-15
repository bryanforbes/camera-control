import autoprefixer from 'autoprefixer';
import nesting from 'postcss-nesting';

const config = {
  plugins: [nesting(), autoprefixer()],
};

export default config;
