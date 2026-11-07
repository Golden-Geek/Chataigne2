// Actual bundled WebView; hold real requests through documented WebSocket routing.
import assert from 'node:assert/strict';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual as equal } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { execFileSync } from 'node:child_process';
import { adapterKey } from './native-adapter.mjs';
import { createGestureQualification } from './gesture-qualification.mjs';
import { createMixedGestureSources } from './gesture-mixed-sources.mjs';
import { createEmptyGestureScenes, verifyEmptyGestureScenes } from './gesture-empty-scenes.mjs';
import { verifyWorkbenchControls } from './gesture-workbench-controls.mjs';
import { heldEditorInput } from './held-editor-input.mjs';
import { contentLock } from './content-lock.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const gpuIndex =
	process.argv[3] === undefined || process.argv[3].startsWith('--')
		? null
		: Number(process.argv[3]);
const liveSync = process.argv.includes('--live-sync');
const cancelInput = process.argv.includes('--cancel-input');
const sharedTexture = process.argv.includes('--shared-texture');
const mixed = process.argv.includes('--mixed-media');
const workbench = process.argv.includes('--workbench-controls');
const populated = process.argv.includes('--populated-3x3') || mixed;
const colors = [
	{ rgba: [1, 0, 0, 1], rgb: [255, 0, 0] },
	{ rgba: [0, 1, 0, 1], rgb: [0, 255, 0] },
	{ rgba: [0, 0, 1, 1], rgb: [0, 0, 255] },
	{ rgba: [1, 1, 0, 1], rgb: [255, 255, 0] },
	{ rgba: [0, 1, 1, 1], rgb: [0, 255, 255] },
	{ rgba: [1, 0, 1, 1], rgb: [255, 0, 255] },
	{ rgba: [1, 1, 1, 1], rgb: [255, 255, 255] },
	{ rgba: [0.5, 0.5, 0.5, 1], rgb: [188, 188, 188] },
	{ rgba: [1, 0.5, 0, 1], rgb: [255, 188, 0] }
];
const matrixSources = [];
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware =
	gpuIndex === null
		? null
		: JSON.parse(
				execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
			);
if (hardware)
	assert(
		Number.isInteger(gpuIndex) &&
			hardware.adapters[gpuIndex] &&
			!hardware.adapters[gpuIndex].software
	);
const native = await openNativeEditor({
	prepareProfile: hardware
		? async (root) => {
				const folder = path.join(root, 'MapGyver');
				await mkdir(folder, { recursive: true });
				execFileSync(
					probe,
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(folder, 'preferences.json'),
						'--selected',
						adapterKey(hardware.adapters[gpuIndex].id)
					],
					{ windowsHide: true }
				);
			}
		: undefined
});
const page = native.page;
page.setDefaultTimeout(9000);
let token,
	stage = 'setup',
	hold = false,
	closing = false,
	qualification;
let mixedSources,
	emptyReports = [],
	workbenchReport;
let livePreview = null;
let livePreviewId = null;
let cancellationAttempt = null;
const liveChecks = [],
	cancellationChecks = [];
const held = [],
	requests = [],
	receipts = [],
	errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
await page.routeWebSocket('**', (socket) => {
	const server = socket.connectToServer();
	socket.onMessage((message) => {
		const frame = JSON.parse(String(message));
		const intent = frame.intent;
		const operation =
			intent?.namespace === 'mapgyver.screens'
				? intent.payload?.kind
				: (intent?.payload?.edit?.kind ?? intent?.payload?.kind);
		const geometry =
			frame.kind === 'intent' &&
			intent?.kind === 'invokeAppOperation' &&
			[
				'EditLayerGeometry',
				'EditCellGeometry',
				'EditInstanceGeometry',
				'EditSurfaceGeometry'
			].includes(operation);
		if (frame.kind === 'intent')
			requests.push({ id: frame.request_id, kind: intent.kind, operation, geometry, stage });
		if (geometry && hold) held.push({ server, message, id: frame.request_id });
		else server.send(message);
	});
	server.onMessage((message) => {
		const frame = JSON.parse(String(message));
		if (frame.kind === 'control' && ['applied', 'rejected'].includes(frame.update.phase))
			receipts.push({
				id: frame.update.request_id,
				phase: frame.update.phase,
				acknowledgements: frame.update.acknowledgements ?? [frame.update.acknowledgement]
			});
		if (!closing) socket.send(message);
	});
});
async function wait(read, ready) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(10000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function operation(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const edit = (payload) => operation('mapgyver.media', payload);
const screens = (payload) => operation('mapgyver.screens', payload);
const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
const values = (shape) =>
	shape.evaluate((element) =>
		element.tagName.toLowerCase() === 'rect'
			? ['x', 'y', 'width', 'height'].map((key) => Number(element.getAttribute(key)))
			: element
					.getAttribute('points')
					.trim()
					.split(/[\s,]+/)
					.map(Number)
	);
const near = (a, b, tolerance = 0.00001) =>
	a.length === b.length && a.every((v, i) => Math.abs(v - b[i]) < tolerance);
async function liveInput(name, shape, control, inspector = false) {
	stage = name;
	liveChecks.push(
		await heldEditorInput({
			page,
			name,
			shape,
			control,
			inspector,
			preview: livePreview?.locator('.native-preview')
		})
	);
}
async function liveInspector(name, shape) {
	const row = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Position', { exact: true }) });
	await liveInput(name, shape, row.locator('.infinite-slider').first(), true);
}
async function cancelGesture(name, svg, shape, control, inspect, tolerance) {
	stage = `${name} cancellation`;
	const requestStart = requests.length;
	const before = await inspect(),
		initial = await values(shape),
		box = await control.boundingBox();
	assert(box);
	const point = { x: box.x + box.width * 0.43, y: box.y + box.height * 0.43 };
	await page.mouse.move(point.x, point.y);
	await page.mouse.down();
	hold = true;
	await page.mouse.move(point.x + 8, point.y + 5);
	await wait(
		() => held.length,
		(count) => count === 1
	);
	const dispatched = await wait(
		() => values(shape),
		(value) => !near(value, initial)
	);
	await page.mouse.move(point.x + 21, point.y + 13);
	const unsent = await wait(
		() => values(shape),
		(value) => !near(value, dispatched)
	);
	cancellationAttempt = { name, initial, dispatched, unsent, tolerance };
	assert.deepEqual(await inspect(), before, 'held edit has not authored state');
	await page.keyboard.press('Escape');
	await wait(
		() => svg.getAttribute('data-geometry-presentation'),
		(value) => value === 'committed'
	);
	assert(
		near(await values(shape), initial, tolerance),
		'cancellation clears the unacknowledged hint'
	);
	await page.mouse.move(point.x + 30, point.y + 18);
	await page.mouse.up();
	assert.equal(held.length, 1, 'cancellation must not dispatch the queued successor');
	hold = false;
	const pending = held.shift();
	pending.server.send(pending.message);
	const end = await wait(
		() => requests.slice(requestStart).find((request) => request.kind === 'endEdit'),
		Boolean
	);
	await wait(() => receipts.some((receipt) => receipt.id === end.id), Boolean);
	assert(
		receipts.some((receipt) => receipt.id === pending.id && receipt.phase === 'applied'),
		'the preserved dispatched edit has its own applied receipt'
	);
	const after = await wait(inspect, (value) => !equal(value, before));
	cancellationAttempt.actual = await values(shape);
	await wait(
		() => values(shape),
		(value) => near(value, dispatched, tolerance)
	);
	const cohort = requests.slice(requestStart);
	assert.equal(
		cohort.filter((request) => request.geometry).length,
		1,
		'only dispatched input survives'
	);
	assert.equal(
		cohort.filter((request) => request.kind === 'endEdit').length,
		1,
		'one history close'
	);
	assert(
		!near(await values(shape), unsent, tolerance),
		'queued pointer position is never authored'
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(inspect, (value) => equal(value, before));
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(inspect, (value) => equal(value, after));
	await wait(
		() => values(shape),
		(value) => near(value, dispatched, tolerance)
	);
	cancellationChecks.push({
		name,
		initial,
		dispatched,
		discarded: unsent,
		final: await values(shape),
		tolerance,
		geometry_requests: 1,
		history_closes: 1,
		canonical_undo_redo: true,
		presentation_cleared_before_receipt: true
	});
}
async function gesture(name, svg, shape, control, inspect, tolerance = 0.00001) {
	stage = name;
	await wait(
		() => svg.getAttribute('data-geometry-presentation'),
		(v) => v === 'committed'
	);
	await wait(() => control.isEnabled(), Boolean);
	const before = await inspect(),
		initial = await values(shape);
	const box = await control.boundingBox();
	assert(box);
	const point = { x: box.x + box.width * 0.43, y: box.y + box.height * 0.43 };
	const inputHit =
		name === 'Matrix Cell translation'
			? await control.evaluate((element, point) => {
					const hit = document.elementFromPoint(point.x, point.y);
					return {
						selected: element.getAttribute('data-layout-cell'),
						actual: hit?.getAttribute('data-layout-cell'),
						matches: hit === element
					};
				}, point)
			: null;
	if (inputHit)
		assert(
			inputHit.matches,
			`Selected Cell must own its overlapping drag area: ${JSON.stringify(inputHit)}`
		);
	await page.mouse.move(point.x, point.y);
	await page.mouse.down();
	hold = true;
	await page.mouse.move(point.x + 8, point.y + 5);
	await wait(
		() => held.length,
		(n) => n === 1
	);
	await wait(
		() => svg.getAttribute('data-geometry-presentation'),
		(v) => v === 'pending'
	);
	const first = await wait(
		() => values(shape),
		(v) => !near(v, initial)
	);
	assert.deepEqual(await inspect(), before, 'held request has not authored state');
	await page.mouse.move(point.x + 21, point.y + 13);
	const latest = await wait(
		() => values(shape),
		(v) => !near(v, first)
	);
	assert.equal(held.length, 1, 'one in-flight request only');
	await qualification?.pending({ name, svg, shape, control, initial, latest });
	assert.deepEqual(
		await inspect(),
		before,
		'native presentation/output observation cannot author the held edit'
	);
	const old = held.shift();
	old.server.send(old.message);
	await wait(
		() => held.length,
		(n) => n === 1
	);
	await wait(() => receipts.some((r) => r.id === old.id), Boolean);
	assert(!equal(await inspect(), before), 'older real request was applied');
	assert(
		near(await values(shape), latest),
		'older receipt must not rewind the latest pointer geometry'
	);
	const start = requests.length;
	await page.mouse.up();
	hold = false;
	const last = held.shift();
	last.server.send(last.message);
	const end = await wait(() => requests.slice(start).find((r) => r.kind === 'endEdit'), Boolean);
	await wait(() => receipts.some((r) => r.id === end.id), Boolean);
	await wait(
		() => svg.getAttribute('data-geometry-presentation'),
		(v) => v === 'committed'
	);
	const after = await inspect();
	assert(!equal(after, before));
	const final = await values(shape);
	const maximumCoordinateError = Math.max(
		...final.map((value, index) => Math.abs(value - latest[index]))
	);
	assert(
		near(final, latest, tolerance),
		`canonical geometry exceeds authored precision: ${JSON.stringify({ latest, final, maximumCoordinateError, tolerance })}`
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(inspect, (v) => equal(v, before));
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(inspect, (v) => equal(v, after));
	await wait(
		() => values(shape),
		(v) => near(v, final)
	);
	checks.push({
		name,
		input_hit: inputHit,
		initial,
		held_first: first,
		held_latest: latest,
		final,
		maximum_coordinate_error: maximumCoordinateError,
		one_inflight: true,
		old_receipt_no_rewind: true,
		canonical_undo_redo: true
	});
	await qualification?.after(name);
	if (cancelInput)
		await cancelGesture(
			name,
			svg,
			shape,
			control,
			inspect,
			name.startsWith('Composition') ? 0.0001 : tolerance
		);
}
try {
	await page.reload();
	token = (await post('/api/ui/session', {})).token;
	if (mixed) {
		assert(hardware, 'Mixed native media fixture requires a pinned GPU');
		mixedSources = await createMixedGestureSources({ edit, directory, colors, adapter: gpuIndex });
	}
	const { reference: source } = mixed
		? mixedSources.records[0]
		: await edit({
				kind: 'CreateMedia',
				media_kind: sharedTexture ? 'SharedTexture' : 'SolidColor',
				label: 'Gesture source'
			});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
	});
	if (sharedTexture)
		await edit({
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'Spout', sender: 'Spout Demo Sender' }
		});
	if (!sharedTexture && !mixed && (hardware || populated)) {
		await edit({
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
	}
	matrixSources.push(source);
	if (mixed) matrixSources.push(...mixedSources.records.slice(1).map((record) => record.reference));
	else if (populated) {
		for (let index = 1; index < colors.length; index++) {
			const { reference } = await edit({
				kind: 'CreateMedia',
				media_kind: 'SolidColor',
				label: `Gesture source ${index + 1}`
			});
			await edit({
				kind: 'ConfigureMediaResolution',
				media: reference.media,
				size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
			});
			await edit({
				kind: 'ConfigureChannel',
				reference,
				source: { kind: 'SolidColor', rgba: colors[index].rgba }
			});
			matrixSources.push(reference);
		}
	}
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Gesture Matrix'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const matrix = (editValue) => edit({ kind: 'Matrix', media: reference.media, edit: editValue });
	const matrixSnapshot = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const snapshot = await matrixSnapshot(),
		layer = snapshot.items[0].id.id,
		column = snapshot.columns[0].id;
	assert.equal(snapshot.items.length, 3);
	assert.equal(snapshot.columns.length, 3);
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [80, 70], size: { width: 180, height: 150 }, duration_ms: 250 }
	});
	await matrix({
		kind: 'SetCells',
		assignments: snapshot.items.flatMap((row, rowIndex) =>
			snapshot.columns.map((col, columnIndex) => ({
				layer: row.id.id,
				column: col.id,
				source: populated
					? matrixSources[rowIndex * 3 + columnIndex]
					: row.id.id === layer && col.id === column
						? source
						: null
			}))
		)
	});
	if (populated) {
		const configured = await matrixSnapshot();
		assert.equal(configured.cells.length, 9);
		const view = (
			await matrix({
				kind: 'InspectViewport',
				viewport: { row_start: 0, row_count: 3, axis_start: 0, axis_count: 3 },
				selection: null
			})
		).snapshot;
		const key = (reference) => `${reference.media}:${reference.channel}`;
		assert.deepEqual(
			view.cells.map((cell) => key(cell.source_preview)).sort(),
			matrixSources.map(key).sort(),
			'Nine owned Cell aliases resolve to the nine distinct original source references'
		);
	}
	await matrix({
		kind: 'ConfigureCell',
		layer,
		column,
		settings: {
			position: [20, 20],
			size: { width: 100, height: 80 },
			fit_mode: 'Fill',
			opacity: 1,
			enabled: true
		}
	});
	await matrix({ kind: 'LaunchColumn', column, mode: 'Cut' });
	const cell = (await matrixSnapshot()).cells.find(
		(c) => c.layer === layer && c.definition.column === column
	).id;
	// Complete document topology before the continuous output observation starts.
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Gesture Composition'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: composition.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const comp = (value) => edit({ kind: 'Composition', reference: composition, edit: value });
	const { instances } = await comp({ kind: 'AddInstances', sources: [source] });
	const compSnapshot = async () => (await comp({ kind: 'Inspect' })).snapshot;
	const original = (await compSnapshot()).instances[0];
	await comp({
		kind: 'ConfigureInstance',
		instance: instances[0].instance,
		definition: {
			...original.definition,
			transform: { position: [190, 140], scale: [1, 1], rotation_degrees: 23, pivot: [0.5, 0.5] }
		}
	});
	const { screen, surface } = await screens({ kind: 'CreateScreen', label: 'Gesture Screen' });
	await screens({ kind: 'RenameSurface', reference: surface, label: 'Gesture Surface' });
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Gesture-${screen}`, extent: { width: 640, height: 360 } }
		}
	});
	const surfaceSnapshot = async () =>
		(await screens({ kind: 'InspectSurface', reference: surface })).snapshot;
	const initialSurface = await surfaceSnapshot();
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: {
			...initialSurface.definition,
			source,
			crop: { x: 0.2, y: 0.2, width: 0.5, height: 0.5 },
			quad: {
				corners: [
					{ x: 0.2, y: 0.2 },
					{ x: 0.8, y: 0.2 },
					{ x: 0.8, y: 0.8 },
					{ x: 0.2, y: 0.8 }
				]
			}
		}
	});
	const emptyScene = mixed ? await createEmptyGestureScenes(edit, screens) : null;
	if (liveSync) {
		await writeFile(
			path.join(directory, 'ready.json'),
			JSON.stringify({
				pid: native.app.pid,
				binary: process.env.MAPGYVER_TEST_BINARY,
				shared_texture: sharedTexture
			})
		);
		const until = Date.now() + 180000;
		while (!(await stat(path.join(directory, 'activated')).catch(() => null))) {
			assert(Date.now() < until, 'owned live-editor foreground readiness expired');
			await delay(100);
		}
	}
	if (hardware)
		qualification = await createGestureQualification({
			native,
			directory,
			post,
			source,
			hardware,
			gpuIndex
		});
	if (emptyScene)
		emptyReports = await verifyEmptyGestureScenes({
			page,
			edit,
			screens,
			scene: emptyScene,
			qualification
		});
	await tab('Medias').click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Gesture Matrix', exact: true })
		.click();
	await tab('Matrix').click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	if (liveSync) {
		const group = tab('Medias').locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		livePreview = page.locator('.media-preview-panel:visible');
		const previewId = await livePreview.getAttribute('data-panel-id');
		assert(previewId);
		livePreviewId = previewId;
		livePreview = page.locator(`.media-preview-panel[data-panel-id="${previewId}"]`);
		await wait(
			() => livePreview.getAttribute('data-preview-media'),
			(value) => value === reference.media
		);
		await (await contentLock(livePreview, false)).click();
		await livePreview.locator('.native-preview[data-preview-state="available"]').waitFor();
	}
	if (populated)
		await qualification?.grid({ phase: 'populated-before-gestures', snapshot, colors });
	await grid.getByTitle('Select Layer 1', { exact: true }).click();
	await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	const panel = page.locator(
		'.workbench-editor[data-panel-id="composition"][data-spatial-kind="Matrix"]'
	);
	const layerSvg = panel.getByRole('group', { name: 'Matrix Layer layout controls', exact: true });
	const layerRect = layerSvg.locator(`[data-layer-id="${layer}"]`);
	await gesture(
		'Matrix Layer translation',
		layerSvg,
		layerRect,
		layerRect,
		async () => (await matrixSnapshot()).items.find((i) => i.id.id === layer).layer,
		1.01
	);
	if (liveSync) {
		await liveInput('Matrix Layer live canvas', layerRect, layerRect);
		await liveInspector('Matrix Layer live Inspector', layerRect);
	}
	await tab('Matrix').click();
	const firstCell = grid.getByTitle(`${snapshot.items[0].label} / ${snapshot.columns[0].label}`, {
		exact: true
	});
	const firstCellBounds = await firstCell.boundingBox();
	assert(firstCellBounds);
	// The accepted Matrix controls occupy the top-left; select its ordinary image area.
	await firstCell.click({
		position: { x: firstCellBounds.width * 0.72, y: firstCellBounds.height * 0.5 }
	});
	await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	await panel.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
	const cellSvg = panel.getByRole('group', { name: 'Matrix Cell layout controls', exact: true });
	const cellRect = cellSvg.locator(`[data-layout-cell="${cell}"]`);
	await gesture(
		'Matrix Cell translation',
		cellSvg,
		cellRect,
		cellRect,
		async () => (await matrixSnapshot()).cells.find((c) => c.id === cell).definition.settings,
		1.01
	);
	if (liveSync) {
		await liveInput('Matrix Cell live canvas', cellRect, cellRect);
		await liveInspector('Matrix Cell live Inspector', cellRect);
	}
	await tab('Medias').click();
	const manager = page.locator('section[aria-label="Medias"]');
	const parent = manager.getByRole('treeitem', { name: 'Gesture Composition', exact: true });
	await parent.getByRole('button', { name: 'Gesture Composition', exact: true }).click();
	if (liveSync) {
		await page.locator(`.gc-dv-tab[data-content-panel-id="${livePreviewId}"]`).click();
		await (await contentLock(livePreview, true)).click();
		await wait(
			() => livePreview.getAttribute('data-preview-media'),
			(value) => value === composition.media
		);
		await (await contentLock(livePreview, false)).click();
		await tab('Medias').click();
	}
	if ((await parent.getAttribute('aria-expanded')) === 'false') {
		await parent.focus();
		await parent.press('ArrowRight');
	}
	// Follow this parent's hierarchy; nested Compositions can legitimately reuse labels.
	await parent.focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('Enter');
	if (liveSync) {
		await page.locator(`.gc-dv-tab[data-content-panel-id="${livePreviewId}"]`).click();
	}
	const compPanel = page.locator('section[aria-label="Composition"]');
	const compSvg = compPanel.getByRole('group', {
		name: 'Composition transform controls',
		exact: true
	});
	const compShape = compSvg
		.locator(`polygon[data-instance-drop-target="${instances[0].instance}"]`)
		.first();
	const transform = async () => (await compSnapshot()).instances[0].definition.transform;
	await gesture(
		'Composition translation',
		compSvg,
		compShape,
		compPanel.locator('[data-instance-handle="Translate"]'),
		transform
	);
	if (liveSync) {
		await liveInput(
			'Composition live canvas',
			compShape,
			compPanel.locator('[data-instance-handle="Translate"]')
		);
		await liveInspector('Composition live Inspector', compShape);
	}
	await gesture(
		'Composition rotated resize',
		compSvg,
		compShape,
		compPanel.locator('[data-instance-resize="BottomRight"]'),
		transform,
		// Canvas-pixel f32 transforms at400px have an ULP above0.00001.
		0.0001
	);
	await tab('Screens').click();
	const tree = page.locator('section[aria-label="Screens"]');
	const screenRow = tree.getByRole('treeitem', { name: 'Gesture Screen', exact: true });
	await screenRow.getByRole('button', { name: 'Gesture Screen', exact: true }).click();
	if ((await screenRow.getAttribute('aria-expanded')) === 'false') {
		await screenRow.focus();
		await screenRow.press('ArrowRight');
	}
	await tree.getByRole('button', { name: initialSurface.label, exact: true }).click();
	await tab('Screen Warping').click();
	const warp = page.locator('[data-panel-id="screen-warp"]');
	await warp
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	await wait(
		() => warp.getAttribute('aria-busy'),
		(v) => v === 'false'
	);
	const warpSvg = warp.getByRole('group', { name: 'Surface mapping controls', exact: true });
	await gesture(
		'Screen warp corner',
		warpSvg,
		warpSvg.locator(`polygon[data-native-hit-id="${surface.surface}"]`),
		warp.getByRole('button', { name: 'Warp corner 1', exact: true }),
		async () => (await surfaceSnapshot()).definition.quad
	);
	await tab('Surface Source').click();
	const cropPanel = page.locator('section[aria-label="Surface Source"]');
	await cropPanel
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	await wait(
		() => cropPanel.getAttribute('aria-busy'),
		(v) => v === 'false'
	);
	const cropSvg = cropPanel.getByRole('group', { name: 'Source crop controls', exact: true });
	await gesture(
		'Surface Source crop',
		cropSvg,
		cropSvg.locator('rect'),
		cropPanel.getByRole('button', { name: 'Crop corner 1', exact: true }),
		async () => (await surfaceSnapshot()).definition.crop
	);
	if (populated) {
		await tab('Medias').click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Gesture Matrix', exact: true })
			.click();
		await tab('Matrix').click();
		await qualification?.grid({ phase: 'populated-after-gestures', snapshot, colors });
	}
	if (workbench) {
		stage = 'ordinary controls, shared scrolling and docking';
		workbenchReport = await verifyWorkbenchControls({ page, edit, post, qualification });
	}
	assert.deepEqual(errors, []);
	assert(
		receipts.every(
			(r) => r.phase === 'applied' && r.acknowledgements.every((a) => !a || a.success)
		),
		JSON.stringify(receipts.filter((r) => r.phase !== 'applied'))
	);
	const qualified = await qualification?.finish();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				app_pid: native.app.pid,
				browser: native.browser.version(),
				checks,
				live_checks: liveChecks,
				cancellation_checks: cancellationChecks,
				cancellation_attempt: cancellationAttempt,
				workbench: workbenchReport,
				workload: {
					populated_3x3: populated,
					source_references: matrixSources,
					mixed_media: mixedSources?.records,
					owned_source: mixedSources?.source,
					empty: emptyReports
				},
				requests,
				receipts,
				errors,
				qualification: qualified,
				scope: qualified
					? qualified.scope
					: 'Functional bundled-app local geometry and history; no presented pixels or high-refresh timing claim'
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({ result: 'pass', checks: checks.map((c) => c.name), app_pid: native.app.pid })
	);
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				stage,
				error: String(error),
				cancellation_attempt: cancellationAttempt,
				checks,
				live_checks: liveChecks,
				cancellation_checks: cancellationChecks,
				requests,
				receipts,
				errors,
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	closing = true;
	try {
		await qualification?.close();
	} finally {
		try {
			await native.close();
		} finally {
			await mixedSources?.close();
		}
	}
}
