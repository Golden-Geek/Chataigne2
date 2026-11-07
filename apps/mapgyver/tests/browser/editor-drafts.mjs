// Actual bundled frontend/Inspector in headless Chromium; no desktop or native FPS claim.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	token,
	stage = 'setup',
	hold = false;
const held = [],
	requests = [],
	receipts = [],
	errors = [],
	checks = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
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
async function wait(read, ready) {
	const deadline = Date.now() + 6000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(15);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const source = (
		await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Draft source' })
	).reference;
	const composition = (
		await edit({ kind: 'CreateMedia', media_kind: 'Composition', label: 'Draft Composition' })
	).reference;
	await edit({
		kind: 'ConfigureMediaResolution',
		media: composition.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const operation = (value) => ({ kind: 'Composition', reference: composition, edit: value });
	const instance = (await edit(operation({ kind: 'AddInstances', sources: [source] }))).instances[0]
		.instance;
	const inspect = async () => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: operation({ kind: 'Inspect' })
		});
		assert(ack.success);
		return { value: ack.result.value.snapshot.instances[0], history: ack.history };
	};
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.routeWebSocket('**', (socket) => {
		const server = socket.connectToServer();
		socket.onMessage((message) => {
			const frame = JSON.parse(String(message));
			if (frame.kind === 'intent' || frame.kind === 'intentBatch')
				requests.push({ stage, ...frame });
			if (hold && frame.kind === 'intent' && frame.intent.kind === 'setParamComponent')
				held.push({ frame, message, server });
			else server.send(message);
		});
		server.onMessage((message) => {
			const frame = JSON.parse(String(message));
			if (frame.kind === 'control') receipts.push({ stage, ...frame.update });
			socket.send(message);
		});
	});
	await page.goto(host.base);
	const tab = (name) =>
		page.getByRole('tab').filter({ hasText: new RegExp('^' + name + '(?::|$)') });
	await tab('Medias').click();
	await page.getByRole('button', { name: 'Draft Composition', exact: true }).click();
	await tab('Composition').click();
	const handle = page.locator('[data-instance-handle="Translate"]');
	await handle.click();
	const position = () => handle.evaluate((element) => parseFloat(element.style.left) * 4);
	async function verifyControl(
		name,
		read,
		display,
		canonical,
		label = 'Position',
		field = false,
		increment = 7
	) {
		const row = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText(label, { exact: true }) });
		const slider = row.locator('.infinite-slider').first(),
			input = row.locator('input.number-field').first();
		await (field ? input : slider).waitFor();
		await wait(read, (value) => !value.history.active_edit_session);
		for (const reject of [false, true]) {
			stage = `${name}: ${reject ? 'rejected' : 'held'} Inspector draft`;
			const before = await read();
			const origin = await wait(
				display,
				(value) => Math.abs(value - canonical(before.value)) < 0.001
			);
			hold = true;
			let pointer;
			if (field) {
				await input.fill(String(origin + increment));
				await input.press('Enter');
			} else {
				const box = await slider.boundingBox();
				assert(box);
				pointer = { x: box.x + box.width * 0.4, y: box.y + box.height * 0.5 };
				await page.mouse.move(pointer.x, pointer.y);
				await page.mouse.down();
				await page.mouse.move(pointer.x + 12, pointer.y);
			}
			await wait(
				() => held.length,
				(value) => value === 1
			);
			assert.equal(held[0].frame.intent.node, Number(await row.getAttribute('data-node-id')));
			const first = await wait(display, (value) => Math.abs(value - origin) > 0.01);
			if (!reject && pointer) await page.mouse.move(pointer.x + 30, pointer.y);
			const expected = Number(await input.inputValue());
			await wait(display, (value) => Math.abs(value - expected) < 0.001);
			assert.deepEqual((await read()).value, before.value, 'held transport cannot author state');
			if (pointer) await page.mouse.up();
			hold = false;
			const pending = held.shift();
			if (reject) {
				pending.frame.intent.component = 2;
				pending.server.send(JSON.stringify(pending.frame));
			} else pending.server.send(pending.message);
			const final = await wait(
				read,
				(value) =>
					!value.history.active_edit_session &&
					Math.abs(canonical(value.value) - (reject ? origin : expected)) < 0.001
			);
			await wait(display, (value) => Math.abs(value - (reject ? origin : expected)) < 0.001);
			const receipt = await wait(
				() =>
					receipts.find(
						(value) => value.request_id === pending.frame.request_id && value.acknowledgement
					),
				Boolean
			);
			assert.equal(receipt.acknowledgement.success, !reject);
			assert.equal(final.history.undo_len, before.history.undo_len + (reject ? 0 : 1));
			if (!reject) {
				await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
				await wait(read, (value) => JSON.stringify(value.value) === JSON.stringify(before.value));
				await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
				await wait(read, (value) => JSON.stringify(value.value) === JSON.stringify(final.value));
			}
			checks.push({
				stage,
				first,
				latest: expected,
				canonical_held: origin,
				final: canonical(final.value),
				rejected: reject,
				exact_history: true
			});
		}
	}
	await verifyControl(
		'Composition Position',
		inspect,
		position,
		(value) => value.definition.transform.position[0]
	);
	stage = 'Matrix setup';
	const matrix = (await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Draft Matrix' }))
		.reference;
	await edit({
		kind: 'ConfigureMediaResolution',
		media: matrix.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const matrixOperation = (value) => ({ kind: 'Matrix', media: matrix.media, edit: value });
	const snapshot = (await edit(matrixOperation({ kind: 'Inspect' }))).snapshot;
	assert.equal(snapshot.items.length, 3);
	assert.equal(snapshot.columns.length, 3);
	await edit(
		matrixOperation({
			kind: 'SetCells',
			assignments: snapshot.items.flatMap((row) =>
				snapshot.columns.map((column) => ({ layer: row.id.id, column: column.id, source }))
			)
		})
	);
	const populated = (await edit(matrixOperation({ kind: 'Inspect' }))).snapshot;
	assert.equal(populated.cells.length, 9);
	const layer = snapshot.items[0].id.id;
	await edit(
		matrixOperation({
			kind: 'ConfigureLayer',
			layer,
			settings: { position: [80, 70], size: { width: 180, height: 150 }, duration_ms: 250 }
		})
	);
	for (const [index, cell] of populated.cells.entries()) {
		await edit(
			matrixOperation({
				kind: 'ConfigureCell',
				layer: cell.layer,
				column: cell.definition.column,
				settings: {
					...cell.definition.settings,
					position: [
						[10, 10],
						[100, 10],
						[10, 80]
					][index % 3],
					size: { width: 80, height: 60 }
				}
			})
		);
	}
	const matrixInspect = async (get) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: matrixOperation({ kind: 'Inspect' })
		});
		assert(ack.success);
		return { value: get(ack.result.value.snapshot), history: ack.history };
	};
	await tab('Medias').click();
	await page.getByRole('button', { name: 'Draft Matrix', exact: true }).click();
	await tab('Composition').click();
	const layerRect = page.locator(`rect[data-layer-id="${layer}"]`);
	await layerRect.click({ position: { x: 4, y: 4 } });
	const inspectLayer = () =>
		matrixInspect((value) => value.items.find((item) => item.id.id === layer).layer);
	await verifyControl(
		'Matrix Layer Position',
		inspectLayer,
		async () => Number(await layerRect.getAttribute('x')),
		(value) => value.position[0]
	);
	await verifyControl(
		'Matrix Layer Size',
		inspectLayer,
		async () => Number(await layerRect.getAttribute('width')),
		(value) => value.size.width,
		'Canvas size',
		true
	);
	await page.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
	const target = populated.cells.find(
		(cell) => cell.layer === layer && cell.definition.column === snapshot.columns[0].id
	);
	const cellRect = page.locator(`rect[data-layout-cell="${target.id}"]`);
	await cellRect.click({ position: { x: 3, y: 3 } });
	const inspectCell = () =>
		matrixInspect((value) => value.cells.find((cell) => cell.id === target.id).definition.settings);
	await verifyControl(
		'Matrix Cell Position',
		inspectCell,
		async () => Number(await cellRect.getAttribute('x')),
		(value) => value.position[0]
	);
	await verifyControl(
		'Matrix Cell Size',
		inspectCell,
		async () => Number(await cellRect.getAttribute('width')),
		(value) => value.size.width,
		'Size',
		true
	);
	stage = 'Surface setup';
	const screens = async (payload) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return { value: ack.result.value, history: ack.history };
	};
	const created = (await screens({ kind: 'CreateScreen', label: 'Draft Screen' })).value;
	const reference = created.surface;
	await screens({ kind: 'RenameSurface', reference, label: 'Draft Surface' });
	const inspectSurface = async () => {
		const result = await screens({ kind: 'InspectSurface', reference });
		return { value: result.value.snapshot.definition, history: result.history };
	};
	const originalSurface = (await inspectSurface()).value;
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: { ...originalSurface, source, crop: { x: 0.1, y: 0.1, width: 0.7, height: 0.7 } }
	});
	await tab('Screens').click();
	const tree = page.locator('section[aria-label="Screens"]');
	const screenRow = tree.getByRole('treeitem', { name: 'Draft Screen', exact: true });
	await screenRow.getByRole('button', { name: 'Draft Screen', exact: true }).click();
	if ((await screenRow.getAttribute('aria-expanded')) === 'false') {
		await screenRow.focus();
		await screenRow.press('ArrowRight');
	}
	await tree.getByRole('button', { name: 'Draft Surface', exact: true }).click();
	await tab('Screen Warping').click();
	const polygon = page.locator(`polygon[data-surface-drop-target="${reference.surface}"]`);
	await verifyControl(
		'Screen corner',
		inspectSurface,
		async () => Number((await polygon.getAttribute('points')).split(/[ ,]/)[0]),
		(value) => value.quad.corners[0].x,
		'Top left',
		true,
		0.05
	);
	await tab('Surface Source').click();
	const crop = page.locator('[aria-label="Source crop controls"] rect');
	await verifyControl(
		'Surface crop origin',
		inspectSurface,
		async () => Number(await crop.getAttribute('x')),
		(value) => value.crop.x,
		'Crop origin',
		true,
		0.05
	);
	await verifyControl(
		'Surface crop size',
		inspectSurface,
		async () => Number(await crop.getAttribute('width')),
		(value) => value.crop.width,
		'Crop size',
		true,
		-0.05
	);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: host.app.pid,
				instance,
				checks,
				requests,
				receipts,
				errors,
				headless: true,
				native_pixels: false
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, requests, receipts, errors }, null, 2)
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
	await writeFile(path.join(directory, 'host.log'), host.log());
}
