import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { env } from 'node:process';
import { defineConfig } from 'vite';
import { uiContentSecurityPolicy } from 'golden_ui/host/content-security-policy';

const outputDir = env.GC_UI_OUT_DIR ?? 'build';

export default defineConfig({
	plugins: [
		sveltekit({
			csp: uiContentSecurityPolicy,
			adapter: adapter({ pages: outputDir, assets: outputDir, fallback: 'index.html' }),
			prerender: { handleUnseenRoutes: 'ignore' }
		})
	],
	ssr: {
		noExternal: ['golden_audio_ui', 'golden_graph_ui', 'golden_ui', 'dockview-core']
	},
	server: {
		fs: {
			allow: ['../../..']
		}
	}
});
