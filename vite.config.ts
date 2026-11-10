// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwind from '@tailwindcss/vite';
import nodePath from 'node:path';
import { isMainThread } from 'node:worker_threads';
import { defineConfig, type ServerOptions } from 'vite';
import checker from 'vite-plugin-checker';

const host = process.env.TAURI_DEV_HOST ?? '';

const watchedDirectories = new Set(['src']);
const watchedFiles = new Set([
  'vite.config.ts',
  'package.json',
  'pnpm-lock.yaml',
  'tsconfig.json',
  'tsconfig.base.json',
  'tsconfig.node.json',
]);

const server: ServerOptions = {
  // tauri expects a fixed port, fail if that port is not available
  port: 1420,
  strictPort: true,
  host: host || false,
  watch: {
    ignored(file) {
      const path = nodePath.relative(import.meta.dirname, file);
      if (path === '') return false;

      const [topLevel] = path.split(nodePath.sep);
      return (
        !watchedDirectories.has(topLevel) &&
        !watchedFiles.has(path) &&
        !/^\.env(?:\.[^/\\]+)?$/.test(path)
      );
    },
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
    // Svelte Check loads this config again from its worker.
    isMainThread &&
      checker({
        enableBuild: false,
        svelteCheck: {
          flags: '--tsconfig ./tsconfig.json',
        },
      }),
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
      version: {
        pollInterval: 0,
      },
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  // prevent vite from obscuring rust errors
  clearScreen: false,

  server,
});
