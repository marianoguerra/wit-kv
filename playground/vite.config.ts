import { defineConfig } from 'vite';

export default defineConfig({
  base: './',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Support top-level await in witast.js (generated WASM bindings)
    target: 'esnext',
    rollupOptions: {
      // Mark node builtins as external for build
      external: ['node:fs/promises'],
    },
  },
  optimizeDeps: {
    // Exclude witast from pre-bundling to avoid node:fs/promises resolution
    exclude: ['node:fs/promises'],
    esbuildOptions: {
      // Support top-level await in dev mode
      target: 'esnext',
    },
  },
  resolve: {
    alias: {
      // Provide empty stub for node:fs/promises (witast.js checks isNode before using it)
      'node:fs/promises': `data:text/javascript,export default {};export const readFile = () => { throw new Error('not available in browser'); };`,
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
