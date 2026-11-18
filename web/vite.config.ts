import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  base: process.env.BASE_PATH || './',
  plugins: [svelte()],
  worker: {
    format: 'es',
  },
  build: {
    target: 'es2022',
    assetsInlineLimit: 0,
  },
  optimizeDeps: {
    exclude: ['./src/lib/wasm-pkg/typst2word_wasm.js'],
  },
});
