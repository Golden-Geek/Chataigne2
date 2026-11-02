import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyChannelsUi({ base, edit, inspectMedia }) {
	const { reference: first } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Channel review'
	});
	const read = async () => (await inspectMedia()).media.find((media) => media.id === first.media);
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [],
		requests = [];
	let stage = 'manager and Channels section';
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent') requests.push(frame.intent);
			} catch {}
		})
	);
	const rejected = observeRejectedRequests(page, () => stage);
	async function wait(test) {
		const deadline = Date.now() + 5000;
		while (true) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
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
		const mediaRow = manager.getByRole('treeitem', { name: 'Channel review', exact: true });
		await mediaRow.getByRole('button', { name: 'Channel review', exact: true }).click();
		const channels = page.getByRole('region', { name: 'Channels', exact: true });
		await channels.waitFor();
		assert.equal(
			await mediaRow.getByRole('button', { name: 'Add child item', exact: true }).count(),
			0
		);
		assert.equal(
			await manager.getByRole('treeitem', { name: 'Channel 1', exact: true }).count(),
			0
		);
		const entries = channels.locator(
			':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
		);
		const entry = (label) => entries.filter({ has: page.getByText(label, { exact: true }) });
		const header = (label) => entry(label).locator(':scope > .node-header > .node-title');
		const menu = async (label, action) => {
			await header(label).click({ button: 'right' });
			await page.getByRole('menuitem', { name: new RegExp(`^${action}(?:\\s|$)`) }).click();
		};
		await channels.getByRole('button', { name: 'Add child item', exact: true }).click();
		const added = await wait((media) => media.channels.length === 2);
		const peer = added.channels.find((channel) => channel.reference.channel !== first.channel);
		assert(peer);
		await header(added.channels[0].label).press('Enter');
		const color = entry(added.channels[0].label)
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Color', { exact: true }) });
		await color.waitFor();
		assert.equal(
			await entry(added.channels[0].label)
				.getByRole('button', { name: 'Add child item', exact: true })
				.count(),
			0
		);
		stage = 'generic typed variant editing and history';
		await color.locator('.cp-preview').click();
		await color.locator('.cp-input-hex').fill('ff0000');
		await color.locator('.cp-input-hex').press('Tab');
		const red = (media) =>
			media.channels.find((channel) => channel.reference.channel === first.channel);
		await wait((media) => JSON.stringify(red(media).definition.source.rgba) === '[1,0,0,1]');
		assert.deepEqual(
			(await read()).channels.find(
				(channel) => channel.reference.channel === peer.reference.channel
			),
			peer
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((media) => JSON.stringify(red(media).definition.source.rgba) === '[1,1,1,1]');
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait((media) => JSON.stringify(red(media).definition.source.rgba) === '[1,0,0,1]');
		stage = 'generic rename, context reorder, copy and removal';
		await entry(added.channels[0].label).locator(':scope > .node-header .title-text').dblclick();
		await channels.locator('input.node-title-input').fill('Red variant');
		await channels.locator('input.node-title-input').press('Enter');
		await wait((media) => red(media).label === 'Red variant');
		await menu('Red variant', 'Move later');
		await wait((media) => media.channels[1].reference.channel === first.channel);
		await menu('Red variant', 'Duplicate');
		const copied = await wait((media) => media.channels.length === 3);
		const copy = copied.channels.find(
			(channel) => ![first.channel, peer.reference.channel].includes(channel.reference.channel)
		);
		assert(copy);
		assert.deepEqual(copy.definition.source.rgba, [1, 0, 0, 1]);
		await menu(copy.label, 'Delete');
		await wait((media) => media.channels.length === 2);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((media) =>
			media.channels.some((channel) => channel.reference.channel === copy.reference.channel)
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait((media) => media.channels.length === 2);
		stage = 'bounded collection rendering';
		for (let index = 0; index < 33; ++index)
			await edit({ kind: 'AddChannel', media: first.media, label: null });
		await wait((media) => media.channels.length === 35);
		await channels.getByRole('button', { name: 'Next channels', exact: true }).waitFor();
		assert.equal(await entries.count(), 32);
		await channels.getByRole('button', { name: 'Next channels', exact: true }).click();
		await page.waitForFunction(
			() =>
				document.querySelectorAll(
					'.channels-section > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
				).length === 3
		);
		const inspected = requests.filter(
			(intent) => intent.kind === 'invokeAppOperation' && intent.payload?.kind === 'InspectChannel'
		);
		assert(inspected.length > 0, 'Expanded variants use exact channel inspection');
		assert(
			inspected.every((intent) => intent.payload.reference.channel === first.channel),
			'Collapsed variants must not trigger channel inspection'
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled media-only manager, typed Channels add/color/rename/reorder/copy/remove/history, closed Parameter folders, exact reads and 32-entry rendering bound'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-4500)
			})
		);
		await page.screenshot({ path: 'target/mapgyver-channels-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
