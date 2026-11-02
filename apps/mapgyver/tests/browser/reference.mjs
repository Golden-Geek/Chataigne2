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
	const channelReads = [];
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (
					frame.kind === 'intent' &&
					frame.intent?.kind === 'invokeAppOperation' &&
					frame.intent.namespace === 'mapgyver.media'
				) {
					const operation = frame.intent.payload;
					if (operation?.kind === 'InspectChannels') channelReads.push(operation);
				}
			} catch {}
		})
	);
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
		const channels = page.getByRole('region', { name: 'Channels', exact: true });
		await channels.getByText('Choose a target.', { exact: true }).waitFor();
		await screenParameterControls(page).reference('Target', 'Reference target', 'Reference target');
		const bound = await wait((media) => media?.channels.length === 1);
		assert.equal(bound.id, initial.id);
		assert.deepEqual(bound.channels[0].reference, { media: initial.id, channel: source.channel });
		assert.deepEqual(bound.channels[0].definition.source, { kind: 'Reference', target: source });
		await channels.locator(`[data-channel="${source.channel}"]`).waitFor();
		assert.equal(
			await channels.getByRole('button', { name: 'Add child item', exact: true }).count(),
			0
		);
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
		await channels.locator(`[data-channel="${peer.channel}"]`).waitFor();
		await edit({ kind: 'RenameChannel', reference: peer, label: 'Renamed mirror' });
		await channels.getByText('Renamed mirror', { exact: true }).waitFor();
		await edit({ kind: 'ReorderChannel', reference: peer, after: null });
		// Read the rendered order after the authoritative history delta, rather than a UI copy.
		await page.waitForFunction(
			(id) => document.querySelector('.reference-channels li')?.getAttribute('data-channel') === id,
			peer.channel
		);
		await channels
			.getByRole('button', { name: 'Edit target channel Renamed mirror', exact: true })
			.click();
		await parameter('Color').waitFor();
		assert.equal(await parameter('Target').count(), 0);
		await parameter('Color').locator('.cp-preview').click();
		await parameter('Color').locator('.cp-input-hex').fill('0000ff');
		await parameter('Color').locator('.cp-input-hex').press('Tab');
		const deadline = Date.now() + 5000;
		while (true) {
			const { snapshot } = await edit({ kind: 'InspectChannel', reference: peer });
			if (JSON.stringify(snapshot.definition.source.rgba) === '[0,0,1,1]') break;
			assert(Date.now() < deadline, JSON.stringify(snapshot));
			await delay(20);
		}
		assert.deepEqual(
			(await edit({ kind: 'InspectChannel', reference: source })).snapshot.definition.source.rgba,
			[1, 1, 1, 1]
		);
		await manager
			.getByRole('treeitem', { name: bound.label, exact: true })
			.getByRole('button', { name: bound.label, exact: true })
			.click();
		await channels.getByText('Renamed mirror', { exact: true }).waitFor();
		for (let index = 2; index < 33; index++) {
			await edit({ kind: 'AddChannel', media: source.media, label: `Variant ${index}` });
		}
		await channels.getByRole('button', { name: 'Next channels', exact: true }).click();
		await page.waitForFunction(
			() => document.querySelector('.reference-channels ul')?.children.length === 1
		);
		await channels.getByText('Variant 32', { exact: true }).waitFor();
		assert.equal(await channels.locator('li').count(), 1);
		const all = await wait((media) => media?.channels.length === 33);
		await edit({
			kind: 'RemoveChannel',
			reference: { media: source.media, channel: all.channels.at(-1).reference.channel }
		});
		await page.waitForFunction(
			() => document.querySelector('.reference-channels ul')?.children.length === 32
		);
		assert.equal(
			await channels.getByRole('button', { name: 'Next channels', exact: true }).count(),
			0
		);
		await edit({ kind: 'RemoveMedia', media: source.media });
		await channels.getByText('Target unavailable.', { exact: true }).waitFor();
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await channels.getByText('Renamed mirror', { exact: true }).waitFor();
		await edit({ kind: 'ConfigureReference', media: bound.id, target: null });
		await channels.getByText('Choose a target.', { exact: true }).waitFor();
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await channels.getByText('Renamed mirror', { exact: true }).waitFor();
		assert(channelReads.some((read) => read.kind === 'InspectChannels' && read.query.start === 32));
		assert(
			channelReads.every((read) => read.kind === 'InspectChannels' && read.query.count === 32)
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled Reference target/Resolution/history, live mirrored rows, bounded paging, target-channel Inspector navigation and missing-target recovery'
			})
		);
	} finally {
		await browser.close();
	}
}
