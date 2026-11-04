import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixOutputExposure({ page, edit, post }) {
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { rejected } = await observeNativeRejectedRequests(page, () => 'Matrix output exposure');
	await post('/api/ui/project-new', {});
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Exposed Matrix'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const { items: groups } = await matrix({
		kind: 'AddGroup',
		parent: null,
		label: 'Exposed Group'
	});
	const original = await inspect();
	const channels = async (media = reference.media) =>
		(await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 32 } })).page;
	async function wait(read, test) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	async function selectMedia(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await manager.getByRole('button', { name: label, exact: true }).click();
	}
	const section = page.locator('.channels-section');
	const rows = section.locator(
		':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
	);
	await selectMedia('Exposed Matrix');
	assert.equal((await channels()).total, 1, 'backend exposes only Final by default');
	await wait(
		() => section.getAttribute('data-channel-count'),
		(count) => count === '1'
	);
	assert.equal(await rows.count(), 1, 'generic Inspector follows public output admission');
	const { media: alias } = await edit({
		kind: 'CreateReference',
		target: reference.media,
		label: 'Exposed Reference'
	});
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	for (const item of [
		original.items[0],
		original.items.find((item) => item.id.id === groups[0].id)
	]) {
		await selectMedia('Exposed Matrix');
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		await page.getByRole('grid').getByTitle(`Select ${item.label}`, { exact: true }).click();
		const expose = parameter('Expose Channel').locator('input[type=checkbox]');
		await expose.waitFor();
		assert.equal(await expose.isChecked(), false);
		await expose.check();
		await wait(
			inspect,
			(value) => value.items.find((row) => row.id.id === item.id.id).settings.expose_channel
		);
		await parameter('Blend Mode').locator('select').selectOption('Screen');
		await wait(
			inspect,
			(value) => value.items.find((row) => row.id.id === item.id.id).settings.blend === 'Screen'
		);
		await selectMedia('Exposed Matrix');
		await wait(
			() => section.getAttribute('data-channel-count'),
			(count) => count === '2'
		);
		assert.equal(await rows.count(), 2);
		assert(
			(await channels()).channels.some(
				(channel) => channel.reference.channel === item.channel.channel
			)
		);
		assert.equal((await channels(alias)).total, 2);
		await selectMedia('Exposed Reference');
		await wait(
			() => page.locator('.reference-channels').getAttribute('data-channel-count'),
			(count) => count === '2'
		);
		assert.equal(
			await page.getByRole('list', { name: 'Mirrored channels' }).getByRole('listitem').count(),
			2
		);
		await selectMedia('Exposed Matrix');
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		await page.getByRole('grid').getByTitle(`Select ${item.label}`, { exact: true }).click();
		await expose.uncheck();
		await wait(channels, (value) => value.total === 1);
		assert.equal((await channels(alias)).total, 1);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(channels, (value) => value.total === 2);
		assert.equal(
			(await channels(alias)).channels.some(
				(channel) => channel.reference.channel === item.channel.channel
			),
			true
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(channels, (value) => value.total === 1);
		assert.equal(
			(await inspect()).items.find((row) => row.id.id === item.id.id).channel.channel,
			item.channel.channel
		);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Actual generic Inspector default/toggled Layer and Group exposure, Blend Mode, backend and Reference discovery, stable output identities and Undo/Redo'
		})
	);
}
