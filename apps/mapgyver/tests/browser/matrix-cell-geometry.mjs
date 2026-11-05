import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { writeFile } from 'node:fs/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixCellGeometry({ page, edit, nativePid }) {
	let stage = 'arrange';
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	const wait = async (read, test) => {
		const until = Date.now() + 9000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	};
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Cell geometry'
	});
	await clearMatrixScene(edit, reference.media);
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const { items } = await matrix({ kind: 'AddLayers', parent: null, count: 1 });
	const layer = items[0].id;
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [0, 0], size: { width: 400, height: 300 }, duration_ms: 777 }
	});
	const { columns } = await matrix({ kind: 'AddColumns', parent: null, count: 1 });
	const { cell, media: source } = await matrix({
		kind: 'CreateCellMedia',
		layer,
		column: columns[0],
		media_kind: 'SolidColor'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source,
		size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
	});
	const baselineSettings = {
		position: [80, 70],
		size: null,
		fit_mode: 'Fill',
		opacity: 0.5,
		enabled: false
	};
	await matrix({ kind: 'ConfigureCell', layer, column: columns[0], settings: baselineSettings });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const settings = (snapshot) =>
		snapshot.cells.find((value) => value.id === cell).definition.settings;
	const current = async () => settings(await inspect());
	const snapshot = await inspect(),
		row = snapshot.items.find((value) => value.id.id === layer);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Cell geometry', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await page
		.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
		.getByTitle(`${row.label} / ${snapshot.columns[0].label}`, { exact: true })
		.click({ position: { x: 5, y: 5 } });
	await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	const panel = page.locator('.workbench-editor[data-panel-id="composition"]');
	await panel.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
	const svg = panel.getByRole('group', { name: 'Matrix Cell layout controls', exact: true });
	const rect = panel.locator(`[data-layout-cell="${cell}"]`);
	const ready = async () => {
		await wait(
			() => panel.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === cell
		);
	};
	async function history(before, after) {
		for (const [title, expected] of [
			['Ctrl/Cmd+Z', before],
			['Ctrl/Cmd+Shift+Z or Ctrl+Y', after],
			['Ctrl/Cmd+Z', before]
		]) {
			await page.getByTitle(title, { exact: true }).click();
			await ready();
			await wait(inspect, (value) => isDeepStrictEqual(value, expected));
		}
	}
	async function drag(locator, delta, escape = false) {
		await ready();
		const expected = await current();
		if ((await rect.evaluate((element) => element.tagName.toLowerCase())) === 'rect')
			await wait(
				async () =>
					Promise.all(
						['x', 'y', 'width', 'height'].map(async (key) => Number(await rect.getAttribute(key)))
					),
				(value) =>
					Math.abs(value[0] - expected.position[0]) < 0.02 &&
					Math.abs(value[1] - expected.position[1]) < 0.02 &&
					value[2] === (expected.size?.width ?? 120) &&
					value[3] === (expected.size?.height ?? 90)
			);
		const box = await svg.boundingBox(),
			handle = await locator.boundingBox();
		assert(box && handle);
		const x = handle.x + handle.width / 2,
			y = handle.y + handle.height / 2;
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x + (delta[0] * box.width) / 400, y + (delta[1] * box.height) / 300, {
			steps: 12
		});
		if (escape) await page.keyboard.press('Escape');
		await page.mouse.up();
		await ready();
	}
	try {
		await ready();
		const baseline = await inspect();
		stage = 'natural translation';
		await drag(rect, [12, -7]);
		const moved = await wait(
			inspect,
			(value) =>
				Math.abs(settings(value).position[0] - 92) < 0.02 &&
				Math.abs(settings(value).position[1] - 63) < 0.02
		);
		assert.equal(settings(moved).size, null);
		await history(baseline, moved);
		const reports = [];
		for (const [handle, direction] of Object.entries({
			TopLeft: [0, 0],
			Top: [0.5, 0],
			TopRight: [1, 0],
			Right: [1, 0.5],
			BottomRight: [1, 1],
			Bottom: [0.5, 1],
			BottomLeft: [0, 1],
			Left: [0, 0.5]
		})) {
			stage = `resize ${handle}`;
			await drag(panel.locator(`[data-matrix-cell-resize="${handle}"]`), [6, 4]);
			const after = await wait(inspect, (value) => !isDeepStrictEqual(value, baseline)),
				changed = settings(after);
			assert.deepEqual(
				{ ...changed, position: baselineSettings.position, size: null },
				baselineSettings
			);
			for (let axis = 0; axis < 2; axis++) {
				const key = axis === 0 ? 'width' : 'height',
					old = [120, 90][axis],
					opposite = 1 - direction[axis];
				assert.equal(
					changed.size[key],
					direction[axis] === 0.5 ? old : old + [6, 4][axis] * (2 * direction[axis] - 1)
				);
				assert(
					Math.abs(
						changed.position[axis] +
							opposite * changed.size[key] -
							baselineSettings.position[axis] -
							opposite * old
					) < 0.02
				);
			}
			await history(baseline, after);
			reports.push(handle);
		}
		stage = 'Escape retirement';
		await drag(rect, [9, 5], true);
		const stopped = await inspect();
		assert(!isDeepStrictEqual(stopped, baseline));
		await page.mouse.move(15, 15);
		assert.deepEqual(await inspect(), stopped);
		await history(baseline, stopped);
		stage = 'unavailable source translation';
		await matrix({ kind: 'SetCells', assignments: [{ layer, column: columns[0], source: null }] });
		await ready();
		await wait(
			() => panel.locator('[data-matrix-cell-resize]').count(),
			(value) => value === 0
		);
		await panel.getByRole('button', { name: 'Frame selection', exact: true }).click();
		const unavailable = await inspect();
		await drag(rect, [4, 3]);
		const movedUnavailable = await wait(inspect, (value) => !isDeepStrictEqual(value, unavailable));
		assert.equal(settings(movedUnavailable).size, null);
		await history(unavailable, movedUnavailable);
		const playback = (await matrix({ kind: 'InspectPlayback' })).snapshot;
		assert.equal(playback.desired_column, null);
		assert.deepEqual(playback.layers, []);
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		const result = {
			result: 'pass',
			nativePid,
			reports,
			errors,
			receipts,
			scope:
				'inactive natural Cell translation, eight opposite-anchor resizes, exact grouped history, Escape retirement, unavailable-source translation without synthetic dimensions and no playback'
		};
		await writeFile('target/m12-cell-geometry-ui.json', JSON.stringify(result, null, 2));
		console.log(JSON.stringify(result));
	} catch (error) {
		await page.screenshot({ path: 'target/m12-cell-geometry-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				stage,
				errors,
				receipts,
				body: (await page.locator('body').innerText()).slice(-3000)
			})
		);
		throw error;
	}
}
