import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { writeFile } from 'node:fs/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixLayerGeometry({ page, edit, nativePid }) {
	let stage = 'arrange';
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	const wait = async (read, predicate) => {
		const until = Date.now() + 8000;
		for (;;) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	};
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Layer geometry'
	});
	await clearMatrixScene(edit, reference.media);
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const { items: groups } = await matrix({
		kind: 'AddGroup',
		parent: null,
		label: 'Geometry group'
	});
	const { items } = await matrix({ kind: 'AddLayers', parent: groups[0].id, count: 1 });
	const layer = items[0].id;
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [80, 70], size: { width: 120, height: 90 }, duration_ms: 777 }
	});
	await matrix({
		kind: 'ConfigureItem',
		item: items[0],
		settings: { expose_channel: false, opacity: 0.75, blend: 'Add', enabled: false }
	});
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const current = async () => (await inspect()).items.find((item) => item.id.id === layer).layer;
	const label = (await inspect()).items.find((item) => item.id.id === layer).label;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Layer geometry', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await page
		.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
		.getByTitle(`Select ${label}`, { exact: true })
		.click();
	await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	const panel = page.locator(
		'.workbench-editor[data-panel-id="composition"][data-spatial-kind="Matrix"]'
	);
	const svg = panel.getByRole('group', { name: 'Matrix Layer layout controls', exact: true });
	const ready = async () => {
		await wait(
			() => panel.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === layer
		);
		await wait(
			() => panel.locator('[data-matrix-layer-resize]').count(),
			(value) => value === 8
		);
	};
	const undo = page.getByTitle('Ctrl/Cmd+Z', { exact: true });
	const redo = page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true });
	async function history(before, after) {
		await undo.click();
		await ready();
		await wait(inspect, (value) => isDeepStrictEqual(value, before));
		await redo.click();
		await ready();
		await wait(inspect, (value) => isDeepStrictEqual(value, after));
		await undo.click();
		await ready();
		await wait(inspect, (value) => isDeepStrictEqual(value, before));
	}
	async function drag(locator, delta, escape = false) {
		await ready();
		const expected = await current();
		await wait(
			async () => [
				Number(await panel.locator(`[data-layer-id="${layer}"]`).getAttribute('x')),
				Number(await panel.locator(`[data-layer-id="${layer}"]`).getAttribute('width'))
			],
			(value) =>
				Math.abs(value[0] - expected.position[0]) < 0.01 && value[1] === expected.size.width
		);
		const box = await svg.boundingBox(),
			handle = await locator.boundingBox();
		assert(box && handle);
		const start = { x: handle.x + handle.width / 2, y: handle.y + handle.height / 2 };
		await page.mouse.move(start.x, start.y);
		await page.mouse.down();
		await page.mouse.move(
			start.x + (delta[0] * box.width) / 400,
			start.y + (delta[1] * box.height) / 300,
			{ steps: 12 }
		);
		if (escape) {
			await page.keyboard.press('Escape');
			await ready();
		}
		await page.mouse.up();
		await ready();
	}
	try {
		await ready();
		const baseline = await inspect();
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
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
			const before = await inspect(),
				origin = await current();
			await drag(panel.locator(`[data-matrix-layer-resize="${handle}"]`), [6, 4]);
			const after = await wait(inspect, (value) => !isDeepStrictEqual(value, before));
			const changed = after.items.find((item) => item.id.id === layer).layer;
			assert.deepEqual(
				after.items.find((item) => item.id.id === layer).settings,
				before.items.find((item) => item.id.id === layer).settings,
				'resizing an inactive Layer preserves opacity, blend and exposure'
			);
			for (let axis = 0; axis < 2; axis++) {
				const key = axis === 0 ? 'width' : 'height',
					opposite = 1 - direction[axis];
				assert(
					Math.abs(
						changed.position[axis] +
							opposite * changed.size[key] -
							origin.position[axis] -
							opposite * origin.size[key]
					) < 0.02
				);
				assert.equal(
					changed.size[key],
					direction[axis] === 0.5
						? origin.size[key]
						: origin.size[key] + [6, 4][axis] * (2 * direction[axis] - 1)
				);
			}
			assert.equal(changed.duration_ms, 777);
			await history(before, after);
			reports.push(handle);
		}
		stage = 'body translation';
		await drag(panel.locator(`[data-layer-id="${layer}"]`), [12, -7]);
		const translated = await wait(
			current,
			(value) => Math.abs(value.position[0] - 92) < 0.02 && Math.abs(value.position[1] - 63) < 0.02
		);
		assert.deepEqual(translated.size, { width: 120, height: 90 });
		assert.equal(translated.duration_ms, 777);
		await history(baseline, await inspect());
		stage = 'Escape retirement';
		await drag(panel.locator(`[data-layer-id="${layer}"]`), [9, 5], true);
		const stopped = await inspect();
		assert(!isDeepStrictEqual(stopped, baseline));
		await page.mouse.move(15, 15);
		assert.deepEqual(await inspect(), stopped);
		await history(baseline, stopped);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		await page.screenshot({ path: 'target/m12-layer-geometry-ui.png' });
		const result = {
			result: 'pass',
			nativePid,
			reports,
			errors,
			receipts,
			scope:
				'actual bundled inactive nested Layer body translation and all eight opposite-anchor integer-pixel resize handles; duration/composite preservation, exact grouped Undo/Redo, Escape capture retirement and no implicit launch'
		};
		await writeFile('target/m12-layer-geometry-ui.json', JSON.stringify(result, null, 2));
		console.log(JSON.stringify(result));
	} catch (error) {
		await page.screenshot({ path: 'target/m12-layer-geometry-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				stage,
				errors,
				receipts,
				body: (await page.locator('body').innerText()).slice(-3500)
			})
		);
		throw error;
	}
}
