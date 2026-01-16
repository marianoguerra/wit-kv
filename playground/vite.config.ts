import { defineConfig } from 'vite';

export default defineConfig({
  base: './',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Support top-level await in witast.js (generated WASM bindings)
    target: 'esnext',
  },
  optimizeDeps: {
    esbuildOptions: {
      // Support top-level await in dev mode
      target: 'esnext',
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
