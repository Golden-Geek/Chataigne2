// Real generic Inspector edits against the bundled app; domain operations only set up/read fixtures.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMediaResolutionUi({ base, edit, inspectMedia }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Resolution review'
	});
	const { reference: peer } = await edit({
		kind: 'AddChannel',
		media: reference.media,
		label: null
	});
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'shared media Resolution');
	const read = async () =>
		(await inspectMedia()).media.find((media) => media.id === reference.media);
	async function wait(test) {
		const deadline = Date.now() + 5000;
		while (true) {
			const media = await read();
			if (test(media)) return media;
			assert(Date.now() < deadline, JSON.stringify(media));
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.locator('section[aria-label="Media Manager"]')
			.getByRole('treeitem', { name: 'Resolution review', exact: true })
			.getByRole('button', { name: 'Resolution review', exact: true })
			.click();
		const resolution = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Resolution', { exact: true }) });
		await resolution.waitFor();
		const toggle = resolution.getByRole('switch', { name: 'Toggle node enabled' });
		assert.equal(await toggle.getAttribute('aria-checked'), 'false');
		await toggle.click();
		await wait((media) =>
			media.channels.every((channel) => channel.definition.size.kind === 'Fixed')
		);
		const inputs = resolution.locator('input.number-field');
		assert.equal(await inputs.count(), 2);
		for (const [index, pixels] of [640, 360].entries()) {
			await inputs.nth(index).fill(String(pixels));
			await inputs.nth(index).press('Enter');
		}
		await wait((media) =>
			media.channels.every(
				(channel) =>
					channel.definition.size.extent?.width === 640 &&
					channel.definition.size.extent?.height === 360
			)
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((media) =>
			media.channels.every((channel) => channel.definition.size.extent?.height === 1080)
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait((media) =>
			media.channels.every((channel) => channel.definition.size.extent?.height === 360)
		);
		await toggle.click();
		const media = await wait((media) =>
			media.channels.every((channel) => channel.definition.size.kind === 'Automatic')
		);
		assert.deepEqual(
			media.channels.map((channel) => channel.reference),
			[reference, peer]
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled generic media Resolution enable/pixels/Undo/Redo/disable across two stable variants'
			})
		);
	} finally {
		await browser.close();
	}
}
