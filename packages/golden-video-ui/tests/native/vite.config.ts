import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	root: fileURLToPath(new URL('./', import.meta.url)),
	base: './',
	plugins: [svelte({ configFile: false })],
	build: { outDir: process.env.GC_UI_OUT_DIR ?? '../native-dist', emptyOutDir: true }
});
