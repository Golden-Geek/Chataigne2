import assert from 'node:assert/strict';
import path from 'node:path';
import { writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixCellLayout({ page, edit, post, nativePid, directory }) {
	let stage = 'arrange';
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.addInitScript(() => {
		window.__cellLayoutSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__cellLayoutSockets.push(this);
			}
		};
	});
	await page.reload();
	const wait = async (read, test) => {
		const until = Date.now() + 9000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
			await delay(30);
		}
	};
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Cell layout Matrix'
	});
	await clearMatrixScene(edit, reference.media);
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const { items } = await matrix({ kind: 'AddLayers', parent: null, count: 1 });
	const layer = items[0].id;
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [0, 0], size: { width: 32, height: 24 }, duration_ms: 777 }
	});
	await matrix({
		kind: 'ConfigureItem',
		item: items[0],
		settings: { expose_channel: false, opacity: 0.2, blend: 'Add', enabled: false }
	});
	const { columns } = await matrix({ kind: 'AddColumns', parent: null, count: 130 });
	const { reference: red } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Cell red source'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference: red,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: red.media,
		size: { kind: 'Fixed', extent: { width: 8, height: 4 } }
	});
	await matrix({
		kind: 'SetCells',
		assignments: columns.map((column) => ({ layer, column, source: red }))
	});
	for (let index = 0; index < columns.length; index++) {
		await matrix({
			kind: 'ConfigureCell',
			layer,
			column: columns[index],
			settings: {
				position: index === 0 ? [3, 2] : index === 129 ? [13, 10] : [100 + index * 10, 0],
				size: index === 0 ? { width: 8, height: 8 } : null,
				fit_mode: 'Fit',
				opacity: 0.5,
				enabled: false
			}
		});
	}
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const snapshot = await inspect(),
		row = snapshot.items.find((item) => item.id.id === layer);
	const first = snapshot.cells.find((cell) => cell.definition.column === columns[0]);
	const last = snapshot.cells.find((cell) => cell.definition.column === columns[129]);
	const panel = () => page.locator('.workbench-editor[data-panel-id="composition"]');
	const activate = () => page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	async function selected(cell) {
		await activate();
		await wait(
			() => panel().getAttribute('data-spatial-level'),
			(value) => value === 'Cells'
		);
		await wait(
			() => panel().getAttribute('data-spatial-selected'),
			(value) => value === cell
		);
		await wait(
			() => panel().getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		const preview = panel().locator(
			'.native-preview[data-preview-state="available"][data-preview-installed="true"]'
		);
		await preview.waitFor();
		assert.deepEqual(JSON.parse(await preview.getAttribute('data-preview-source')), {
			kind: 'MatrixCell',
			media: reference.media,
			cell
		});
		return preview.getAttribute('data-preview-view');
	}
	try {
		stage = 'enter inactive Cell content';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Cell layout Matrix', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await grid
			.getByTitle(`${row.label} / ${snapshot.columns[0].label}`, { exact: true })
			.click({ position: { x: 5, y: 5 } });
		await activate();
		await panel().getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
		const lease = await selected(first.id);
		assert.equal(await panel().locator('[data-layout-cell]').count(), 128);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		await (await contentLock(panel())).click();
		stage = 'independent off-page Cell';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		await grid.evaluate((element) => {
			element.scrollLeft = element.scrollWidth;
			element.dispatchEvent(new Event('scroll'));
		});
		const lastColumn = snapshot.columns.find((column) => column.id === columns[129]);
		await grid
			.getByTitle(`${row.label} / ${lastColumn.label}`, { exact: true })
			.click({ position: { x: 5, y: 5 } });
		const visibleLease = await selected(last.id);
		assert(visibleLease, 'returning from the grid installs the current Cell preview');
		assert.equal(await panel().locator('[data-layout-cell]').count(), 129);
		await panel().getByRole('button', { name: 'Frame selection', exact: true }).click();
		await panel().getByRole('button', { name: 'Home view', exact: true }).click();
		await panel().locator(`[data-layout-cell="${first.id}"]`).click();
		assert.equal(
			await selected(first.id),
			visibleLease,
			'visible Cell target changes retain their native view'
		);
		await panel().getByRole('button', { name: 'Next Cells', exact: true }).click();
		await panel().getByText('Page 2 / 2', { exact: true }).waitFor();
		await wait(
			() => panel().locator('[data-layout-cell]').count(),
			(value) => value === 3
		);
		await panel().locator(`[data-layout-cell="${last.id}"]`).click();
		assert.equal(await selected(last.id), visibleLease);
		await wait(
			() => panel().locator('[data-layout-cell]').count(),
			(value) => value === 2
		);
		await persistedPanelState(page, 'composition', {
			matrix: { media: reference.media, layer, cell: last.id, page: 1 }
		});
		const project = path.join(directory, 'cell-layout.mapgyver');
		await post('/api/ui/project-save', { path: project });
		stage = 'layout restore and reconnect';
		await page.reload();
		const restoredLease = await selected(last.id);
		await panel().getByText('Page 2 / 2', { exact: true }).waitFor();
		await contentLock(panel(), true);
		const sockets = await page.evaluate(() => window.__cellLayoutSockets.length);
		await page.evaluate(() =>
			window.__cellLayoutSockets
				.findLast(
					(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
				)
				.close(4001, 'Cell layout recovery')
		);
		await page.waitForFunction(
			(count) =>
				window.__cellLayoutSockets.length > count &&
				window.__cellLayoutSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		assert.equal(await selected(last.id), restoredLease);
		stage = 'missing Cell and Undo';
		await matrix({ kind: 'RemoveColumn', column: columns[129] });
		await panel().getByText('Cell unavailable in this Layer.', { exact: true }).waitFor();
		assert.equal(await panel().locator('.native-preview').count(), 0);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await selected(last.id);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		await page.screenshot({ path: 'target/m12-cell-layout-ui.png' });
		stage = 'New and Load';
		await post('/api/ui/project-new', {});
		await wait(
			() => panel().locator('.native-preview').count(),
			(value) => value === 0
		);
		await post('/api/ui/project-load', { path: project });
		await selected(last.id);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		const result = {
			result: 'pass',
			nativePid,
			lease,
			restoredLease,
			errors,
			receipts,
			scope:
				'inactive Cell content preview without playback; 128 visible definitions plus independent selected Cell; Frame/Home, pagination, tab lock/layout restore, stable reconnect, missing/Undo, New/Load fencing'
		};
		await writeFile('target/m12-cell-layout-ui.json', JSON.stringify(result, null, 2));
		console.log(JSON.stringify(result));
	} catch (error) {
		await page.screenshot({ path: 'target/m12-cell-layout-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				stage,
				errors,
				receipts,
				body: (await page.locator('body').innerText()).slice(-4000)
			})
		);
		throw error;
	}
}
