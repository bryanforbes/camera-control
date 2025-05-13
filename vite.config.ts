import { sveltekit } from '@sveltejs/kit/vite';
import tailwind from '@tailwindcss/vite';
import { defineConfig, type ServerOptions } from 'vite';

const host = process.env['TAURI_DEV_HOST'] ?? '';

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
  plugins: [tailwind(), sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  // prevent vite from obscuring rust errors
  clearScreen: false,

  server,
});
