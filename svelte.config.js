// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  extensions: ['.svelte'],
  // Consult https://kit.svelte.dev/docs/integrations#preprocessors
  // for more information about preprocessors
  preprocess: [vitePreprocess()],

  kit: {
    adapter: adapter(),
  },

  vitePlugin: {
    dynamicCompileOptions: ({ filename, compileOptions }) => {
      if (!filename.includes('/node_modules/') && !compileOptions.runes) {
        return { ...compileOptions, runes: true };
      }
      return;
    },
  },
};
export default config;
