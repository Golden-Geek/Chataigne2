import adapter from '@sveltejs/adapter-static';

const outputDir = process.env.GC_UI_OUT_DIR ?? 'build';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: outputDir,
			assets: outputDir,
			fallback: 'index.html'
		}),
		alias: {
			'$gc-ui': '$lib/golden_ui'
		},
		prerender: {
			handleUnseenRoutes: 'ignore'
		}
	}
};

export default config;
