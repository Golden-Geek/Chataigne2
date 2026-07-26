import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [svelte()],
	ssr: {
		noExternal: ['golden_ui', 'dockview-core']
	},
	test: {
		environment: 'node',
		include: ['tests/**/*.test.ts']
	}
});
