import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { screenParameterControls } from './screen-parameters.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyReferenceUi({ base, edit, inspectMedia }) {
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Reference target'
	});
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Reference authoring');
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	async function wait(test) {
		const deadline = Date.now() + 5000;
		while (true) {
			const media = (await inspectMedia()).media.find((media) => media.media_kind === 'Reference');
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
		const manager = page.locator('section[aria-label="Media Manager"]');
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name: 'Reference', exact: true }).click();
		await parameter('Target').waitFor();
		assert.equal(await parameter('Color').count(), 0);
		const initial = await wait((media) => media?.channels.length === 0);
		await screenParameterControls(page).reference('Target', 'Reference target', 'Reference target');
		const bound = await wait((media) => media?.channels.length === 1);
		assert.equal(bound.id, initial.id);
		assert.deepEqual(bound.channels[0].reference, { media: initial.id, channel: source.channel });
		assert.deepEqual(bound.channels[0].definition.source, { kind: 'Reference', target: source });
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((media) => media?.channels.length === 0);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait((media) => media?.channels.length === 1);
		await parameter('Resolution').getByRole('switch', { name: 'Toggle node enabled' }).click();
		await wait((media) => media?.channels[0].definition.size.kind === 'Fixed');
		for (const [index, value] of [17, 9].entries()) {
			const field = parameter('Resolution').locator('input.number-field').nth(index);
			await field.fill(String(value));
			await field.press('Enter');
		}
		await wait(
			(media) =>
				media?.channels[0].definition.size.extent?.width === 17 &&
				media.channels[0].definition.size.extent.height === 9
		);
		const { reference: peer } = await edit({
			kind: 'AddChannel',
			media: source.media,
			label: null
		});
		const mirrored = await wait((media) => media?.channels.length === 2);
		assert.deepEqual(
			mirrored.channels.map((channel) => channel.reference.channel),
			[source.channel, peer.channel]
		);
		assert(mirrored.channels.every((channel) => channel.definition.size.extent.width === 17));
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled Reference creation, generic target picker/resolution, Undo/Redo and virtual target-channel mirroring'
			})
		);
	} finally {
		await browser.close();
	}
}
