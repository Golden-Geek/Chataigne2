import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyNativeCompactChannels({ page, edit, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_COMPACT_CHANNELS_DIRECTORY);
	await mkdir(directory);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Compact channels'
	});
	const { reference: peer } = await edit({ kind: 'AddChannel', media: reference.media });
	const read = async () =>
		(
			await edit({
				kind: 'InspectChannels',
				query: { media: reference.media, start: 0, count: 128 }
			})
		).page;
	async function wait(read, predicate) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	await page.getByRole('tab').first().click();
	await page.getByRole('button', { name: 'Compact channels', exact: true }).click();
	const section = page.locator('.channels-section');
	const rows = section.locator(
		':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
	);
	await wait(
		() => rows.count(),
		(count) => count === 2
	);
	for (const label of ['Kind', 'Source kind', 'Sampling'])
		assert.equal(await page.getByText(label, { exact: true }).count(), 0);
	for (const row of await rows.all()) {
		assert.equal(
			await row.locator(':scope > .node-header > .node-title').getAttribute('aria-checked'),
			'true'
		);
		assert.equal(await row.locator('.parameter-inspector').count(), 1);
		assert.equal(await row.locator('.parameter-inspector.density-compact').count(), 1);
		assert.equal(await row.getByText('Parameters', { exact: true }).count(), 0);
	}
	const first = rows.first();
	const color = first
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Color', { exact: true }) });
	await color.locator('.cp-preview').click();
	await color.locator('.cp-input-hex').fill('ff0000');
	await color.locator('.cp-input-hex').press('Tab');
	const red = (page) =>
		page.channels.find((channel) => channel.reference.channel === reference.channel);
	await wait(read, (page) => JSON.stringify(red(page).definition.source.rgba) === '[1,0,0,1]');
	assert.deepEqual(
		(await read()).channels.find((channel) => channel.reference.channel === peer.channel).definition
			.source.rgba,
		[1, 1, 1, 1]
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (page) => JSON.stringify(red(page).definition.source.rgba) === '[1,1,1,1]');
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(read, (page) => JSON.stringify(red(page).definition.source.rgba) === '[1,0,0,1]');
	await first.locator(':scope > .node-header .title-text').dblclick();
	await section.locator('input.node-title-input').fill('Red channel');
	await section.locator('input.node-title-input').press('Enter');
	await wait(read, (page) => red(page).label === 'Red channel');
	const header = first.locator(':scope > .node-header > .node-title');
	assert.equal(
		await header.getAttribute('aria-checked'),
		'true',
		'Enter finishes renaming without collapsing the row'
	);
	await header.click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^Duplicate(?:\s|$)/ }).click();
	const copied = await wait(read, (page) => page.channels.length === 3);
	const duplicate = copied.channels.find(
		(channel) => ![reference.channel, peer.channel].includes(channel.reference.channel)
	);
	assert.deepEqual(duplicate.definition.source.rgba, [1, 0, 0, 1]);
	const copyRow = rows.filter({ has: page.getByText(duplicate.label, { exact: true }) });
	await copyRow.locator(':scope > .node-header > .node-title').click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^Delete(?:\s|$)/ }).click();
	await wait(read, (page) => page.channels.length === 2);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (page) =>
		page.channels.some((channel) => channel.reference.channel === duplicate.reference.channel)
	);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(read, (page) => page.channels.length === 2);
	for (let i = 0; i < 35; i++)
		await edit({ kind: 'AddChannel', media: reference.media, label: `Paged ${i + 1}` });
	await wait(
		() => rows.count(),
		(count) => count === 32
	);
	await wait(
		() => section.locator('.parameter-inspector').count(),
		(count) => count === 32
	);
	await section.getByRole('button', { name: 'Next channels', exact: true }).click();
	await wait(
		() => rows.count(),
		(count) => count === 5
	);
	await wait(
		() => section.locator('.parameter-inspector').count(),
		(count) => count === 5
	);
	await section.getByRole('button', { name: 'Previous channels', exact: true }).click();
	await wait(
		() => rows.count(),
		(count) => count === 32
	);
	await page.screenshot({ path: path.join(directory, 'compact-channels.png') });
	assert.deepEqual(errors, []);
	const receipt = {
		nativePid,
		expandedByDefault: true,
		oneColorPerChannel: true,
		nestedParameterHeaders: 0,
		hiddenKindAndSampling: true,
		editUndoRedo: true,
		renameDuplicateDeleteHistory: true,
		isolatedPeer: true,
		totalChannels: 37,
		mountedPageCounts: [32, 5, 32],
		pageErrors: errors
	};
	await writeFile(path.join(directory, 'compact-channels.json'), JSON.stringify(receipt, null, 2));
	console.log(JSON.stringify(receipt));
}
