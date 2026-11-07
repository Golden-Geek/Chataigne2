// Bundled manager UI with ordinary authoritative receipts; no desktop input.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	token,
	stage = 'startup';
const checks = [],
	errors = [],
	receipts = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(6000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
async function intent(edit) {
	const ack = await post('/api/ui/intent', edit);
	assert(ack.success, JSON.stringify(ack));
	return ack;
}
async function operation(payload) {
	return (
		await intent({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload
		})
	).result.value;
}
async function wait(read, predicate) {
	const deadline = Date.now() + 6000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, stage + ': ' + JSON.stringify(value));
		await delay(20);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framereceived', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				const ack = frame.update?.acknowledgement;
				if (ack?.created_node) receipts.push({ stage, ...ack });
			} catch {}
		})
	);
	await page.goto(host.base, { waitUntil: 'networkidle' });
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp('^' + name + '$') });
	await tab('Medias').click();
	const media = page.locator('section[aria-label="Medias"]');
	const screens = page.locator('section[aria-label="Screens"]');
	const menu = (name) => page.getByRole('menuitem', { name, exact: true });
	const row = (panel, id) => panel.locator(`[data-tree-node-id="${id}"]`);
	async function created(panel, before, action) {
		const count = receipts.length;
		await action();
		await wait(
			async () => receipts,
			(items) => items.length === count + 1
		);
		const ack = receipts.at(-1),
			identity = ack.created_node;
		assert(ack.success && ack.history.undo_len === before.history.undo_len + 1);
		const document = await snapshot();
		const node = document.nodes.find((node) => node.uuid === identity.uuid);
		assert.equal(node?.node_id, identity.node_id);
		await row(panel, identity.node_id).waitFor();
		await wait(
			() => row(panel, identity.node_id).getAttribute('aria-selected'),
			(selected) => selected === 'true'
		);
		await wait(
			() => page.locator('.inspector .title-text').allTextContents(),
			(labels) => labels.some((label) => label.trim() === node.meta.label)
		);
		await wait(
			() => page.getByRole('menuitem').count(),
			(count) => count === 0
		);
		checks.push({ stage, identity, selected: true, inspector: true, single_history: true });
		return node;
	}
	const root = (await snapshot()).nodes.find((node) => node.node_type === 'mapgyver.media.manager');
	assert(root);
	const createdMedia = [];
	for (const label of [
		'Shared Texture',
		'Solid Color',
		'Picture',
		'Reference',
		'Composition',
		'Matrix',
		'Folder'
	]) {
		stage = 'root Add ' + label;
		await media.locator('.manager-list-content').click({ position: { x: 3, y: 2 } });
		const node = await created(media, await snapshot(), async () => {
			await media
				.locator('.manager-list-header')
				.getByRole('button', { name: 'Add child item' })
				.click();
			await menu(label).click();
		});
		createdMedia.push(node);
	}
	for (const node of createdMedia.filter(
		(node) => !['Matrix', 'Folder'].includes(node.meta.label)
	)) {
		assert.equal(await row(media, node.node_id).getAttribute('aria-expanded'), null);
		assert.equal(await row(media, node.node_id).locator('.has-arrow').count(), 0);
	}
	const folder = createdMedia.find((node) => node.meta.label === 'Folder');
	assert.equal(await row(media, folder.node_id).getAttribute('aria-expanded'), null);
	stage = 'nested Folder row Add';
	const nested = await created(media, await snapshot(), async () => {
		await row(media, folder.node_id).getByRole('button', { name: 'Add child item' }).click();
		await menu('Folder').click();
	});
	stage = 'nested context Add Shared Texture';
	const leaf = await created(media, await snapshot(), async () => {
		await row(media, nested.node_id)
			.getByRole('button', { name: nested.meta.label, exact: true })
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: /^Add\b/ }).hover();
		await menu('Shared Texture').click();
	});
	assert.equal(await row(media, leaf.node_id).getAttribute('aria-expanded'), null);
	await media.getByPlaceholder('Search media…').fill(leaf.meta.label);
	await row(media, leaf.node_id).waitFor();
	assert.equal(await row(media, leaf.node_id).locator('.has-arrow').count(), 0);
	await media.getByPlaceholder('Search media…').fill('');
	stage = 'root context Add Solid Color';
	await created(media, await snapshot(), async () => {
		await media
			.locator('.manager-list-content')
			.click({ button: 'right', position: { x: 3, y: 2 } });
		await page.getByRole('menuitem', { name: /^Add\b/ }).hover();
		await menu('Solid Color').click();
	});
	stage = 'single-item Screens Add';
	await tab('Screens').click();
	const screen = await created(screens, await snapshot(), () =>
		screens.locator('.manager-list-header').getByRole('button', { name: 'Add child item' }).click()
	);
	stage = 'Screen context Add Surface';
	await created(screens, await snapshot(), async () => {
		await row(screens, screen.node_id)
			.getByRole('button', { name: screen.meta.label, exact: true })
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: /^Add\b/ }).hover();
		await menu('Surface').click();
	});
	stage = 'real Composition children';
	await tab('Medias').click();
	const composition = createdMedia.find((node) => node.meta.label === 'Composition');
	const catalog = (await operation({ kind: 'InspectManager' })).snapshot;
	const reference = catalog.media.find((entry) => entry.id === composition.uuid).channels[0]
		.reference;
	await operation({
		kind: 'Composition',
		reference,
		edit: {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Nested authored color',
			position: { x: 0, y: 0 }
		}
	});
	const child = (await snapshot()).nodes.find(
		(node) => node.meta.label === 'Nested authored color'
	);
	await row(media, child.node_id).waitFor();
	assert.equal(await row(media, composition.node_id).getAttribute('aria-expanded'), 'true');
	assert.equal(await row(media, child.node_id).getAttribute('aria-expanded'), null);
	checks.push({ stage, actual_child: child.uuid });
	stage = 'real Matrix hierarchy through hidden Final channel';
	const matrix = createdMedia.find((node) => node.meta.label === 'Matrix');
	const matrixState = (
		await operation({ kind: 'Matrix', media: matrix.uuid, edit: { kind: 'Inspect' } })
	).snapshot;
	const layer = matrixState.items.find((item) => item.id.kind === 'Layer');
	const matrixEdit = (edit) => operation({ kind: 'Matrix', media: matrix.uuid, edit });
	const group = (await matrixEdit({ kind: 'AddGroup', parent: null, label: 'Manager Layer Group' }))
		.items[0];
	await matrixEdit({ kind: 'MoveItem', item: layer.id, parent: group.id, after: null });
	const columnGroup = (
		await matrixEdit({ kind: 'AddColumnGroup', parent: null, label: 'Manager Column Group' })
	).group;
	await matrixEdit({
		kind: 'MoveColumnItem',
		item: { kind: 'Column', id: matrixState.columns[0].id },
		parent: columnGroup,
		after: null
	});
	const cell = await operation({
		kind: 'Matrix',
		media: matrix.uuid,
		edit: {
			kind: 'CreateCellMedia',
			layer: layer.id.id,
			column: matrixState.columns[0].id,
			media_kind: 'SolidColor'
		}
	});
	const authored = await snapshot();
	for (const uuid of [
		group.id,
		columnGroup,
		layer.id.id,
		matrixState.columns[0].id,
		cell.cell,
		cell.media
	]) {
		const actual = authored.nodes.find((node) => node.uuid === uuid);
		await row(media, actual.node_id).waitFor();
	}
	assert.equal(await row(media, matrix.node_id).getAttribute('aria-expanded'), 'true');
	assert.equal(
		await media
			.locator('.outliner-item-label')
			.filter({ hasText: /^Parameters$/ })
			.count(),
		0
	);
	assert.equal(
		await media
			.locator('.outliner-item-label')
			.filter({ hasText: /^Final$/ })
			.count(),
		0
	);
	checks.push({ stage, real_layer_column_cell_and_owned_media: true });
	stage = 'exact history and reload';
	const before = (await operation({ kind: 'InspectManager' })).snapshot;
	const history = (await snapshot()).history.undo_len;
	await intent({ kind: 'undo' });
	assert(!(await snapshot()).nodes.some((node) => node.uuid === cell.cell));
	await intent({ kind: 'redo' });
	assert.deepEqual((await operation({ kind: 'InspectManager' })).snapshot, before);
	assert.equal((await snapshot()).history.undo_len, history);
	const project = path.join(directory, 'manager.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual((await operation({ kind: 'InspectManager' })).snapshot, before);
	const restoredChild = (await snapshot()).nodes.find((node) => node.uuid === child.uuid);
	await row(media, restoredChild.node_id).waitFor();
	checks.push({ stage, exact: true });
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{ result: 'pass', pid: host.app.pid, checks, receipts, errors, desktop_input: false },
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, receipts, errors }, null, 2)
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
	await writeFile(path.join(directory, 'host.log'), host.log());
}
