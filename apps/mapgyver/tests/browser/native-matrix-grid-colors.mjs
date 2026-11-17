// Focused bundled native Matrix smoke; no playback/timing qualification.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token;
let stage = 'setup';
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function wait(read, accepts) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Selection colors'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const { snapshot } = await matrix({ kind: 'Inspect' });
	const layer = snapshot.items.find((item) => item.id.kind === 'Layer');
	const column = snapshot.columns[0];
	const { cell: cellId } = await matrix({
		kind: 'CreateCellMedia',
		layer: layer.id.id,
		column: column.id,
		media_kind: 'SolidColor'
	});
	const { items: groups } = await matrix({ kind: 'AddGroup', parent: null, label: 'Color Group' });
	const rowGroup = groups[0].id;
	await matrix({ kind: 'MoveItem', item: groups[0], parent: null, after: null });
	const { group: columnGroup } = await matrix({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Color Column Group'
	});
	await matrix({
		kind: 'MoveColumnItem',
		item: { kind: 'Group', id: columnGroup },
		parent: null,
		after: null
	});
	await matrix({ kind: 'AddColumns', parent: columnGroup, count: 1 });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Selection colors', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const cell = grid.locator(`[data-cell-id="${cellId}"]`);
	const row = grid.locator(`[role="rowheader"][data-row-id="${layer.id.id}"]`);
	const header = grid.locator(`[role="columnheader"][data-column-id="${column.id}"]`);
	async function choose(target, uuid) {
		stage = `select ${uuid}`;
		await wait(
			() => grid.locator('xpath=ancestor::section[1]').getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		const box = await target.boundingBox();
		assert(box);
		const label = target.locator('button[title^="Select "]');
		if (await label.count()) await label.click();
		else await target.click({ position: { x: box.width * 0.8, y: box.height * 0.75 } });
		await wait(
			() => page.locator('.inspector').getAttribute('data-inspected-uuid'),
			(value) => value === uuid
		);
		await wait(
			() => target.getAttribute('aria-selected'),
			(value) => value === 'true'
		);
		assert.equal(await grid.locator('[aria-selected="true"]').count(), 1);
	}
	await choose(cell, cellId);
	await choose(row, layer.id.id);
	await header.getByRole('button', { name: column.label, exact: true }).click();
	await wait(
		() => header.getAttribute('aria-selected'),
		(value) => value === 'true'
	);
	assert.equal(await grid.locator('[aria-selected="true"]').count(), 1);
	const targets = [
		{ node: cell, uuid: cellId },
		{ node: row, uuid: layer.id.id },
		{ node: header, uuid: column.id },
		{ node: grid.locator(`[role="rowheader"][data-row-id="${rowGroup}"]`), uuid: rowGroup },
		{ node: grid.locator(`[data-column-group-id="${columnGroup}"]`), uuid: columnGroup }
	];
	const colors = [];
	for (const { node, uuid } of targets) {
		await choose(node, uuid);
		await page.locator('.inspector-header').click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
		const color = page.locator('.gc-node-context-color-submenu .cp-input-hex');
		await color.fill('D63C50');
		await color.press('Tab');
		await page.keyboard.press('Escape');
		await page.keyboard.press('Escape');
		await wait(
			() => node.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		const colored = await node.evaluate((element) =>
			getComputedStyle(element).getPropertyValue('--matrix-item-color')
		);
		assert(colored.includes('214') && colored.includes('60') && colored.includes('80'), colored);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => node.getAttribute('class'),
			(value) => !value.includes('custom-color')
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => node.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		colors.push({ uuid, colored });
	}
	stage = 'selected colored Cell remains editable offscreen';
	await choose(cell, cellId);
	await matrix({ kind: 'AddLayers', parent: null, count: 30 });
	await wait(
		() => grid.getAttribute('aria-rowcount'),
		(value) => Number(value) >= 36
	);
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
	});
	await wait(
		() => cell.count(),
		(value) => value === 0
	);
	assert.equal(await page.locator('.inspector').getAttribute('data-inspected-uuid'), cellId);
	await grid.evaluate((element) => {
		element.scrollTop = 0;
	});
	await wait(
		() => cell.getAttribute('class'),
		(value) => value.includes('custom-color')
	);
	const project = path.join(directory, 'Matrix colors.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await post('/api/ui/project-load', { path: project });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Selection colors', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	for (const { node } of targets)
		await wait(
			() => node.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
	assert.deepEqual(errors, []);
	await page.screenshot({ path: path.join(directory, 'matrix-selection-color.png') });
	const result = {
		result: 'pass',
		pid: native.app.pid,
		scope:
			'Native bundled UI explicit Cell/Layer/Column/Group selection; Inspector colors and Undo/Redo for all five headers/item kinds; saved project reload. Nested copies have current-source Rust proof. No GPU/deep qualification.',
		colors
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failed-native-check.png') }).catch(() => {});
	throw error;
} finally {
	await native.close();
}
