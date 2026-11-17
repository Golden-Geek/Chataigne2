import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { chromium } from 'playwright-core';
import { gridSelectionChecks } from './selection.mjs';
import { gridAppearanceChecks } from './appearance.mjs';

const configFile = fileURLToPath(
	new URL('../../../screens/tests/browser/vite.config.ts', import.meta.url)
);
await build({ configFile, logLevel: 'warn' });
const server = await preview({ configFile, logLevel: 'warn' });
let browser;
try {
	const address = server.httpServer.address();
	assert(address && typeof address === 'object');
	const executablePath = process.env.GOLDEN_PREVIEW_BROWSER_PATH;
	browser = await chromium.launch({
		executablePath,
		channel: executablePath ? undefined : 'msedge',
		headless: true
	});
	const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
	/** @type {string[]} */
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`http://127.0.0.1:${address.port}`);
	await gridSelectionChecks(page);
	await gridAppearanceChecks(page);
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Production Matrix grid: selection/colors, rounded quiet cards, active-state isolation, stable geometry, interrupted transitions, reduced motion, narrow/scaled themes and independent panels; no GPU timing qualification'
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
