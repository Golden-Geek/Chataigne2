import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	ssr: {
		noExternal: ['golden_audio_ui', 'golden_graph_ui', 'golden_ui', 'dockview-core']
	},
	server: {
		fs: {
			allow: ['../../..']
		}
	}
});
