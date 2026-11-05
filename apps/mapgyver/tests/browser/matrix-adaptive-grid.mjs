import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

// Native M12/5 acceptance supplements the existing browser authoring contract.
export async function verifyAdaptiveMatrixGrid({ page, edit, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_GRID_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const receipts = await observeNativeRejectedRequests(page, () => 'adaptive Matrix grid');
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Adaptive grid review'
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	async function wait(read, check) {
		const deadline = Date.now() + 10000;
		while (true) {
			const value = await read();
			if (check(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	await manager.getByRole('button', { name: 'Adaptive grid review', exact: true }).click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	assert.equal(await page.locator('[data-panel-id="matrix"] .canvas-toolbar').count(), 0);
	let state = await inspect();
	for (const row of state.items.filter((row) => row.parent === null))
		await matrix({ kind: 'RemoveItem', item: row.id });
	for (const column of state.columns) await matrix({ kind: 'RemoveColumn', column: column.id });
	await wait(inspect, (s) => s.items.length === 0 && s.columns.length === 0);
	const columnAdd = grid.getByRole('button', { name: 'Add Column or Column Group', exact: true });
	const layerAdd = grid.getByRole('button', { name: 'Add Layer or Layer Group', exact: true });
	const emptyStates = [];
	async function emptyState(phase, rows, columns, image) {
		await wait(inspect, (s) => s.items.length === rows && s.columns.length === columns);
		const mounted = await wait(
			() =>
				grid.evaluate((element) => ({
					rows: element.querySelectorAll('.row[data-row-id]').length,
					columns: element.querySelectorAll('.heading[data-column-id]').length,
					cells: element.querySelectorAll('[role="gridcell"]').length
				})),
			(value) => value.rows === rows && value.columns === columns && value.cells === 0
		);
		await columnAdd.click({ trial: true });
		await layerAdd.click({ trial: true });
		assert((await columnAdd.isVisible()) && (await layerAdd.isVisible()));
		await page.screenshot({ path: path.join(directory, image) });
		emptyStates.push({ phase, mounted, column_creation: true, layer_creation: true, image });
	}
	await emptyState('both-empty', 0, 0, 'empty.png');
	async function create(button, label) {
		await button.click();
		await page.getByRole('menuitem', { name: label, exact: true }).click();
	}
	await create(columnAdd, 'Column');
	state = await wait(inspect, (s) => s.columns.length === 1);
	await emptyState('zero-rows', 0, 1, 'zero-rows.png');
	await create(layerAdd, 'Layer');
	await wait(inspect, (s) => s.items.length === 1);
	await matrix({ kind: 'RemoveColumn', column: state.columns[0].id });
	await emptyState('zero-columns', 1, 0, 'zero-columns.png');
	await create(columnAdd, 'Column');
	state = await wait(inspect, (s) => s.items.length === 1 && s.columns.length === 1);
	assert.equal(state.cells.length, 0, 'creation does not materialize an empty address');
	await matrix({ kind: 'RemoveItem', item: state.items[0].id });
	await matrix({ kind: 'RemoveColumn', column: state.columns[0].id });
	await emptyState('both-empty-again', 0, 0, 'empty-again.png');
	await create(columnAdd, 'Column Group');
	state = await wait(inspect, (s) => s.column_groups.length === 1);
	const group = state.column_groups[0].id;
	await create(columnAdd, 'Column');
	await wait(inspect, (s) => s.columns.length === 1);
	await create(columnAdd, 'Column');
	state = await wait(inspect, (s) => s.columns.length === 2);
	assert(
		state.columns.every((column) => column.parent === group),
		'backend places siblings inside selected hierarchy'
	);
	await create(layerAdd, 'Layer Group');
	state = await wait(inspect, (s) => s.items.length === 1);
	const rowGroup = state.items[0].id.id;
	await create(layerAdd, 'Layer');
	await wait(inspect, (s) => s.items.length === 2);
	await create(layerAdd, 'Layer');
	state = await wait(inspect, (s) => s.items.length === 3);
	assert(
		state.items.filter((row) => row.id.kind === 'Layer').every((row) => row.parent === rowGroup)
	);
	const row = state.items.find((row) => row.id.kind === 'Layer');
	const column = state.columns[0];
	const cell = grid.locator(`[data-row-id="${row.id.id}"].row [data-column-id="${column.id}"]`);
	const add = cell.getByRole('button', {
		name: `Add Media to ${row.label} / ${column.label}`,
		exact: true
	});
	await page.mouse.move(0, 0);
	await grid.focus();
	assert.equal(
		await cell.locator('.add').evaluate((element) => getComputedStyle(element).opacity),
		'0'
	);
	assert.equal(await cell.locator('.empty-icon').count(), 1);
	await add.focus();
	assert.equal(
		await cell.locator('.add').evaluate((element) => getComputedStyle(element).opacity),
		'1'
	);
	await add.press('Enter');
	await page.getByRole('menuitem', { name: 'Solid Color', exact: true }).click();
	const populated = await wait(inspect, (s) => s.cells.length === 1 && s.cells[0].media);
	const remove = cell.getByRole('button', {
		name: `Remove media from ${row.label} / ${column.label}`,
		exact: true
	});
	await remove.waitFor();
	const removeIcon = await remove.locator('img').getAttribute('src');
	await grid.focus();
	await page.mouse.move(0, 0);
	assert.equal(
		await cell.locator('.remove').evaluate((element) => getComputedStyle(element).opacity),
		'0'
	);
	await remove.focus();
	assert.equal(
		await cell.locator('.remove').evaluate((element) => getComputedStyle(element).opacity),
		'1'
	);
	await remove.press('Enter');
	await wait(inspect, (s) => s.cells.length === 0 || s.cells[0].media === null);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(inspect, (s) => s.cells[0]?.media === populated.cells[0].media);
	await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Removal icon review' });
	await manager
		.getByRole('button', { name: 'Removal icon review', exact: true })
		.click({ button: 'right' });
	const genericDelete = page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Delete', { exact: true }) });
	await genericDelete.waitFor();
	assert.equal(
		await genericDelete.locator('img').getAttribute('src'),
		removeIcon,
		'public hook applies the same X to generic removal menus'
	);
	await page.keyboard.press('Escape');
	await manager.getByRole('button', { name: 'Adaptive grid review', exact: true }).click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const handle = await grid.elementHandle();
	const beforeResize = await inspect();
	const preview = page.locator('[data-panel-id="matrix"] .cell-previews .native-preview');
	const lease = await wait(
		() =>
			preview.evaluate((element) =>
				element.getAttribute('data-preview-installed') === 'true'
					? element.getAttribute('data-preview-view')
					: null
			),
		(value) => Boolean(value)
	);
	assert(lease, 'the populated Cell has an actual native preview lease');
	const geometry = [];
	for (const [width, height] of [
		[720.25, 640.25],
		[1000.5, 800.5],
		[400.5, 350.5]
	]) {
		const sample = await grid.evaluate(
			async (element, [width, height]) => {
				const original = element.style.cssText;
				element.style.width = `${width}px`;
				element.style.height = `${height}px`;
				const sizes = [],
					leases = [];
				try {
					for (let frame = 0; frame < 45; frame++) {
						await new Promise(requestAnimationFrame);
						leases.push(
							element
								.closest('.viewport-container')
								.querySelector('.native-preview')
								?.getAttribute('data-preview-view') ?? null
						);
						if (frame >= 15)
							sizes.push([
								element.clientWidth,
								element.clientHeight,
								getComputedStyle(element).getPropertyValue('--row'),
								getComputedStyle(element).getPropertyValue('--column')
							]);
					}
				} finally {
					element.style.cssText = original;
				}
				return { sizes, leases };
			},
			[width, height]
		);
		assert.equal(
			new Set(sample.sizes.map(JSON.stringify)).size,
			1,
			'resize settles without geometry oscillation'
		);
		assert.deepEqual(
			[...new Set(sample.leases)],
			[lease],
			'every resize frame retains the same native preview lease'
		);
		if (width > 700) {
			assert(
				parseFloat(sample.sizes.at(-1)[2]) > 7.5 && parseFloat(sample.sizes.at(-1)[3]) > 7.5,
				'small tables expand their rows and Columns'
			);
		}
		geometry.push(sample.sizes.at(-1));
	}
	assert(
		await handle.evaluate((element) => element.isConnected),
		'resizing preserves the actual grid component'
	);
	assert.deepEqual(await inspect(), beforeResize, 'resizing does not author data');
	assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
	await page.screenshot({ path: path.join(directory, 'adaptive.png') });
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	const result = {
		result: 'pass',
		nativePid,
		group,
		rowGroup,
		empty_states: emptyStates,
		geometry,
		preview_lease: lease,
		preview_resize_frames: 135,
		errors,
		rejected: receipts.rejected,
		transport: receipts.observed,
		limits:
			'Actual native zero axes, trailing creation, selected hierarchy, keyboard reveal/create/remove, generic X hook, history and settled resize. Full group/Inspector and 1000x1000 cases run separately in the same acceptance flow.'
	};
	await writeFile(path.join(directory, 'adaptive.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
}
