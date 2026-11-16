import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
import { contentLock } from './content-lock.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixLayoutSelection({ base, edit, post }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Selection Matrix'
	});
	await clearMatrixScene(edit, reference.media);
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 160, height: 100 } }
	});
	let history;
	async function media(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value;
	}
	const matrix = (operation) => media({ kind: 'Matrix', media: reference.media, edit: operation });
	const { items } = await matrix({ kind: 'AddLayers', parent: null, count: 2 });
	for (const [index, item] of items.entries()) {
		await matrix({
			kind: 'ConfigureLayer',
			layer: item.id,
			settings: { position: [8 + index * 60, 8], size: { width: 48, height: 36 }, duration_ms: 0 }
		});
	}
	const { columns } = await matrix({ kind: 'AddColumns', parent: null, count: 2 });
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Selection source'
	});
	await matrix({
		kind: 'SetCells',
		assignments: columns.map((column) => ({ layer: items[0].id, column, source }))
	});
	for (const [index, column] of columns.entries()) {
		await matrix({
			kind: 'ConfigureCell',
			layer: items[0].id,
			column,
			settings: {
				position: [4 + index * 16, 4],
				size: { width: 8, height: 8 },
				fit_mode: 'Fit',
				opacity: 1,
				enabled: index === 0
			}
		});
	}
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const before = await inspect();
	const beforeHistory = history;
	const cellPage = (
		await media({
			kind: 'InspectMatrixCells',
			query: {
				media: reference.media,
				layer: items[0].id,
				selected: null,
				node: items[0].id,
				start: 0,
				count: 128
			}
		})
	).page;
	const layers = items.map((item) => {
		const row = before.items.find((entry) => entry.id.id === item.id);
		return { id: item.id, label: row.label, position: row.layer.position, level: 'Layers' };
	});
	const cells = cellPage.cells.map((entry) => ({
		id: entry.view.cell.id,
		label: entry.label,
		position: entry.view.cell.definition.settings.position,
		level: 'Cells'
	}));
	assert.equal(cells.length, 2);
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1800, height: 1100 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	let stage = 'startup';
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	const panel = (id) =>
		page.locator(`.workbench-editor[data-panel-id="${id}"][data-spatial-kind="Matrix"]`);
	const main = panel('composition');
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
	async function wait(read, predicate) {
		const until = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	async function inspector(label, position = null) {
		await tab('Inspector')
			.filter({ has: page.getByText(`Inspector: ${label}`, { exact: true }) })
			.waitFor();
		if (position) {
			const input = page
				.locator('.inspector .parameter-inspector')
				.filter({ has: page.getByText('Position', { exact: true }) })
				.locator('input.number-field')
				.first();
			await wait(
				async () => Number(await input.inputValue()),
				(value) => value === position[0]
			);
		}
	}
	async function target(current, level, layer = null) {
		await wait(
			() => current.getAttribute('data-spatial-level'),
			(value) => value === level
		);
		await wait(
			() => current.getAttribute('data-spatial-media'),
			(value) => value === reference.media
		);
		assert.equal(await current.getAttribute('data-spatial-media'), reference.media);
		if (layer) assert.equal(await current.getAttribute('data-spatial-layer'), layer);
		await wait(
			() => current.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	}
	async function visual(current, id) {
		await wait(
			() =>
				current
					.locator('rect.selected')
					.evaluateAll((nodes) =>
						nodes.map(
							(node) => node.getAttribute('data-layer-id') ?? node.getAttribute('data-layout-cell')
						)
					),
			(value) => JSON.stringify(value) === JSON.stringify(id ? [id] : [])
		);
		assert.equal(
			await current.locator('[data-matrix-layer-resize], [data-matrix-cell-resize]').count(),
			id ? 8 : 0
		);
	}
	const hit = (current, item) =>
		current.locator(
			`rect[${item.level === 'Layers' ? 'data-layer-id' : 'data-layout-cell'}="${item.id}"]`
		);
	async function press(current, item, held = async () => {}) {
		const rect = await hit(current, item).boundingBox();
		assert(rect);
		await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
		await page.mouse.down();
		try {
			await visual(current, item.id);
			await inspector(item.label, item.position);
			await held();
		} finally {
			await page.mouse.up();
		}
		await wait(inspect, () => !history.active_edit_session);
		await wait(
			() => current.getAttribute('data-spatial-selected'),
			(value) => value === item.id
		);
	}
	async function clear(current, owner) {
		const rect = await current.locator('.active-zone').boundingBox();
		assert(rect);
		await page.mouse.move(rect.x + rect.width * 0.95, rect.y + rect.height * 0.85);
		await page.mouse.down();
		try {
			await visual(current, null);
			await inspector(owner.label, owner.position);
		} finally {
			await page.mouse.up();
		}
	}
	async function escape(current, owner) {
		await wait(
			() => current.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		await current.getByRole('application').focus();
		for (let repeat = 0; repeat < 2; repeat++) {
			await page.keyboard.press('Escape');
			await visual(current, null);
			await inspector(owner.label, owner.position);
		}
	}
	async function selectMedia(label) {
		await tab('Medias').click();
		const tree = page.getByRole('region', { name: 'Medias', exact: true });
		await tree.locator('input[type="search"]').fill(label);
		await tree.getByRole('button', { name: label, exact: true }).click();
		await tree.locator('input[type="search"]').fill('');
	}
	try {
		await page.goto(base);
		await selectMedia('Selection Matrix');
		await tab('Composition').click();
		await target(main, 'Layers');
		await visual(main, null);
		await inspector('Selection Matrix');
		stage = 'Layer hover and primary-down owner selection';
		await hit(main, layers[0]).hover();
		assert.equal(await hit(main, layers[0]).getAttribute('data-viewport-hovered'), 'true');
		await visual(main, null);
		await inspector('Selection Matrix');
		await press(main, layers[0]);
		await clear(main, { label: 'Selection Matrix' });
		await press(main, layers[1]);
		await escape(main, { label: 'Selection Matrix' });
		await press(main, layers[0]);
		await main.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
		await target(main, 'Cells', layers[0].id);
		await visual(main, null);
		await inspector(layers[0].label, layers[0].position);
		stage = 'Cell primary-down and owner selection including disabled content';
		await press(main, cells[0]);
		await clear(main, layers[0]);
		await press(main, cells[1]);
		await escape(main, layers[0]);
		assert.equal(await main.getAttribute('data-spatial-layer'), layers[0].id);
		stage = 'independent locked Layer and Cell editors with visible Inspector';
		await (await contentLock(main)).click();
		const managerGroup = tab('Medias').locator(
			'xpath=ancestor::*[contains(@class,"dv-groupview")][1]'
		);
		await managerGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Composition', { exact: true }) })
			.click();
		await wait(
			() => page.locator('.gc-dv-tab[data-panel-type="composition"]').count(),
			(count) => count === 2
		);
		const peerId = await page
			.locator('.gc-dv-tab[data-panel-type="composition"]')
			.evaluateAll(
				(nodes) =>
					nodes.find((node) => node.dataset.contentPanelId !== 'composition')?.dataset
						.contentPanelId
			);
		assert(peerId);
		const peer = panel(peerId);
		await target(peer, 'Layers');
		await (await contentLock(peer)).click();
		await press(main, cells[0], async () => {
			await visual(peer, layers[0].id);
		});
		await press(peer, layers[1], async () => {
			await visual(main, null);
			assert.equal(await main.getAttribute('data-spatial-layer'), layers[0].id);
		});
		await clear(peer, { label: 'Selection Matrix' });
		await press(main, cells[0]);
		await selectMedia('Selection source');
		await page.locator(`.gc-dv-tab[data-content-panel-id="${peerId}"]`).click();
		await target(peer, 'Layers');
		await target(main, 'Cells', layers[0].id);
		await visual(main, cells[0].id);
		await inspector('Selection source');
		stage = 'selection leaves authoring, history and playback unchanged';
		assert.deepEqual(await inspect(), before);
		assert.deepEqual(history, beforeHistory);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				native: false,
				scope:
					'real Matrix Layer/Cell primary-down selection and generic Inspector, hover without controls, owner/background/idle Escape, independent locked editors, recognized Matrix focus clearing and unrelated-selection retention; unchanged authoring/history/playback',
				layerAndCellBeforeRelease: true,
				disabledCellSelectable: true,
				ownerHasNoImplicitChild: true,
				inspectorPositionMatches: true,
				independentLockedScopes: true,
				pageErrors: errors,
				rejected
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-3000)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
