import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  base: './',
  plugins: [svelte()],
  // The store (web/store/) is shared by every mode.
  // The renderer is crates/render/js, outside web/: the dev server may read the repository.
  server: { fs: { allow: ['..'] } },
  
  // The 3D view's chunk is three.js (~580 kB), loaded only when 3D opens.
  build: { target: 'es2022', chunkSizeWarningLimit: 700 },
});
