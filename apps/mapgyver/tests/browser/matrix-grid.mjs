import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixGridUi({ base, edit, post, page: nativePage = null }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Workbench grid'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	let history;
	const inspect = async () => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: { kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } }
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value.snapshot;
	};
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1200 } }));
	page.setDefaultTimeout(7000);
	const errors = [];
	let stage = 'grid setup';
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Matrix square grid');
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	try {
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Workbench grid', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const panel = grid.locator('xpath=ancestor::section[1]');
		const inspector = page.locator('.inspector');
		const inspected = (id) =>
			wait(
				() => inspector.getAttribute('data-inspected-uuid'),
				(value) => value === id
			);
		const ready = () =>
			wait(
				() => panel.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		async function bodyPosition(target) {
			const box = await target.boundingBox();
			assert(box);
			return { x: box.width * 0.15, y: box.height * 0.8 };
		}
		async function contextCell(target) {
			await ready();
			await target.scrollIntoViewIfNeeded();
			await target.click({ button: 'right', position: await bodyPosition(target) });
		}
		async function press(target, id, selected = target) {
			await ready();
			const box = await target.boundingBox();
			assert(box);
			await page.mouse.move(box.x + box.width * 0.15, box.y + box.height * 0.8);
			await page.mouse.down();
			try {
				await inspected(id);
				await wait(
					() => selected.getAttribute('aria-selected'),
					(value) => value === 'true'
				);
			} finally {
				await page.mouse.up();
			}
			await ready();
		}
		async function clear(kind) {
			await ready();
			if (kind === 'background') {
				const box = await grid.boundingBox();
				assert(box);
				await page.mouse.move(box.x + box.width - 12, box.y + box.height - 16);
				await page.mouse.down();
				try {
					await inspected(reference.media);
					assert.equal(await grid.locator('[role="gridcell"][aria-selected="true"]').count(), 0);
				} finally {
					await page.mouse.up();
				}
			} else {
				await grid.focus();
				await page.keyboard.press('Escape');
				await inspected(reference.media);
			}
			await ready();
			assert.equal(await grid.locator('[aria-selected="true"]').count(), 0);
		}
		const initial = await inspect();
		await grid.locator(`.row[data-row-id="${initial.items[0].id.id}"]`).waitFor();
		const cell = (row = 'Layer 1', column = 'Column 1') =>
			grid.getByTitle(`${row} / ${column}`, { exact: true });
		await cell().waitFor();
		const bounds = await cell().boundingBox();
		const rem = await page.evaluate(() =>
			parseFloat(getComputedStyle(document.documentElement).fontSize)
		);
		assert(bounds.width >= 7.5 * rem - 1 && bounds.height >= 7.5 * rem - 1, JSON.stringify(bounds));
		assert((await grid.boundingBox()).height > 500, 'grid uses panel height');
		assert.equal(await page.locator('.editor-toolbar').count(), 0);
		stage = 'grid primary-down selection and owner clearing';
		await ready();
		const selectionSnapshot = await inspect();
		const selectionHistory = history;
		assert.equal(initial.cells.length, 0, 'empty grid addresses have no authored Cell nodes');
		assert.equal(await cell().getAttribute('data-cell-id'), null);
		await cell().hover({ position: { x: bounds.width * 0.15, y: bounds.height * 0.8 } });
		assert.equal(await cell().getAttribute('data-viewport-hovered'), 'true');
		await inspected(reference.media);
		await press(cell(), initial.items[0].id.id);
		await clear('background');
		const firstRow = grid.locator(`[role="rowheader"][data-row-id="${initial.items[0].id.id}"]`);
		await press(
			grid.getByTitle('Select Layer 1', { exact: true }),
			initial.items[0].id.id,
			firstRow
		);
		await clear('escape');
		const firstColumn = grid.locator(
			`[role="columnheader"][data-column-id="${initial.columns[0].id}"]`
		);
		await press(
			grid.getByTitle('Select Column 1', { exact: true }),
			initial.columns[0].id,
			firstColumn
		);
		await clear('escape');
		await clear('escape');
		await press(cell(), initial.items[0].id.id);
		await cell().focus();
		await page.keyboard.press('Escape');
		await inspected(reference.media);
		await ready();
		await cell().focus();
		await page.keyboard.press('Enter');
		await inspected(initial.items[0].id.id);
		await ready();
		await clear('escape');
		assert.deepEqual(await inspect(), selectionSnapshot);
		assert.deepEqual(
			history,
			selectionHistory,
			'selection and owner clearing cannot author history'
		);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		stage = 'grid owned media and complete structural history';
		await cell().hover({ position: await bodyPosition(cell()) });
		await grid
			.getByRole('button', { name: 'Add Media to Layer 1 / Column 1', exact: true })
			.click();
		await page.getByRole('menuitem', { name: 'Solid Color', exact: true }).click();
		const created = await wait(inspect, (state) => state.cells.some((cell) => cell.media));
		const originalCell = created.cells[0];
		stage = 'authored Cell primary-down and typed Inspector identity';
		await ready();
		const cellSnapshot = await inspect();
		const cellHistory = history;
		assert.equal(await cell().getAttribute('data-cell-id'), originalCell.id);
		await clear('escape');
		await press(cell(), originalCell.id);
		await clear('background');
		assert.deepEqual(await inspect(), cellSnapshot);
		assert.deepEqual(history, cellHistory);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		stage = 'grid owned media and complete structural history';
		await contextCell(cell('Layer 2', 'Column 2'));
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Add Media', { exact: true }) })
			.hover();
		await page.getByRole('menuitem', { name: 'Reference', exact: true }).click();
		await wait(inspect, (state) =>
			state.cells.some((cell) => cell.layer === initialLayerId(state, 'Layer 2') && cell.media)
		);
		for (const [oldLabel, label, field] of [
			['Layer 1', 'Stage A', 'items'],
			['Column 1', 'Scene A', 'columns']
		]) {
			const before = await inspect();
			await grid.getByTitle(`Select ${oldLabel}`, { exact: true }).dblclick();
			const input = grid.getByRole('textbox', { name: `Rename ${oldLabel}`, exact: true });
			await input.fill(label);
			await input.press('Enter');
			const after = await wait(inspect, (state) =>
				state[field].some((entry) => entry.label === label)
			);
			await undo();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await redo();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
		}
		const beforeRemoval = await inspect();
		await cell('Stage A', 'Scene A').hover({
			position: await bodyPosition(cell('Stage A', 'Scene A'))
		});
		await grid
			.getByRole('button', { name: 'Remove media from Stage A / Scene A', exact: true })
			.click();
		const removed = await wait(inspect, (state) => state.cells[0]?.media === null);
		assert.deepEqual(removed.cells[0].definition, {
			...beforeRemoval.cells[0].definition,
			source: null
		});
		await undo();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeRemoval));
		assert.equal((await inspect()).cells[0].media, originalCell.media);
		await redo();
		await wait(inspect, (state) => isDeepStrictEqual(state, removed));
		await contextCell(cell('Stage A', 'Scene A'));
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		const opacity = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Opacity', { exact: true }) })
			.locator('input.number-field');
		await opacity.waitFor();
		await grid.getByTitle('Select Layer 2', { exact: true }).click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Duplicate row', exact: true }).click();
		await wait(inspect, (state) => state.items.length === 4);
		await undo();
		await wait(inspect, (state) => state.items.length === 3);
		await grid.getByRole('button', { name: 'Add Layer or Layer Group', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Layer Group', exact: true }).click();
		const grouped = await wait(inspect, (state) =>
			state.items.some((row) => row.id.kind === 'Group')
		);
		const group = grouped.items.find((row) => row.id.kind === 'Group');
		stage = 'Group primary-down selection and rename Escape isolation';
		await ready();
		const groupSnapshot = await inspect();
		const groupHistory = history;
		const groupHeader = grid.locator(`[role="rowheader"][data-row-id="${group.id.id}"]`);
		const groupLabel = grid.getByTitle(`Select ${group.label}`, { exact: true });
		await press(groupLabel, group.id.id, groupHeader);
		await groupLabel.dblclick();
		const renameGroup = grid.getByRole('textbox', { name: `Rename ${group.label}`, exact: true });
		await renameGroup.fill('Discard this group label');
		await renameGroup.press('Escape');
		await inspected(group.id.id);
		assert.equal(await groupHeader.getAttribute('aria-selected'), 'true');
		await clear('escape');
		assert.deepEqual(await inspect(), groupSnapshot);
		assert.deepEqual(history, groupHistory);
		stage = 'grouped owned structure and exact history';
		await grid.getByTitle(`Select ${group.label}`, { exact: true }).click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Add layer in group', exact: true }).click();
		const withChild = await wait(inspect, (state) =>
			state.items.some((row) => row.parent === group.id.id)
		);
		const child = withChild.items.find((row) => row.parent === group.id.id);
		await cell(child.label, 'Scene A').hover({
			position: await bodyPosition(cell(child.label, 'Scene A'))
		});
		await grid
			.getByRole('button', { name: `Add Media to ${child.label} / Scene A`, exact: true })
			.click();
		await page.getByRole('menuitem', { name: 'Solid Color', exact: true }).click();
		await wait(inspect, (state) =>
			state.cells.some((cell) => cell.layer === child.id.id && cell.media)
		);
		for (const [label, action, rowDelta, columnDelta] of [
			[group.label, 'Duplicate row', 2, 0],
			[group.label, 'Delete row', -2, 0],
			['Layer 2', 'Delete row', -1, 0],
			['Scene A', 'Duplicate Column', 0, 1],
			['Column 3', 'Delete Column', 0, -1]
		]) {
			const before = await inspect();
			await grid.getByTitle(`Select ${label}`, { exact: true }).click({ button: 'right' });
			await page.getByRole('menuitem', { name: action, exact: true }).click();
			const after = await wait(
				inspect,
				(state) =>
					state.items.length === before.items.length + rowDelta &&
					state.columns.length === before.columns.length + columnDelta
			);
			if (action.startsWith('Duplicate'))
				assert(after.cells.length > before.cells.length, 'complete owned Cell subtree copied');
			await undo();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await redo();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
			await undo();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
		}
		await contextCell(cell('Stage A', 'Scene A'));
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		await opacity.waitFor();
		await page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Enabled', { exact: true }) })
			.locator('input[type=checkbox]')
			.waitFor();
		await grid.getByTitle(`Select ${group.label}`, { exact: true }).waitFor({ state: 'visible' });
		await page.screenshot({ path: 'target/m07-grid-ui.png' });
		stage = 'virtualized Cell remains inspected and editable';
		await inspected(originalCell.id);
		await matrix({ kind: 'AddLayers', parent: null, count: 128 });
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(count) => count === '135'
		);
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.dispatchEvent(new Event('scroll'));
		});
		await wait(
			() => cell('Stage A', 'Scene A').count(),
			(count) => count === 0
		);
		assert((await grid.getByRole('rowheader').count()) < 30, 'bounded virtual rows');
		await inspected(originalCell.id);
		await opacity.fill('0.375');
		await opacity.press('Tab');
		await wait(
			inspect,
			(state) =>
				state.cells.find((cell) => cell.id === originalCell.id)?.definition.settings.opacity ===
				0.375
		);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				selectionBeforeRelease: true,
				ownerAndIdleEscape: true,
				typedInspectorIdentity: true,
				selectionPreservesHistoryAndPlayback: true,
				scope:
					'Adaptive full-panel grid; contextual owned-media creation/removal and structural actions; inline rename and exact history; bounded offscreen Cell Inspector; no implicit launch',
				browser: (browser ?? page.context().browser()).version()
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-grid-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		throw error;
	} finally {
		await browser?.close();
	}
}

function initialLayerId(snapshot, label) {
	return snapshot.items.find((row) => row.label === label).id.id;
}
