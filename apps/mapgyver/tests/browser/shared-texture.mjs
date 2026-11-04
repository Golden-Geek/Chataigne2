import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifySharedTextureUi({ base, edit, inspectMedia }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Sender ownership'
	});
	const { reference: peer } = await edit({
		kind: 'AddChannel',
		media: reference.media,
		label: null
	});
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'sender-only Shared Texture');
	const read = async () =>
		(await inspectMedia()).media.find((media) => media.id === reference.media);
	async function wait(sender) {
		const deadline = Date.now() + 5000;
		while (true) {
			const media = await read();
			const first = media.channels.find(
				(channel) => channel.reference.channel === reference.channel
			);
			const second = media.channels.find((channel) => channel.reference.channel === peer.channel);
			assert.deepEqual(second.definition.source, { kind: 'Spout', sender: null });
			if (first.definition.source.sender === sender) {
				assert.deepEqual(first.definition.source, { kind: 'Spout', sender });
				return;
			}
			assert(Date.now() < deadline, JSON.stringify(media));
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page.getByRole('tab').first().click();
		await page
			.locator('section[data-panel-id="media-manager"]')
			.getByRole('treeitem', { name: 'Sender ownership', exact: true })
			.getByRole('button', { name: 'Sender ownership', exact: true })
			.click();
		const channels = page.getByRole('region', { name: 'Channels', exact: true });
		const label = (await read()).channels.find(
			(channel) => channel.reference.channel === reference.channel
		).label;
		const entry = channels
			.locator('.node-inspector')
			.filter({ has: page.getByText(label, { exact: true }) })
			.first();
		const title = entry.locator(':scope > .node-header > .node-title');
		if ((await title.getAttribute('aria-checked')) === 'false') await title.press('Enter');
		const sender = entry
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Spout sender', { exact: true }) });
		await sender.waitFor();
		for (const label of [
			'Maximum width',
			'Maximum height',
			'Pixel format',
			'Color encoding',
			'Alpha mode'
		])
			assert.equal(await entry.getByText(label, { exact: true }).count(), 0);
		const input = sender.locator('input[type="text"]');
		await input.fill('Exact user sender');
		await input.press('Tab');
		await wait('Exact user sender');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(null);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait('Exact user sender');
		assert.equal(
			await entry.getByRole('button', { name: 'Detect sender', exact: true }).count(),
			1
		);
		const mode = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Sender selection', { exact: true }) })
			.locator('select');
		await mode.selectOption('Auto');
		await input.waitFor({ state: 'detached' });
		const auto = entry.locator('select.dropdown-editor');
		await auto.waitFor();
		await auto
			.locator('option')
			.filter({ hasText: /^Unconfigured$/ })
			.waitFor({ state: 'attached' });
		assert.equal(await auto.inputValue(), 'Exact user sender');
		assert.match(await auto.locator('option:checked').textContent(), /unavailable/);
		assert.equal(
			await entry.getByRole('button', { name: 'Detect sender', exact: true }).count(),
			0
		);
		assert.equal((await read()).sender_selection, 'Auto');
		await mode.selectOption('Manual');
		await input.waitFor();
		assert.equal(await input.inputValue(), 'Exact user sender');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await auto.waitFor();
		assert.equal(await auto.inputValue(), 'Exact user sender');
		await wait('Exact user sender');
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled Shared Texture Manual/Auto Parameters, headless live catalog metadata, missing selection, independent variants and Undo/Redo'
			})
		);
	} finally {
		await browser.close();
	}
}
