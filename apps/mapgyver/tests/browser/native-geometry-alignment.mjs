// Focused bundled-app pointer/typed-geometry acceptance; no deep GPU/timing claim.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { verifyAlignmentPixels } from './geometry-alignment-pixels.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const native = await openNativeEditor(),
	{ page } = native;
page.setDefaultTimeout(8000);
let token,
	stage = 'setup';
const errors = [],
	checks = [],
	pixels = [];
page.on('pageerror', (error) => errors.push(String(error)));
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
async function edit(payload, namespace = 'mapgyver.media') {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function wait(read, test) {
	const until = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (test(value)) return value;
		assert(Date.now() < until, `${stage}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}
const near = (actual, expected, epsilon = 0.05) =>
	actual.length === expected.length && actual.every((v, i) => Math.abs(v - expected[i]) < epsilon);
const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
const manager = () => page.getByRole('region', { name: 'Medias', exact: true });
async function selectMedia(label) {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager().getByRole('button', { name: label, exact: true }).click();
	await tab('composition').click();
}
async function drag(view, svg, hit, from, to, extent, aligned = true, held) {
	const panel = view.locator('xpath=ancestor::section[contains(@class,"workbench-editor")][1]');
	await wait(
		() =>
			panel.evaluate(
				(node) =>
					!['data-composition-busy', 'data-composition-loading', 'aria-busy'].some(
						(key) => node.getAttribute(key) === 'true'
					)
			),
		(v) => v
	);
	await hit.waitFor();
	const rect = await svg.boundingBox();
	assert(rect);
	const screen = ([x, y]) => ({
		x: rect.x + (x / extent[0]) * rect.width,
		y: rect.y + (y / extent[1]) * rect.height
	});
	const start = screen(from),
		end = screen(to);
	await page.mouse.move(start.x, start.y);
	await page.mouse.down();
	await page.mouse.move(end.x, end.y, { steps: 8 });
	if (aligned) {
		await view.locator('[data-alignment-guides]').waitFor();
		assert((await view.locator('[data-alignment-guides] line').count()) > 0);
		assert.equal(
			await view
				.locator('[data-alignment-guides]')
				.evaluate((n) => getComputedStyle(n).pointerEvents),
			'none'
		);
		await page.screenshot({ path: path.join(directory, `${stage}-held.png`) });
	} else assert.equal(await view.locator('[data-alignment-guides]').count(), 0);
	await held?.();
	await page.mouse.up();
	await wait(
		() => svg.getAttribute('data-geometry-presentation'),
		(v) => v === 'committed'
	);
	await wait(
		() => view.locator('[data-alignment-guides]').count(),
		(v) => v === 0
	);
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Alignment source'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 80, height: 60 } }
	});
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Alignment Composition'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const composition = (operation) => edit({ kind: 'Composition', reference, edit: operation });
	await composition({ kind: 'AddInstances', sources: [source, source] });
	const initial = (await composition({ kind: 'Inspect' })).snapshot;
	for (const [i, instance] of initial.instances.entries())
		await composition({
			kind: 'ConfigureInstance',
			instance: instance.reference.instance,
			definition: {
				...instance.definition,
				transform: {
					position: i ? [260, 170] : [80, 70],
					scale: [1, 1],
					rotation_degrees: 0,
					pivot: [0, 0]
				}
			}
		});
	const primary = initial.instances[0].reference.instance;
	const current = async () =>
		(await composition({ kind: 'Inspect' })).snapshot.instances.find(
			(item) => item.reference.instance === primary
		).definition.transform;
	await selectMedia('Alignment Composition');
	const view = page.getByRole('application', { name: 'Composition view', exact: true });
	const svg = view.getByRole('group', { name: 'Composition transform controls', exact: true });
	const hit = svg.locator(`[data-instance-drop-target="${primary}"]`);
	await hit.waitFor();
	await view.getByRole('button', { name: 'Align geometry', exact: true }).click();
	stage = 'composition-move';
	await drag(view, svg, hit, [120, 100], [223, 143], [400, 300], true, async () => {
		await wait(current, (v) => near(v.position, [180, 110]));
		if (process.argv.includes('--pixels')) {
			await view.locator('.native-preview[data-preview-installed="true"]').waitFor();
			pixels.push(await verifyAlignmentPixels(native, view, 'x'));
			pixels.push(await verifyAlignmentPixels(native, view, 'y'));
			await writeFile(path.join(directory, 'held-pixels.json'), JSON.stringify(pixels, null, 2));
		}
	});
	await wait(current, (v) => near(v.position, [180, 110]));
	const moved = await current();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(current, (v) => near(v.position, [80, 70]));
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(current, (v) => near(v.position, moved.position));
	checks.push('Composition: two-axis move, held guides, one-step Undo/Redo');
	stage = 'composition-off';
	await view.getByRole('button', { name: 'Align geometry', exact: true }).click();
	await drag(view, svg, hit, [220, 140], [225, 145], [400, 300], false);
	await wait(current, (v) => near(v.position, [185, 115]));
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(current, (v) => near(v.position, [180, 110]));
	await view.getByRole('button', { name: 'Align geometry', exact: true }).click();
	stage = 'composition-resize';
	const handle = view.locator('[data-instance-resize="BottomRight"]');
	await drag(view, svg, handle, [260, 170], [298, 198], [400, 300]);
	await wait(current, (v) => near(v.scale, [1.5, 1.5]));
	checks.push('Composition: toggle-off preserves raw samples; resize aligns both dimensions');

	stage = 'composition-zoom-pan';
	const canvasBeforeZoom = await svg.boundingBox();
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await wait(
		() => svg.boundingBox(),
		(box) => box.width > canvasBeforeZoom.width * 1.18
	);
	const zoomed = await svg.boundingBox();
	const center = { x: zoomed.x + zoomed.width / 2, y: zoomed.y + zoomed.height / 2 };
	await page.mouse.move(center.x, center.y);
	await page.mouse.down({ button: 'middle' });
	await page.mouse.move(center.x - 25, center.y + 20);
	await page.mouse.up({ button: 'middle' });
	await wait(
		() => svg.boundingBox(),
		(box) => box.x < zoomed.x - 20 && box.y > zoomed.y + 15
	);
	await drag(view, svg, hit, [235, 147], [273, 172], [400, 300], true, async () => {
		assert.equal(await view.locator('[data-alignment-guides] line').count(), 1);
		await wait(current, (v) => near(v.position, [220, 135], 0.25));
		await page.keyboard.press('Escape');
		await wait(
			() => view.locator('[data-alignment-guides]').count(),
			(v) => v === 0
		);
	});
	await wait(current, (v) => near(v.position, [220, 135], 0.25));
	checks.push(
		'Composition: one-axis alignment under zoom and pan; Escape clears guides while held'
	);

	stage = 'composition-collapsed-source';
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	await wait(
		() => svg.boundingBox(),
		(box) => Math.abs(box.width - canvasBeforeZoom.width) < 1
	);
	await composition({
		kind: 'ConfigureInstance',
		instance: initial.instances[1].reference.instance,
		definition: {
			...initial.instances[1].definition,
			transform: { position: [260, 170], scale: [0, 1], rotation_degrees: 0, pivot: [0, 0] }
		}
	});
	const { instances: unavailable } = await composition({
		kind: 'CreateMedia',
		media_kind: 'Reference',
		label: 'Unavailable alignment peer',
		position: { x: 80, y: 170 }
	});
	await composition({
		kind: 'ConfigureInstance',
		instance: primary,
		definition: {
			...initial.instances[0].definition,
			transform: { position: [80, 70], scale: [1, 1], rotation_degrees: 0, pivot: [0, 0] }
		}
	});
	const missing = `[data-instance-drop-target="${unavailable[0].instance}"]`;
	await view.locator(missing).waitFor();
	assert.equal(await svg.locator(missing).count(), 0);
	await wait(
		() => hit.getAttribute('points'),
		(value) => value?.startsWith('80,70 ')
	);
	const collapsed = svg.locator(
		`[data-instance-drop-target="${initial.instances[1].reference.instance}"]`
	);
	await wait(
		() => collapsed.getAttribute('points'),
		(value) => value?.startsWith('260,170 260,170 ')
	);
	await drag(view, svg, hit, [120, 100], [223, 143], [400, 300], false);
	await wait(current, (v) => near(v.position, [183, 113], 0.15));
	checks.push(
		'Composition: unavailable and zero-area peers do not attract either axis with alignment enabled'
	);

	stage = 'matrix-setup';
	const { reference: matrixRef } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Alignment Matrix'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: matrixRef.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: matrixRef.media, edit: operation });
	const topology = (await matrix({ kind: 'Inspect' })).snapshot;
	const layers = topology.items.filter((item) => item.id.kind === 'Layer');
	for (const [i, layer] of layers.entries())
		await matrix({
			kind: 'ConfigureLayer',
			layer: layer.id.id,
			settings: {
				position: i === 0 ? [40, 40] : i === 1 ? [240, 160] : [600, 500],
				size: { width: 80, height: 60 },
				duration_ms: 1000
			}
		});
	await matrix({ kind: 'AddLayers', parent: null, count: 128 });
	await matrix({ kind: 'AddColumns', count: 128 });
	const layer = layers[0].id.id;
	const layerState = async () =>
		(await matrix({ kind: 'Inspect' })).snapshot.items.find((item) => item.id.id === layer).layer;
	await selectMedia('Alignment Matrix');
	await tab('matrix').click();
	await page
		.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
		.getByTitle(`Select ${layers[0].label}`, { exact: true })
		.click();
	await tab('composition').click();
	const layerView = page.getByRole('application', {
		name: 'Matrix Layer layout view',
		exact: true
	});
	const layerSvg = layerView.getByRole('group', {
		name: 'Matrix Layer layout controls',
		exact: true
	});
	await layerView.getByText('Page 1 / 2', { exact: true }).waitFor();
	assert.equal(await layerSvg.locator('[data-layer-id]').count(), 128);
	await layerView.getByRole('button', { name: 'Align geometry', exact: true }).click();
	stage = 'matrix-layer-move';
	await drag(
		layerView,
		layerSvg,
		layerSvg.locator(`[data-layer-id="${layer}"]`),
		[80, 70],
		[203, 133],
		[400, 300]
	);
	await wait(layerState, (v) => near(v.position, [160, 100]));
	stage = 'matrix-layer-resize';
	await drag(
		layerView,
		layerSvg,
		layerView.locator('[data-matrix-layer-resize="BottomRight"]'),
		[240, 160],
		[317, 218],
		[400, 300]
	);
	await wait(layerState, (v) => v.size.width === 160 && v.size.height === 120);
	checks.push(
		'Matrix Layer: typed move and resize match snapped controls in a sparse 131 by 131 Matrix with 128 mounted Layer candidates'
	);
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [0, 0], size: { width: 400, height: 300 }, duration_ms: 1000 }
	});
	await matrix({
		kind: 'SetCells',
		assignments: topology.columns
			.slice(0, 2)
			.map((column) => ({ layer, column: column.id, source }))
	});
	for (const [i, column] of topology.columns.slice(0, 2).entries())
		await matrix({
			kind: 'ConfigureCell',
			layer,
			column: column.id,
			settings: {
				position: i ? [240, 160] : [40, 40],
				size: { width: 80, height: 60 },
				fit_mode: 'Fill',
				opacity: 1,
				enabled: true
			}
		});
	const cell = (await matrix({ kind: 'Inspect' })).snapshot.cells.find(
		(entry) => entry.layer === layer && entry.definition.column === topology.columns[0].id
	);
	assert(cell);
	await layerView.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
	const cellView = page.getByRole('application', { name: 'Matrix Cell content view', exact: true });
	const cellSvg = cellView.getByRole('group', { name: 'Matrix Cell layout controls', exact: true });
	await cellView.getByRole('button', { name: 'Align geometry', exact: true }).click();
	stage = 'matrix-cell-move';
	await drag(
		cellView,
		cellSvg,
		cellSvg.locator(`[data-layout-cell="${cell.id}"]`),
		[80, 70],
		[203, 133],
		[400, 300]
	);
	const cellState = async () =>
		(await matrix({ kind: 'Inspect' })).snapshot.cells.find((entry) => entry.id === cell.id)
			.definition.settings;
	await wait(cellState, (v) => near(v.position, [160, 100]));
	checks.push('Matrix Cell: shared alignment path commits typed layout');
	stage = 'matrix-cell-resize';
	await drag(
		cellView,
		cellSvg,
		cellView.locator('[data-matrix-cell-resize="BottomRight"]'),
		[240, 160],
		[317, 218],
		[400, 300]
	);
	await wait(cellState, (v) => v.size.width === 160 && v.size.height === 120);
	checks.push('Matrix Cell: resize commits exact snapped dimensions');

	stage = 'source-setup';
	const screens = (payload) => edit(payload, 'mapgyver.screens');
	const screen = (await screens({ kind: 'InspectManager' })).snapshot.screens[0];
	const surface = screen.surfaces[0].reference;
	const peer = (await screens({ kind: 'AddSurface', screen: screen.id, label: 'Alignment peer' }))
		.reference;
	for (const [i, reference] of [surface, peer].entries()) {
		const definition = (await screens({ kind: 'InspectSurface', reference })).snapshot.definition;
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: {
				...definition,
				source,
				crop: i
					? { x: 0.6, y: 0.5, width: 0.2, height: 0.2 }
					: { x: 0.1, y: 0.1, width: 0.2, height: 0.2 }
			}
		});
	}
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.getByRole('region', { name: 'Screens', exact: true })
		.getByRole('button', { name: screen.surfaces[0].label, exact: true })
		.click();
	await tab('surface-source').click();
	const sourceView = page.getByRole('application', { name: 'Surface source view', exact: true });
	await sourceView.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	await wait(
		() => sourceView.locator('[data-related-surface]').count(),
		(v) => v >= 1
	);
	await sourceView.getByRole('button', { name: 'Align geometry', exact: true }).click();
	const cropSvg = sourceView.getByRole('group', { name: 'Source crop controls', exact: true });
	stage = 'source-move';
	await drag(
		sourceView,
		cropSvg,
		cropSvg.getByRole('button', { name: 'Select source crop', exact: true }),
		[0.2, 0.2],
		[0.503, 0.403],
		[1, 1]
	);
	await wait(
		async () =>
			(await screens({ kind: 'InspectSurface', reference: surface })).snapshot.definition.crop,
		(v) => near([v.x, v.y], [0.4, 0.3], 0.0001)
	);
	checks.push('Surface Source: normalized crop movement aligns related/canvas bounds');
	stage = 'source-resize';
	await drag(
		sourceView,
		cropSvg,
		sourceView.getByRole('button', { name: 'Crop corner 3', exact: true }),
		[0.6, 0.5],
		[0.703, 0.598],
		[1, 1]
	);
	await wait(
		async () =>
			(await screens({ kind: 'InspectSurface', reference: surface })).snapshot.definition.crop,
		(v) => near([v.x, v.y, v.width, v.height], [0.4, 0.3, 0.3, 0.3], 0.0001)
	);
	checks.push('Surface Source: crop corner aligns to peer centers while its opposite stays fixed');
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: native.app.pid,
				checks,
				pixels,
				scope:
					'Functional typed geometry, live DOM guides and history. The optional Windows Graphics Capture witness independently compares native Composition image edges and painted guides while held.'
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', pid: native.app.pid, checks, pixels }));
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), errors }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
