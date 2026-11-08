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
	page.setDefaultTimeout(6000);
	const errors = [],
		reads = [],
		fullTreeRequests = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('request', (request) => {
		if (request.url().endsWith('/api/ui/reference-targets')) fullTreeRequests.push(request.url());
	});
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				const edit = frame.intent?.payload;
				if (frame.intent?.namespace === 'mapgyver.media' && edit?.kind === 'InspectTargetPicker')
					reads.push(edit.query);
			} catch {}
		})
	);
	const rejected = observeRejectedRequests(page, () => 'Reference authoring');
	const controls = screenParameterControls(page);
	const parameter = controls.parameter;
	async function wait(predicate) {
		const deadline = Date.now() + 6000;
		for (;;) {
			const media = (await inspectMedia()).media.find((item) => item.media_kind === 'Reference');
			if (predicate(media)) return media;
			assert(Date.now() < deadline, JSON.stringify(media));
			await delay(20);
		}
	}
	const selected = (media) => media?.channels[0].definition.source.target;
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		const manager = page.locator('section[aria-label="Medias"]');
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name: 'Reference', exact: true }).click();
		await parameter('Media').waitFor();
		assert.equal(await parameter('Color').count(), 0);
		const initial = await wait((media) => media?.channels.length === 1 && selected(media) === null);
		const output = initial.channels[0].reference;
		assert.notEqual(output.channel, source.channel);
		const channels = page.locator('.reference-channels');
		await channels.getByText('Choose a target.', { exact: true }).waitFor();
		await controls.reference('Media', 'Reference target', 'Reference target');
		await wait((media) => selected(media)?.channel === source.channel);
		await channels.locator(`[data-channel="${output.channel}"]`).waitFor();
		assert.equal(
			await channels.getByRole('button', { name: 'Add child item', exact: true }).count(),
			0
		);
		await undo();
		await wait(
			(media) => selected(media) === null && media.channels[0].reference.channel === output.channel
		);
		await redo();
		await wait((media) => selected(media)?.channel === source.channel);
		await parameter('Resolution').getByRole('switch', { name: 'Toggle node enabled' }).click();
		await wait((media) => media?.channels[0].definition.size.kind === 'Fixed');
		for (const [index, value] of [17, 9].entries())
			await controls.setNumber('Resolution', String(value), index);
		await wait(
			(media) =>
				media?.channels[0].definition.size.extent?.width === 17 &&
				media.channels[0].definition.size.extent.height === 9
		);
		const { reference: peer } = await edit({
			kind: 'AddChannel',
			media: source.media,
			label: 'Second variant'
		});
		assert.equal(
			(await wait((media) => selected(media)?.channel === source.channel)).channels.length,
			1
		);
		await controls.reference('Channel', 'Second variant', 'Second variant');
		await wait((media) => selected(media)?.channel === peer.channel);
		await edit({ kind: 'RenameChannel', reference: peer, label: 'Renamed variant' });
		await parameter('Channel').getByText('Renamed variant', { exact: true }).waitFor();
		await edit({ kind: 'ReorderChannel', reference: peer, after: null });
		const bound = await wait((media) => selected(media)?.channel === peer.channel);
		assert.deepEqual(bound.channels[0].reference, output);
		assert.deepEqual(bound.channels[0].definition.size, {
			kind: 'Fixed',
			extent: { width: 17, height: 9 }
		});
		// The owned Output is identity-only. Source settings belong to the producer.
		await channels.getByRole('button', { name: 'Select output Output', exact: true }).click();
		await page
			.locator('.inspector-header .title-text')
			.filter({ hasText: /^Output$/ })
			.waitFor();
		assert.equal(await parameter('Color').count(), 0);
		assert.equal(await parameter('Media').count(), 0);
		await manager.getByRole('button', { name: bound.label, exact: true }).click();
		await parameter('Channel').waitFor();
		let last;
		for (let index = 2; index <= 64; index++) {
			last = (await edit({ kind: 'AddChannel', media: source.media, label: `Variant ${index}` }))
				.reference;
		}
		await parameter('Channel').locator('button.choice-trigger').click();
		await page.getByRole('menuitem', { name: 'Next page', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Variant 64', exact: true }).click();
		await wait((media) => selected(media)?.channel === last.channel);
		assert.equal(await channels.locator('li').count(), 1, 'Only the independent output is listed');
		await edit({ kind: 'RemoveChannel', reference: last });
		await parameter('Channel')
			.getByText(/unavailable/)
			.waitFor();
		await undo();
		await parameter('Channel').getByText('Variant 64', { exact: true }).waitFor();
		await wait((media) => selected(media)?.channel === last.channel);
		await edit({ kind: 'RemoveMedia', media: source.media });
		await channels.getByText('Target unavailable.', { exact: true }).waitFor();
		await undo();
		await channels.locator(`[data-channel="${output.channel}"]`).waitFor();
		await controls.reference('Media', 'None', 'None');
		await channels.getByText('Choose a target.', { exact: true }).waitFor();
		await undo();
		const restored = await wait((media) => selected(media)?.channel === last.channel);
		assert.deepEqual(restored.channels[0].reference, output);
		assert.deepEqual(restored.channels[0].definition.size, bound.channels[0].definition.size);
		assert(reads.some((query) => query.cursor !== null && query.limit === 64));
		assert(reads.every((query) => query.limit === 0 || query.limit === 64));
		assert.deepEqual(fullTreeRequests, []);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled Reference creation, Media/Channel and Resolution widgets, stable identity-only Output, bounded Channel paging, explicit rename/reorder/missing/recovery, exact history and no full-tree picker'
			})
		);
	} finally {
		await browser.close();
	}
}
