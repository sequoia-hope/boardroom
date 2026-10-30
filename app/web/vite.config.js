import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  base: './',
  plugins: [svelte()],
  // The 3D view's chunk is three.js (~580 kB), loaded only when 3D opens.
  build: { target: 'es2022', chunkSizeWarningLimit: 700 },
});
