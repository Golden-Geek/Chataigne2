import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	root: fileURLToPath(new URL('./', import.meta.url)),
	plugins: [svelte({ configFile: false })],
	build: {
		outDir: fileURLToPath(
			new URL('../../../../../../../../target/m14-screen-selection/', import.meta.url)
		),
		emptyOutDir: true
	},
	preview: { host: '127.0.0.1', port: 0, strictPort: true }
});
