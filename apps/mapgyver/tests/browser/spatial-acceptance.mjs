import assert from 'node:assert/strict';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { stat, writeFile } from 'node:fs/promises';
import { contentLock } from './content-lock.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifySpatialAcceptance({ page, edit, post, directory, nativePid }) {
	let stage = 'arrange sparse project';
	let resizeFocus;
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	const wait = async (read, test) => {
		const until = Date.now() + 15000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value).slice(0, 1500)}`);
			await delay(30);
		}
	};
	try {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.waitFor();
		const { reference: source } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Spatial sparse source'
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: source.media,
			size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
		});
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Spatial sparse Matrix'
		});
		const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
		await matrix({ kind: 'AddLayers', parent: null, count: 997 });
		await matrix({ kind: 'AddColumns', parent: null, count: 997 });
		const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
		const topology = await inspect(),
			layer = topology.items[999].id.id,
			column = topology.columns[999].id;
		assert.equal(topology.items.length, 1000);
		assert.equal(topology.columns.length, 1000);
		await matrix({
			kind: 'ConfigureLayer',
			layer,
			settings: { position: [0, 0], size: { width: 400, height: 300 }, duration_ms: 777 }
		});
		await matrix({
			kind: 'SetCells',
			assignments: [
				{ layer: topology.items[0].id.id, column: topology.columns[0].id, source },
				{ layer, column: topology.columns[0].id, source },
				{ layer, column, source }
			]
		});
		const initial = await inspect(),
			cell = initial.cells.find(
				(value) => value.layer === layer && value.definition.column === column
			).id;
		await matrix({
			kind: 'ConfigureCell',
			layer,
			column,
			settings: { position: [80, 70], size: null, fit_mode: 'Fill', opacity: 0.5, enabled: true }
		});
		const authored = await inspect();
		assert.equal(authored.cells.length, 3);
		const settings = (snapshot) =>
			snapshot.cells.find((value) => value.id === cell).definition.settings;
		const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Spatial sparse Matrix', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(value) => value === '1002'
		);
		await wait(
			() => grid.getAttribute('aria-colcount'),
			(value) => value === '1002'
		);
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.scrollLeft = element.scrollWidth;
			element.dispatchEvent(new Event('scroll'));
		});
		await grid
			.getByTitle(`${topology.items[999].label} / ${topology.columns[999].label}`, { exact: true })
			.click({ position: { x: 5, y: 5 } });
		await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
		const panel = page.locator('.workbench-editor[data-panel-id="composition"]');
		const ready = async () =>
			wait(
				() => panel.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		await ready();
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === layer
		);
		assert.equal(
			await panel.locator('[data-layer-id]').count(),
			129,
			'128 visible Layers plus the independent far selection'
		);
		assert.deepEqual(await inspect(), authored, 'sparse selection does not materialize addresses');
		const activation = process.env.MAPGYVER_SPATIAL_ACTIVATION;
		if (activation) {
			assert(Number.isSafeInteger(nativePid) && nativePid > 0);
			await writeFile(`${activation}.json`, JSON.stringify({ nativePid }));
			const deadline = Date.now() + 90000;
			while (!(await stat(activation).catch(() => null))) {
				assert(Date.now() < deadline, 'Owned sparse fixture activation timed out');
				await delay(50);
			}
		}
		const undo = async (expected) => {
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(inspect, (value) => isDeepStrictEqual(value, expected));
			await ready();
		};
		stage = 'far Layer keyboard geometry';
		await panel.getByRole('application', { name: 'Matrix Layer layout view', exact: true }).focus();
		await page.keyboard.press('Shift+ArrowRight');
		await wait(
			inspect,
			(value) => value.items.find((value) => value.id.id === layer).layer.position[0] === 10
		);
		await ready();
		await undo(authored);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.scrollLeft = element.scrollWidth;
			element.dispatchEvent(new Event('scroll'));
		});
		await grid
			.getByTitle(`${topology.items[999].label} / ${topology.columns[999].label}`, { exact: true })
			.click({ position: { x: 5, y: 5 } });
		await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
		await ready();
		await panel.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
		await ready();
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === cell
		);
		assert.equal(await panel.locator('[data-layout-cell]').count(), 2);
		const view = panel.getByRole('application', { name: 'Matrix Cell content view', exact: true });
		const rect = panel.locator(`[data-layout-cell="${cell}"]`);
		await panel
			.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
			.waitFor();
		assert.equal(
			(await playback()).layers.length,
			0,
			'selection and Layer edits never launch playback'
		);
		stage = 'active Cell geometry preserves playback';
		await matrix({ kind: 'LaunchCell', layer, column, mode: 'Cut' });
		const playing = await playback();
		assert(playing.layers.some((request) => request.layer === layer && request.column === column));
		await ready();
		await view.focus();
		await page.keyboard.press('ArrowRight');
		await wait(inspect, (value) => settings(value).position[0] === 81);
		await ready();
		assert.equal(settings(await inspect()).size, null);
		assert.deepEqual(
			await playback(),
			playing,
			'authoring an active Cell does not relaunch or replace playback'
		);
		await undo(authored);
		stage = 'natural source resize then independent placement';
		await edit({
			kind: 'ConfigureMediaResolution',
			media: source.media,
			size: { kind: 'Fixed', extent: { width: 240, height: 90 } }
		});
		await ready();
		await wait(
			() => rect.getAttribute('width'),
			(value) => value === '240'
		);
		const beforeResize = await inspect();
		const handle = panel.locator('[data-matrix-cell-resize="Right"]');
		await wait(() => handle.isEnabled(), Boolean);
		await handle.focus();
		resizeFocus = await handle.evaluate((element) => ({
			focused: document.activeElement === element,
			nativeDisabled: element.disabled,
			ariaDisabled: element.getAttribute('aria-disabled')
		}));
		assert(resizeFocus.focused && !resizeFocus.nativeDisabled, JSON.stringify(resizeFocus));
		for (let step = 0; step < 3; step++) await page.keyboard.down('ArrowRight');
		await page.keyboard.up('ArrowRight');
		const resized = await wait(inspect, (value) => settings(value).size?.width === 243);
		await ready();
		assert.deepEqual(settings(resized), {
			...settings(beforeResize),
			size: { width: 243, height: 90 }
		});
		assert.deepEqual(
			(await edit({ kind: 'InspectChannel', reference: source })).snapshot.definition.size,
			{ kind: 'Fixed', extent: { width: 240, height: 90 } }
		);
		assert.deepEqual(await playback(), playing);
		await undo(beforeResize);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (value) => isDeepStrictEqual(value, resized));
		await ready();
		stage = 'copy and project reload';
		const { columns } = await matrix({ kind: 'DuplicateColumn', column });
		const copied = await inspect();
		assert.equal(
			copied.cells.length,
			4,
			'one million sparse addresses remain four authored Cells after copying one address'
		);
		const copy = copied.cells.find(
			(value) => value.layer === layer && value.definition.column === columns[0]
		);
		assert.deepEqual(copy.definition.settings, settings(resized));
		assert.deepEqual(await playback(), playing);
		await (await contentLock(panel)).click();
		const savedPath = path.join(directory, 'spatial-sparse.mapgyver');
		await post('/api/ui/project-save', { path: savedPath });
		const saved = await inspect();
		await page.reload();
		await ready();
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === cell
		);
		await contentLock(panel, true);
		assert.deepEqual(await inspect(), saved);
		assert.equal(await panel.locator('[data-layout-cell]').count(), 3);
		stage = 'New while held and same-identity Load fence';
		await view.focus();
		await page.keyboard.down('ArrowRight');
		await wait(inspect, (value) => settings(value).position[0] === 81);
		await post('/api/ui/project-new', {});
		await wait(
			() => panel.locator('.native-preview').count(),
			(value) => value === 0
		);
		await post('/api/ui/project-load', { path: savedPath });
		await ready();
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === cell
		);
		await view.focus();
		await page.keyboard.down('ArrowRight');
		await page.keyboard.up('ArrowRight');
		assert.deepEqual(
			await inspect(),
			saved,
			'a stale held repeat cannot edit reloaded identical UUIDs'
		);
		await page.keyboard.press('ArrowRight');
		await wait(inspect, (value) => settings(value).position[0] === 81);
		await ready();
		await undo(saved);
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		const result = {
			result: 'pass',
			nativePid,
			stage,
			layers: 1000,
			columns: 1000,
			initialCells: 3,
			cellsAfterCopy: 4,
			visibleLayerBound: 129,
			resizeFocus,
			errors,
			receipts,
			scope:
				'large sparse spatial Layer/Cell selection and geometry; active Cell authoring retains exact playback; natural source resize and independent authored Size; exact history/copy/layout/project reload; locked target and New-while-held/same-UUID Load fence'
		};
		await writeFile(
			process.env.MAPGYVER_SPATIAL_RESULT ?? 'target/m12-spatial-acceptance-ui.json',
			JSON.stringify(result, null, 2)
		);
		console.log(JSON.stringify(result));
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				receipts,
				focus: await page
					.evaluate(() => ({
						document: document.hasFocus(),
						active: document.activeElement?.tagName,
						resize: document.activeElement?.getAttribute('data-matrix-cell-resize'),
						right: [...document.querySelectorAll('[data-matrix-cell-resize="Right"]')].map(
							(element) => ({
								disabled: element.disabled,
								focused: element === document.activeElement
							})
						)
					}))
					.catch(() => null),
				body: (await page.locator('body').innerText()).slice(-3000)
			})
		);
		throw error;
	}
}
