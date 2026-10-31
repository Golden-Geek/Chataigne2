// Real bundled UI against the ordinary host used by tests/headless/media.mjs --ui-reload.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyPictureReloadUi({ base, inspect, picture, asset }) {
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
		page.setDefaultTimeout(5000);
		const errors = [];
		const reloadRequests = [];
		page.on('pageerror', (error) => errors.push(String(error)));
		page.on('websocket', (socket) => {
			socket.on('framesent', ({ payload }) => {
				if (typeof payload === 'string' && payload.includes('ReloadAsset'))
					reloadRequests.push(JSON.parse(payload));
			});
		});
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Media Manager$/ });
		await tab.click();
		const manager = page.locator('section.manager');
		const before = await inspect();
		const media = before.media.find((entry) => entry.id === picture.first.media);
		const selected = media.channels.find(
			(entry) => entry.reference.channel === picture.first.channel
		);
		const article = manager.locator('article').filter({
			has: page.getByRole('textbox', { name: `Media name: ${media.label}`, exact: true })
		});
		await article.locator('button.channel').filter({ hasText: selected.label }).click();
		const reload = manager.getByRole('button', { name: 'Reload Picture', exact: true });
		await reload.waitFor();
		assert.equal(await reload.isEnabled(), true);
		await reload.click();
		await page.waitForFunction(() => {
			const manager = document.querySelector('section.manager');
			return (
				manager &&
				[...manager.querySelectorAll('button')].some(
					(button) => button.textContent === 'Refresh' && !button.disabled
				)
			);
		});
		assert.equal(reloadRequests.length, 1);
		assert(
			JSON.stringify(reloadRequests[0]).includes(asset),
			'UI must send the selected asset identity'
		);
		assert.deepEqual(await inspect(), before);
		assert.equal(await manager.getByRole('alert').count(), 0);
		// The previous authored step is RemoveMedia/undo. Reload must not consume its redo entry.
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await page.waitForFunction(
			(label) =>
				![...document.querySelectorAll('section.manager input')].some(
					(input) => input.getAttribute('aria-label') === `Media name: ${label}`
				),
			media.label
		);
		assert.equal(
			(await inspect()).media.some((entry) => entry.id === picture.first.media),
			false
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await manager
			.getByRole('textbox', { name: `Media name: ${media.label}`, exact: true })
			.waitFor();
		assert.deepEqual(await inspect(), before);
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope: 'real Reload Picture UI, unchanged catalog/IDs and preserved redo/undo',
				browser: browser.version(),
				asset,
				page_errors: errors.length
			})
		);
	} finally {
		await browser.close();
	}
}
