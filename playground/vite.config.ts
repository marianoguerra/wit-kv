import { defineConfig } from 'vite';

export default defineConfig({
  base: './',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Support top-level await in witast.js (generated WASM bindings)
    target: 'esnext',
  },
  // Suppress node:fs/promises warning for witast.js (it handles the fallback)
  resolve: {
    alias: {
      'node:fs/promises': 'data:text/javascript,export default {}',
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/health': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
});
