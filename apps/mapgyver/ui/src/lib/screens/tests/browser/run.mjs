import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { chromium } from 'playwright-core';

const configFile = fileURLToPath(new URL('./vite.config.ts', import.meta.url));
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
	const view = page.getByLabel('Warp fixture', { exact: true });
	const owner = page.getByLabel('Inspected owner');
	/** @param {string} label */
	const polygon = (label) => page.getByRole('button', { name: label, exact: true });
	const first = polygon('Select A');
	const second = polygon('Select B (invalid warp shape)');
	/** @param {import('playwright-core').Locator} element */
	const point = async (element) => {
		const box = await element.boundingBox();
		assert(box);
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const a = await point(first),
		b = await point(second);
	await page.mouse.move(a.x, a.y);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'A', 'pointer down selects before release');
	assert.equal(
		await page.getByLabel('Geometry starts').textContent(),
		'0',
		'selection creates no edit'
	);
	assert.equal(await page.getByRole('group', { name: 'Warp handles for A' }).count(), 1);
	assert.equal(await page.getByRole('group', { name: 'Warp handles for B' }).count(), 0);
	await page.mouse.up();
	await page.mouse.move(b.x, b.y);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'B',
		'unavailable source/invalid geometry remains selectable'
	);
	await page.mouse.up();
	const zone = await view.locator('.active-zone').boundingBox();
	assert(zone);
	await page.mouse.move(zone.x + zone.width / 2, zone.y + zone.height / 2);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'screen', 'background inspects owner before release');
	assert.equal(await page.getByRole('group', { name: /Warp handles for/ }).count(), 0);
	await page.mouse.up();
	await first.focus();
	await page.keyboard.press('Enter');
	assert.equal(await owner.textContent(), 'A', 'keyboard selection preserved');
	await page.keyboard.press('Escape');
	assert.equal(await owner.textContent(), 'screen', 'idle Escape clears child selection');
	await first.focus();
	await page.keyboard.press('Enter');
	await view.focus();
	await page.keyboard.down('Space');
	await page.mouse.move(b.x, b.y);
	await page.mouse.down();
	await page.mouse.move(b.x + 12, b.y + 7);
	await page.mouse.up();
	await page.keyboard.up('Space');
	assert.equal(await owner.textContent(), 'A', 'pan does not select or clear');
	assert.equal(await page.getByLabel('Geometry starts').textContent(), '0');
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'DOM Screen primary-down/background/idle Escape/keyboard/pan and selected-only handles; no native pixel or full M14 acceptance'
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
