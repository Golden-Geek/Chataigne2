// The same generic File workflow can run in a native WebView or bundled headless browser.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyPictureFiles({ page, edit, post, nativePid = null, directory }) {
	directory = path.resolve(directory ?? process.env.MAPGYVER_CHANNEL_FILE_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { rejected, observed } = await observeNativeRejectedRequests(
		page,
		() => 'Picture-owned File'
	);
	const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const binding = async (reference) =>
		(await edit({ kind: 'InspectPictureFile', reference })).binding;
	const source = async (reference) =>
		(await edit({ kind: 'InspectChannel', reference })).snapshot.definition.source;
	async function wait(read, accept) {
		const deadline = Date.now() + 8000;
		for (;;) {
			const value = await read();
			if (accept(value)) return value;
			assert(Date.now() < deadline, JSON.stringify({ value, rejected }));
			await delay(25);
		}
	}
	async function topology() {
		const { nodes } = await snapshot();
		const parents = new Map(
			nodes.flatMap((node) => (node.children ?? []).map((id) => [id, node.uuid]))
		);
		return nodes
			.map((node) => [node.uuid, node.node_type, parents.get(node.node_id) ?? null])
			.sort();
	}
	const selected = () =>
		page
			.locator('[role="treeitem"][aria-selected="true"]')
			.evaluateAll((rows) => rows.map((row) => row.getAttribute('data-tree-node-id')).sort());
	async function select(reference) {
		const catalog = await inspect();
		const media = catalog.media.find((media) => media.id === reference.media);
		const index = media.channels.findIndex(
			(channel) => channel.reference.channel === reference.channel
		);
		await page.getByRole('tab').first().click();
		const root = (await snapshot()).nodes.find((node) => node.uuid === reference.media);
		const manager = page.locator('section[data-panel-id="media-manager"]');
		await manager
			.locator(`[data-tree-node-id="${root.node_id}"]`)
			.getByRole('button', { name: media.label, exact: true })
			.click();
		const row = page
			.locator('.channels-section')
			.locator(
				':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
			)
			.nth(index);
		const title = row.locator(':scope > .node-header > .node-title');
		if ((await title.getAttribute('aria-checked')) === 'false') await title.press('Enter');
		await row.locator('.parameter-inspector input.path-input').waitFor();
		return row;
	}
	const beforeCatalog = await inspect();
	const { reference: first } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Picture',
		label: 'Owned Picture files'
	});
	const { reference: peer } = await edit({ kind: 'AddChannel', media: first.media });
	const firstAddress = (await binding(first)).parameter;
	const peerAddress = (await binding(peer)).parameter;
	assert.notEqual(firstAddress.uuid, peerAddress.uuid);
	const shape = await topology();
	let row = await select(first);
	const selection = await selected();
	async function enter(reference, value, key = 'Tab') {
		row = await select(reference);
		const beforeSelection = await selected();
		const input = row.locator('.parameter-inspector input.path-input');
		await input.fill(value);
		await input.press(key);
		await wait(
			() => source(reference),
			(source) => (source.file?.path ?? '') === value
		);
		assert.deepEqual(await topology(), shape);
		assert.deepEqual((await inspect()).assets, beforeCatalog.assets);
		assert.deepEqual(
			await selected(),
			beforeSelection,
			'File editing must not select another object'
		);
		assert.equal(
			(await binding(reference)).parameter.uuid,
			reference.channel === first.channel ? firstAddress.uuid : peerAddress.uuid
		);
	}
	assert.equal(await row.locator('.parameter-inspector').count(), 1);
	assert.equal(
		await row.getByRole('button', { name: 'Browse', exact: true }).isEnabled(),
		nativePid !== null,
		'Browse follows the host native-dialog capability'
	);
	for (const label of [
		'Choose existing Picture asset',
		'Inspect Picture asset',
		'Import and assign'
	])
		assert.equal(await row.getByRole('button', { name: label, exact: true }).count(), 0);
	await enter(first, 'images/source.png');
	assert.equal((await source(peer)).file, null);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => source(first),
		(source) => source.file === null
	);
	assert.deepEqual(await topology(), shape);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(
		() => source(first),
		(source) => source.file?.path === 'images/source.png'
	);
	await enter(peer, 'images/source.png');
	await enter(first, path.join(directory, 'absolute.png'));
	assert.equal((await source(first)).file.kind, 'absolute');
	assert.equal((await source(peer)).file.path, 'images/source.png');
	await enter(first, 'images/relinked.png', 'Enter');
	await enter(first, '');
	assert.equal((await source(peer)).file.path, 'images/source.png');
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => source(first),
		(source) => source.file?.path === 'images/relinked.png'
	);
	row = await select(first);
	const draft = row.locator('.parameter-inspector input.path-input');
	await draft.fill('../cancelled.png');
	await draft.press('Escape');
	assert.equal(await draft.inputValue(), 'images/relinked.png');
	assert(await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).isEnabled());
	const beforeReload = await snapshot();
	await row.getByRole('button', { name: 'Reload Picture', exact: true }).click();
	await wait(
		() => row.getByRole('button', { name: 'Reload Picture', exact: true }).isEnabled(),
		Boolean
	);
	const afterReload = await snapshot();
	assert.deepEqual(afterReload.history, beforeReload.history);
	assert.deepEqual(await topology(), shape);
	assert.deepEqual(await selected(), selection);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(
		() => source(first),
		(source) => source.file === null
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => source(first),
		(source) => source.file?.path === 'images/relinked.png'
	);
	const document = await snapshot();
	const media = document.nodes.find((node) => node.uuid === first.media);
	const manager = document.nodes.find((node) => node.node_type === 'mapgyver.media.manager');
	const duplicate = await post('/api/ui/intent', {
		kind: 'duplicateNode',
		source: media.node_id,
		new_parent: manager.node_id,
		new_prev_sibling: media.node_id,
		initial_params: []
	});
	assert(duplicate.success, JSON.stringify(duplicate));
	const copy = (await inspect()).media.find(
		(item) => item.id !== first.media && !beforeCatalog.media.some((old) => old.id === item.id)
	);
	assert(copy && copy.channels.length === 2);
	assert.deepEqual(
		copy.channels.map((channel) => channel.definition),
		(await inspect()).media
			.find((media) => media.id === first.media)
			.channels.map((channel) => channel.definition)
	);
	const copied = copy.channels[0].reference;
	assert.notEqual((await binding(copied)).parameter.uuid, firstAddress.uuid);
	row = await select(copied);
	await row.locator('.parameter-inspector input.path-input').fill('images/copy.png');
	await row.locator('.parameter-inspector input.path-input').press('Tab');
	await wait(
		() => source(copied),
		(source) => source.file?.path === 'images/copy.png'
	);
	assert.equal((await source(first)).file.path, 'images/relinked.png');
	assert.equal((await source(peer)).file.path, 'images/source.png');
	assert.deepEqual((await inspect()).assets, beforeCatalog.assets);
	const savedCatalog = await inspect();
	const project = path.join(directory, 'owned-files.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual(await inspect(), savedCatalog);
	assert.equal((await binding(first)).parameter.uuid, firstAddress.uuid);
	assert.equal((await binding(peer)).parameter.uuid, peerAddress.uuid);
	row = await select(copied);
	assert.equal(
		await row.locator('.parameter-inspector input.path-input').inputValue(),
		'images/copy.png'
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	assert(observed.requests > 0 && observed.receipts > 0);
	await page.screenshot({ path: path.join(directory, 'picture-files.png') });
	const receipt = {
		nativePid,
		ownedFiles: true,
		topologyAndSelection: true,
		independentCopies: true,
		typedGenericWidgets: true,
		historyRedoAndReload: true,
		noImplicitAssets: true,
		absoluteAndRelativePaths: true,
		browseAvailable: nativePid !== null,
		observed,
		errors,
		rejected,
		limits:
			'Authored File behavior only; decoded/native pixels and cadence require separate qualification.'
	};
	await writeFile(path.join(directory, 'picture-files.json'), JSON.stringify(receipt, null, 2));
	console.log(JSON.stringify(receipt));
}
