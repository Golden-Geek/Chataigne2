// Focused bundled native Matrix smoke; no playback/timing qualification.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { verifyMatrixAppearance } from './matrix-appearance.mjs';
import { verifyMatrixColumnEnabled } from './matrix-column-enabled.mjs';
import { verifyCompactGroups, verifyExistingCompactGroup } from './matrix-compact-groups.mjs';
import { verifyGroupRails } from './matrix-group-rails.mjs';

const directory = path.resolve(process.argv[2]);
const contextMenus = process.argv.includes('--context-menus');
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token;
let stage = 'setup';
const historyTrace = [];
if (process.argv.includes('--trace-history')) {
	const client = await page.context().newCDPSession(page);
	client.on('Network.webSocketFrameSent', ({ response }) => {
		try {
			const frame = JSON.parse(response.payloadData);
			if (frame.kind !== 'intent' && frame.kind !== 'intentBatch') return;
			historyTrace.push({
				stage,
				request: frame.request_id,
				intent: frame.intent ?? frame.intents
			});
			if (historyTrace.length > 100) historyTrace.shift();
		} catch {}
	});
	client.on('Network.webSocketFrameReceived', ({ response }) => {
		try {
			const frame = JSON.parse(response.payloadData);
			if (frame.kind !== 'control' || !['applied', 'rejected'].includes(frame.update.phase)) return;
			const update = frame.update;
			historyTrace.push({
				stage,
				request: update.request_id,
				phase: update.phase,
				ack: (update.acknowledgements ?? [update.acknowledgement]).filter(Boolean).map((ack) => ({
					success: ack.success,
					error: ack.error_message,
					history: ack.history,
					projection: ack.result?.value?.page?.snapshot
						? {
								groups: ack.result.value.page.snapshot.group_total,
								cells: ack.result.value.page.snapshot.cells.map((entry) => entry.cell.id)
							}
						: undefined
				}))
			});
			if (historyTrace.length > 100) historyTrace.shift();
		} catch {}
	});
	await client.send('Network.enable');
}
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
	async function context(target) {
		await wait(
			() => grid.locator('xpath=ancestor::section[1]').getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		const box = await target.boundingBox();
		assert(box);
		const cellTarget = (await target.getAttribute('role')) === 'gridcell';
		await target.click({
			button: 'right',
			position: {
				x: cellTarget ? box.width * 0.8 : Math.min(50, box.width * 0.3),
				y: cellTarget ? box.height * 0.75 : Math.min(16, box.height * 0.3)
			}
		});
	}
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
		const selectionBefore = await grid.locator('[aria-selected="true"]').count();
		if (contextMenus) await context(node);
		else await page.locator('.inspector-header').click({ button: 'right' });
		assert.equal(await grid.locator('[aria-selected="true"]').count(), selectionBefore);
		assert(await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).isDisabled());
		await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
		const color = page.locator('.gc-node-context-color-submenu .cp-input-hex');
		await color.fill('4080B0');
		await color.press('Tab');
		await wait(
			() =>
				node.evaluate((element) =>
					getComputedStyle(element).getPropertyValue('--matrix-item-color')
				),
			(value) => value.includes('64') && value.includes('128') && value.includes('176')
		);
		await color.fill('D63C50');
		await color.press('Tab');
		await page.keyboard.press('Escape');
		await page.keyboard.press('Escape');
		await wait(
			() => node.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		const colored = await wait(
			() =>
				node.evaluate((element) =>
					getComputedStyle(element).getPropertyValue('--matrix-item-color')
				),
			(value) => value.includes('214') && value.includes('60') && value.includes('80')
		);
		assert(colored.includes('214') && colored.includes('60') && colored.includes('80'), colored);
		const inspectorColor = await page
			.locator('.inspector-header')
			.evaluate((element) => getComputedStyle(element).getPropertyValue('--node-accent-color'));
		assert.equal(inspectorColor.replaceAll(' ', ''), colored.replaceAll(' ', ''));
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
		if (contextMenus) {
			await context(node);
			await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).click();
			await wait(
				() => node.getAttribute('class'),
				(value) => !value.includes('custom-color')
			);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(
				() => node.getAttribute('class'),
				(value) => value.includes('custom-color')
			);
		}
		colors.push({ uuid, colored });
	}
	if (contextMenus) {
		stage = 'context target preserves multiselection';
		await choose(row, layer.id.id);
		await header
			.getByRole('button', { name: column.label, exact: true })
			.click({ modifiers: ['Control'] });
		const selected = () =>
			grid
				.locator('[aria-selected="true"]')
				.evaluateAll((nodes) =>
					nodes
						.map(
							(node) =>
								node.getAttribute('data-row-id') ??
								node.getAttribute('data-column-id') ??
								node.getAttribute('data-cell-id')
						)
						.sort()
				);
		const before = await selected();
		assert.equal(before.length, 2);
		await context(cell);
		await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).click();
		await wait(
			() => cell.getAttribute('class'),
			(value) => !value.includes('custom-color')
		);
		assert.deepEqual(await selected(), before);
		assert((await row.getAttribute('class')).includes('custom-color'));
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => cell.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		stage = 'empty Cell first color is one Undo';
		const emptyColumn = snapshot.columns[1].id;
		const empty = grid.locator(
			`[data-cell-layer="${layer.id.id}"][data-cell-column="${emptyColumn}"] [role="gridcell"]`
		);
		assert.equal(await empty.getAttribute('data-cell-id'), null);
		await context(empty);
		assert(await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).isDisabled());
		await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
		const input = page.locator('.gc-node-context-color-submenu .cp-input-hex');
		await input.fill('80C040');
		await input.press('Tab');
		await page.keyboard.press('Escape');
		await wait(
			() => empty.getAttribute('class'),
			(value) => value.includes('custom-color')
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => empty.getAttribute('data-cell-id'),
			(value) => value === null
		);
		assert.deepEqual(await selected(), before);
	}
	stage = 'compact Group headers and Inspector';
	const compact = process.argv.includes('--compact-groups')
		? await verifyCompactGroups({ page, grid, matrix, group: rowGroup, wait })
		: null;
	stage = 'nested Group rails';
	const groupRails = process.argv.includes('--group-rails')
		? await verifyGroupRails({ page, grid, matrix, context, wait, compact, directory })
		: null;
	stage = 'selected colored Cell remains editable offscreen';
	await choose(cell, cellId);
	await matrix({ kind: 'AddLayers', parent: null, count: 30 });
	await wait(
		() => grid.getAttribute('aria-rowcount'),
		(value) => Number(value) >= 36
	);
	if (contextMenus) {
		await context(cell);
		await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
	}
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
	});
	await wait(
		() => cell.count(),
		(value) => value === 0
	);
	assert.equal(await page.locator('.inspector').getAttribute('data-inspected-uuid'), cellId);
	if (contextMenus) {
		const input = page.locator('.gc-node-context-color-submenu .cp-input-hex');
		await input.fill('D63C50');
		await input.press('Tab');
		await page.keyboard.press('Escape');
	}
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
	const appearance = process.argv.includes('--appearance')
		? await verifyMatrixAppearance({
				page,
				grid,
				cell,
				layer,
				column,
				matrix,
				post,
				reference,
				directory,
				wait
			})
		: null;
	const columnEnabled = process.argv.includes('--column-enabled')
		? await verifyMatrixColumnEnabled({ page, grid, column, matrix, post, directory, wait })
		: null;
	if (compact) {
		const restored = (await matrix({ kind: 'Inspect' })).snapshot;
		const group = restored.items.find((item) => item.id.id === compact.group);
		assert.equal(group.settings.opacity, compact.expected.opacity);
		assert.equal(group.settings.blend, compact.expected.blend);
		compact.reload = true;
	}
	assert.deepEqual(errors, []);
	await page.screenshot({ path: path.join(directory, 'matrix-selection-color.png') });
	const existingArgument = process.argv.indexOf('--existing-project');
	const existing =
		existingArgument >= 0
			? await verifyExistingCompactGroup({
					page,
					post,
					wait,
					project: path.resolve(process.argv[existingArgument + 1])
				})
			: null;
	const result = {
		result: 'pass',
		pid: native.app.pid,
		scope: contextMenus
			? 'Bundled Matrix Set Color/Reset Color for all five kinds; two accepted writes in one Undo, Redo, Inspector parity, preserved multiselection, atomic empty Cell color, offscreen palette and reload. Nested copies have current-source Rust proof. No GPU/deep qualification.'
			: 'Native bundled UI explicit Cell/Layer/Column/Group selection; Inspector colors and Undo/Redo for all five headers/item kinds; saved project reload. Nested copies have current-source Rust proof. No GPU/deep qualification.',
		contextMenus,
		colors,
		appearance,
		columnEnabled,
		compact,
		groupRails,
		existing
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} catch (error) {
	if (historyTrace.length)
		await writeFile(
			path.join(directory, 'history-trace.json'),
			JSON.stringify(historyTrace, null, 2)
		);
	await page.screenshot({ path: path.join(directory, 'failed-native-check.png') }).catch(() => {});
	throw error;
} finally {
	await native.close();
}
