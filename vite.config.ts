// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwind from '@tailwindcss/vite';
import { defineConfig, type ServerOptions } from 'vite';

const host = process.env.TAURI_DEV_HOST ?? '';

const server: ServerOptions = {
  // tauri expects a fixed port, fail if that port is not available
  port: 1420,
  strictPort: true,
  host: host || false,
  watch: {
    ignored: ['**/src-tauri/**'],
  },
};

if (host) {
  server.hmr = {
    protocol: 'ws',
    host,
    port: 1421,
  };
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    tailwind(),
    sveltekit({
      extensions: ['.svelte'],
      // Consult https://kit.svelte.dev/docs/integrations#preprocessors
      // for more information about preprocessors
      preprocess: [vitePreprocess()],
      dynamicCompileOptions: ({ filename, compileOptions }) => {
        if (!filename.includes('/node_modules/') && !compileOptions.runes) {
          return { ...compileOptions, runes: true };
        }

        return;
      },
      adapter: adapter(),
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  // prevent vite from obscuring rust errors
  clearScreen: false,

  server,
});
