import assert from 'node:assert/strict';
import path from 'node:path';
import { preview } from 'vite';
import { chromium } from 'playwright-core';

const server = await preview({
	configFile: path.resolve('tests/native/vite.config.ts'),
	preview: { host: '127.0.0.1', port: 0, strictPort: true },
	logLevel: 'warn'
});
let browser;
try {
	const address = server.httpServer.address();
	assert(address && typeof address === 'object');
	const executablePath = process.env.GOLDEN_PREVIEW_BROWSER_PATH;
	browser = await chromium.launch({
		executablePath,
		channel: executablePath ? undefined : process.platform === 'win32' ? 'msedge' : undefined,
		headless: true
	});
	const page = await browser.newPage();
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.addInitScript(() => {
		window.isTauri = true;
		let views = [],
			revision = 0;
		window.__previewMockRequests = [];
		window.__TAURI_INTERNALS__ = {
			invoke: async (command, args) => {
				if (command === 'plugin:event|emit') return;
				if (command !== 'native_presentation_control') throw new Error(command);
				const request = JSON.parse(args.request);
				window.__previewMockRequests.push(request.kind);
				if (request.kind === 'Connect')
					return JSON.stringify({
						kind: 'Connected',
						session: '35537689-b168-461d-90a9-7d8566eafdd5',
						version: 1,
						max_views: 64
					});
				if (request.kind === 'Disconnect') return JSON.stringify({ kind: 'Disconnected' });
				if (request.kind === 'Layout') {
					views = request.views;
					revision = request.revision;
				}
				// Metadata acceptance intentionally precedes installation, exercising real browser
				// ResizeObserver/animation-frame ordering that pure fake-clock tests cannot provide.
				return JSON.stringify({
					kind: 'Applied',
					revision,
					visible_views: views.filter((view) => view.visible).length,
					installed: request.kind === 'Status'
				});
			}
		};
	});
	await page.goto(`http://127.0.0.1:${address.port}`);
	await page.waitForFunction(
		() => ['done', 'failed'].includes(window.__previewSmoke?.phase),
		{},
		{ timeout: 10000 }
	);
	const report = await page.evaluate(() => window.__previewSmoke);
	assert.equal(report.phase, 'done', JSON.stringify(report));
	assert.equal(report.error, null);
	assert.deepEqual(
		report.trace.map((item) => item.state.visibleViews),
		[2, 2, 1, 0, 0]
	);
	assert(report.maxRetirementMasks > 0 && report.maxRetirementMasks <= 64);
	assert(report.trace.every((item) => item.retirementMasks === 0 && item.state.installed));
	const clipped = report.trace
		.find((item) => item.phase === 'clipped')
		.views.find((view) => view.source === 'red');
	assert(
		clipped.rect.x < clipped.clip.x,
		'partial clipping must preserve the complete source rectangle'
	);
	assert(report.trace.slice(1).every((item) => item.overlayClicks === 1));
	const requests = await page.evaluate(() => window.__previewMockRequests);
	assert(requests.includes('Status'));
	assert.equal(requests.at(-1), 'Disconnect');
	assert.deepEqual(errors, []);

	const browserPage = await browser.newPage();
	const browserErrors = [];
	browserPage.on('pageerror', (error) => browserErrors.push(error.message));
	await browserPage.goto(`http://127.0.0.1:${address.port}`);
	await browserPage.waitForFunction(
		() => window.__previewSmoke?.phase === 'failed',
		{},
		{ timeout: 5000 }
	);
	assert.match(
		await browserPage.evaluate(() => window.__previewSmoke.error),
		/capability unavailable/
	);
	assert.equal(await browserPage.getByText('Preview unavailable', { exact: true }).count(), 2);
	await browserPage.getByRole('button', { name: 'DOM overlay control' }).click();
	await browserPage.getByText('Overlay clicks: 1', { exact: true }).waitFor();
	assert.deepEqual(browserErrors, []);
	console.log(
		JSON.stringify({
			fakeNativeVisibleCounts: report.trace.map((item) => item.state.visibleViews),
			retirementMaskPeak: report.maxRetirementMasks,
			metadataRequests: requests.length,
			browserUnavailableEditingControl: true,
			pageErrors: []
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
