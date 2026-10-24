import adapter from '@sveltejs/adapter-static';
import { uiContentSecurityPolicy } from '../../../packages/golden-ui/host/content-security-policy.mjs';

const outputDir = process.env.GC_UI_OUT_DIR ?? 'build';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		csp: uiContentSecurityPolicy,
		adapter: adapter({
			pages: outputDir,
			assets: outputDir,
			fallback: 'index.html'
		}),
		prerender: {
			handleUnseenRoutes: 'ignore'
		}
	}
};

export default config;
