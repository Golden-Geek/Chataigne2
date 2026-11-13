// M10/7: the ordinary bundled app, real generic Inspector and independent SDK pixels.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';
import { inspectLocalVideoLibraries } from './video-runtime-libraries.mjs';
import { inspectNativeDiagnostics } from './native-diagnostics.mjs';
import { clearMatrixScene } from './matrix-scene.mjs';

const execute = promisify(execFile);
const directory = path.resolve(process.argv[2]);
const selected = process.env.MAPGYVER_VIDEO_ADAPTER;
const observer = path.resolve('target/debug/native_qualification.exe');
const binary = path.resolve(process.env.MAPGYVER_TEST_BINARY ?? 'target/debug/MapGyver.exe');
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
const hardware = JSON.parse(await run(observer, ['--role', 'hardware']));
assert(hardware.adapters.some((gpu) => !gpu.software && adapterKey(gpu.id) === selected));
await mkdir(directory, { recursive: true });
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const folder = path.join(root, 'MapGyver');
		await mkdir(folder, { recursive: true });
		await run(observer, [
			'--role',
			'gpu-preferences',
			'--file',
			path.join(folder, 'preferences.json'),
			'--selected',
			selected
		]);
	}
});
const { page } = native;
const errors = [],
	observations = [],
	phases = [];
page.on('pageerror', (error) => errors.push(String(error)));
let phase = 'startup',
	token,
	primary,
	source,
	surface,
	screen,
	surfaceDefinition,
	adapter,
	report;
const { rejected, observed } = await observeNativeRejectedRequests(page, () => phase);
const sender = `MapGyver-Video-Matrix-${randomUUID()}`;
let requests = 0,
	ordinal = 0;
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(8000)
	});
	const result = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(result)}`);
	return result;
}
async function edit(payload, namespace = 'media') {
	requests++;
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, `${phase}: ${JSON.stringify(result)}`);
	return result.result.value;
}
const control = (request) =>
	page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
async function wait(read, accept, label, milliseconds = 10000) {
	const end = Date.now() + milliseconds;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < end, `${phase}/${label}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
const state = async (media = source.media) =>
	(await edit({ kind: 'InspectVideo', media })).snapshot;
const video = async (operation, media = source.media) =>
	(await edit({ kind: 'Video', media, edit: operation })).snapshot;
const matrix = (operation, reference = primary) =>
	edit({ kind: 'Matrix', media: reference.media, edit: operation });
const create = async (kind, label) =>
	(await edit({ kind: 'CreateMedia', media_kind: kind, label })).reference;
const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
const parameter = (label) =>
	page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
async function select(label) {
	await tab('Medias').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
}
async function route(reference) {
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...surfaceDefinition, source: reference }
		},
		'screens'
	);
}
async function capture(label, frame = null, profile = 'gray') {
	const deadline = Date.now() + 8000;
	for (;;) {
		const folder = path.join(directory, `${String(++ordinal).padStart(3, '0')}-${label}`);
		await mkdir(folder);
		const shape = JSON.parse(
			await run(observer, [
				'--role',
				'video-pixels',
				'--adapter',
				String(adapter.index),
				'--name',
				sender,
				'--directory',
				folder
			])
		);
		assert.deepEqual(shape, { width: 160, height: 120, retired: true });
		const result = JSON.parse(
			await run('python', [
				'apps/mapgyver/tests/native/video_policy_pixels.py',
				path.join(folder, 'pixels.png'),
				...(frame === null ? [] : ['--frame', String(frame), '--profile', profile])
			])
		);
		const entry = {
			phase,
			label,
			...result,
			transport: source ? await state() : null,
			file: path.join(folder, 'pixels.png')
		};
		observations.push(entry);
		if (frame === null || result.matches) return entry;
		assert(
			Date.now() < deadline,
			`${label}: expected new output pixels, all attempts retained: ${JSON.stringify(entry)}`
		);
		await delay(40);
	}
}
async function policies(activation, deactivation) {
	// All nine pairs go through the current shared generic Inspector widgets.
	for (const [label, value] of [
		['On Activate', activation],
		['On Deactivate', deactivation]
	]) {
		const before = { ...observed };
		const widget = parameter(label).locator('select');
		if ((await widget.inputValue()) === value) continue;
		await widget.focus();
		await widget.selectOption(value);
		await widget.blur();
		await delay(40);
		await wait(
			() => observed,
			(value) =>
				value.requests > before.requests &&
				value.requests - before.requests === value.receipts - before.receipts,
			'enum edit session receipts'
		);
		assert.deepEqual(rejected, [], 'enum sessions must commit and end before the next widget');
	}
	await wait(
		state,
		(value) =>
			value.settings.on_activate === activation && value.settings.on_deactivate === deactivation,
		'generic policies'
	);
}
async function launch(column, mode = 'Cut', reference = primary) {
	const before = (await matrix({ kind: 'InspectPlayback' }, reference)).snapshot;
	const invoked = await matrix({ kind: 'LaunchColumn', column, mode }, reference);
	assert.equal(
		invoked.kind,
		'MatrixControlInvoked',
		'the public operation acknowledges its trigger'
	);
	// A trigger acknowledgement is not an acquired Video use. Qualify a shared consumer
	// only after its actual playback receipt has committed, before withdrawing another.
	await wait(
		async () => (await matrix({ kind: 'InspectPlayback' }, reference)).snapshot,
		(value) =>
			value.desired_column === column && value.launch_id && value.launch_id !== before.launch_id,
		'committed Matrix activation'
	);
}
async function transition(column, duration_ms, reference = primary) {
	await matrix(
		{
			kind: 'ConfigureColumn',
			column,
			definition: { transition: { duration_ms, curve: 'Linear' } }
		},
		reference
	);
}
async function shape(reference, count = 1) {
	await clearMatrixScene((payload) => edit(payload), reference.media);
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 160, height: 120 } }
	});
	const rows = (await matrix({ kind: 'AddLayers', parent: null, count }, reference)).items.map(
		(item) => item.id
	);
	const columns = (await matrix({ kind: 'AddColumns', parent: null, count: 3 }, reference)).columns;
	for (const row of rows)
		await matrix(
			{
				kind: 'ConfigureLayer',
				layer: row,
				settings: { position: [0, 0], size: { width: 160, height: 120 }, duration_ms: 350 }
			},
			reference
		);
	return { rows, columns };
}
async function assign(reference, row, column, owner, copy = false) {
	const result = await matrix(
		{ kind: 'DropSource', layer: row, column, source: owner, copy },
		reference
	);
	await matrix(
		{
			kind: 'ConfigureCell',
			layer: row,
			column,
			settings: {
				position: [0, 0],
				size: { width: 160, height: 120 },
				fit_mode: 'Stretch',
				opacity: 1,
				enabled: true
			}
		},
		reference
	);
	return result;
}
try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	adapter = await observeNativeAdapter(page, hardware);
	assert.equal(adapter.key, selected);
	const libraries = await inspectLocalVideoLibraries(native.app.pid, binary);
	source = await create('Video', 'Matrix policy Video');
	await edit({
		kind: 'ConfigureVideoFile',
		reference: source,
		file: path.resolve('crates/golden_video/src/decode/tests/fixtures/h264.mp4')
	});
	primary = await create('Matrix', 'Matrix policy target');
	const { rows, columns } = await shape(primary);
	await assign(primary, rows[0], columns[0], source.media);
	({ screen, surface } = await edit(
		{ kind: 'CreateScreen', label: 'Matrix Video policy output' },
		'screens'
	));
	surfaceDefinition = (await edit({ kind: 'InspectSurface', reference: surface }, 'screens'))
		.snapshot.definition;
	await route(source);
	await edit(
		{
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				target: { kind: 'Spout', sender, extent: { width: 160, height: 120 } }
			}
		},
		'screens'
	);
	await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference: source })).status,
		(value) => value.state === 'Ready' && value.duration_ms === 250,
		'actual decoder'
	);
	await select('Matrix policy Video');
	assert.equal(await parameter('On Activate').locator('select').inputValue(), 'PlayFromStart');
	assert.equal(await parameter('On Deactivate').locator('select').inputValue(), 'Stop');
	await video({ kind: 'Speed', speed: 0 });
	await video({ kind: 'Seek', position_ms: 125 });
	await capture('prepared-independent-oracle', 3);

	for (const activation of ['PlayFromStart', 'Resume', 'Nothing']) {
		for (const deactivation of ['Stop', 'Pause', 'Nothing']) {
			phase = `${activation}-${deactivation}`;
			await matrix({ kind: 'ClearPlayback' });
			await policies(activation, deactivation);
			await video({ kind: 'Pause' });
			await video({ kind: 'Seek', position_ms: 125 });
			await route(primary);
			await transition(columns[0], 150);
			await launch(columns[0], 'Configured');
			const expected = activation === 'PlayFromStart' ? 0 : 125;
			const playing = activation !== 'Nothing';
			await wait(
				state,
				(value) => value.position_ms === expected && value.playing === playing,
				'activation'
			);
			await delay(200);
			await capture('activated', expected === 0 ? 0 : 3);
			await wait(
				() => parameter('Playing').locator('input[type=checkbox]').isChecked(),
				(value) => value === playing,
				'Inspector activation feedback'
			);
			await transition(columns[1], 350);
			await launch(columns[1], 'Configured');
			const outgoing = await state();
			assert.equal(outgoing.playing, playing, 'outgoing boundary must not act at launch');
			assert.equal(outgoing.position_ms, expected);
			await delay(450);
			const mode =
				deactivation === 'Stop'
					? 'Stopped'
					: deactivation === 'Pause'
						? 'Paused'
						: playing
							? 'Playing'
							: 'Paused';
			const position = deactivation === 'Stop' ? 0 : expected;
			await wait(
				state,
				(value) => value.settings.mode === mode && value.position_ms === position,
				'deactivation'
			);
			await route(source);
			await capture('deactivated', position === 0 ? 0 : 3);
			phases.push({ activation, deactivation, outgoing, final: await state() });
		}
	}

	phase = 'moving-outgoing';
	await policies('PlayFromStart', 'Stop');
	await video({ kind: 'Loop', looping: true });
	await video({ kind: 'Speed', speed: 1 });
	await route(primary);
	await launch(columns[0]);
	await delay(250);
	await transition(columns[1], 4500);
	await launch(columns[1], 'Configured');
	const moving = [];
	for (const interval of [17, 53, 89, 127, 173, 211]) {
		await delay(interval);
		const image = await capture('moving-outgoing');
		assert(image.transport.playing, 'outgoing Video clock must remain running');
		moving.push(image.rgba[1]);
	}
	assert(
		new Set(moving.map((rgba) => rgba.slice(0, 3).join(','))).size >= 3,
		'actual outgoing pixels must keep changing'
	);
	assert(
		moving.at(-1)[3] < moving[0][3] && moving.at(-1)[3] > 0,
		'actual pixels carry a live outgoing fade'
	);
	await wait(
		state,
		(value) => value.settings.mode === 'Stopped' && value.position_ms === 0,
		'moving outgoing end'
	);

	phase = 'manual-and-reactivation-fences';
	await video({ kind: 'Speed', speed: 0 });
	await launch(columns[0]);
	await transition(columns[1], 600);
	await launch(columns[1], 'Configured');
	await video({ kind: 'Seek', position_ms: 125 });
	await delay(750);
	assert.equal((await state()).position_ms, 125);
	assert((await state()).playing);
	await route(source);
	await capture('manual-seek-survives-deferred-stop', 3);
	await route(primary);
	await launch(columns[0]);
	await launch(columns[1], 'Configured');
	await launch(columns[0], 'Configured');
	await delay(750);
	assert((await state()).playing);
	assert.equal((await state()).position_ms, 0);
	await capture('reactivation-survives-old-deadline', 0);

	phase = 'matrix-enabled';
	const matrixDocument = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const matrixOwner = matrixDocument.nodes.find((node) => node.uuid === primary.media);
	assert(matrixOwner);
	await video({ kind: 'Seek', position_ms: 125 });
	for (const enabled of [false, true]) {
		assert(
			(
				await post('/api/ui/intent', {
					kind: 'patchMeta',
					node: matrixOwner.node_id,
					patch: { enabled }
				})
			).success
		);
		await wait(
			state,
			(value) => value.playing === enabled && value.position_ms === 0,
			'Matrix enabled transport'
		);
		await route(enabled ? primary : source);
		await capture(enabled ? 'matrix-enabled-reactivation' : 'matrix-disabled-withdrawal', 0);
	}

	phase = 'shared-matrix-and-layer-stop';
	await policies('Resume', 'Stop');
	await video({ kind: 'Seek', position_ms: 125 });
	const second = await create('Matrix', 'Second shared Matrix');
	const secondShape = await shape(second, 2);
	for (const row of secondShape.rows)
		await assign(second, row, secondShape.columns[0], source.media);
	await launch(secondShape.columns[0], 'Cut', second);
	await matrix({ kind: 'ClearPlayback' });
	assert((await state()).playing, 'another Matrix owns the canonical clock');
	await route(second);
	await capture('two-shared-layers-one-clock', 3);
	// M14/10 will add fading Layer Stop; qualify today's immediate scoped withdrawal first.
	const document = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	for (const [index, row] of secondShape.rows.entries()) {
		const owner = document.nodes.find((node) => node.uuid === row);
		assert(owner);
		const fields = await post('/api/ui/snapshot', {
			scope: { subtree: { root: owner.node_id, max_depth: 2 } }
		});
		const stop = fields.nodes.find((node) => node.decl_id === 'parameters/stop');
		assert(stop);
		assert(
			(
				await post('/api/ui/intent', {
					kind: 'setParam',
					node: stop.node_id,
					value: { Trigger: [] },
					behaviour: 'Append'
				})
			).success
		);
		await wait(
			async () => (await matrix({ kind: 'InspectPlayback' }, second)).snapshot,
			(value) => value.layers.some((request) => request.layer === row && request.column === null),
			'committed Layer Stop'
		);
		if (index === 0) {
			assert((await state()).playing, 'the other Layer still owns the Video clock');
			await capture('one-shared-layer-stopped', 3);
		}
	}
	await wait(state, (value) => value.settings.mode === 'Stopped', 'generic Stop Parameter');
	await route(source);
	await capture('last-shared-stop', 0);

	phase = 'independent-copy-and-project';
	const copyMatrix = await create('Matrix', 'Independent Video copy');
	const copyShape = await shape(copyMatrix);
	const copied = await assign(
		copyMatrix,
		copyShape.rows[0],
		copyShape.columns[0],
		source.media,
		true
	);
	assert(copied.media, 'owned Video copy receipt must identify the copy');
	const copiedChannels = (
		await edit({ kind: 'InspectChannels', query: { media: copied.media, start: 0, count: 1 } })
	).page.channels;
	const copy = copiedChannels[0].reference;
	assert.notEqual(copy.media, source.media);
	assert.equal((await state(copy.media)).settings.on_activate, 'Resume');
	assert.equal((await state(copy.media)).settings.on_deactivate, 'Stop');
	await route(copy);
	await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference: copy })).status,
		(value) => value.state === 'Ready',
		'copy decoder'
	);
	await video({ kind: 'Seek', position_ms: 125 }, copy.media);
	await launch(copyShape.columns[0], 'Cut', copyMatrix);
	assert.equal((await state(copy.media)).position_ms, 125);
	assert.equal((await state()).position_ms, 0);
	await capture('independent-copy', 3);
	const project = path.join(native.directory, 'video-matrix-policy.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	const loaded = await post('/api/ui/project-load', { path: project });
	assert.equal(path.resolve(loaded.path), path.resolve(project));
	assert.equal((await state()).settings.on_activate, 'Resume');
	assert.equal((await state(copy.media)).settings.on_deactivate, 'Stop');
	assert.equal((await state(copy.media)).position_ms, 0, 'runtime playhead is not project state');
	await route(copyMatrix);
	await launch(copyShape.columns[0], 'Cut', copyMatrix);
	await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference: copy })).status,
		(value) => value.state === 'Ready',
		'reloaded copy'
	);
	await capture('loaded-copy-policy', 0);
	await matrix({ kind: 'ClearPlayback' }, copyMatrix);
	await wait(
		async () => state(copy.media),
		(value) => value.settings.mode === 'Stopped',
		'loaded outgoing Stop'
	);
	assert((await post('/api/ui/project-new', {})).ok);
	const fresh = (await edit({ kind: 'InspectManager' }, 'screens')).snapshot;
	assert.equal(fresh.screens.length, 1, 'New preserves the accepted default Screen contract');
	for (const item of fresh.screens)
		await edit({ kind: 'RemoveScreen', screen: item.id }, 'screens');
	const retired = await wait(
		async () => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) =>
			!value.error &&
			value.screens.length === 0 &&
			(!value.resources ||
				(value.resources.used_bytes === 0 &&
					value.resources.in_flight === 0 &&
					value.resources.retiring === 0)),
		'final native retirement'
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	report = {
		result: 'pass',
		pid: native.app.pid,
		adapter,
		hardware,
		libraries,
		phases,
		observations,
		requests,
		uiReceipts: observed,
		errors,
		rejected,
		projectReload: true,
		independentCopy: true,
		matrixEnableVerified: true,
		freshDefaultsRemoved: fresh.screens.map((item) => item.id),
		finalRetirement: retired,
		normalShutdown: false,
		appSha256: createHash('sha256')
			.update(
				await readFile(
					path.resolve(process.env.MAPGYVER_TEST_BINARY ?? 'target/debug/MapGyver.exe')
				)
			)
			.digest('hex')
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				phase,
				error: String(error),
				errors,
				rejected,
				phases,
				observations,
				log: native.log(),
				body: await page
					.locator('body')
					.innerText()
					.catch(() => '')
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
report.normalShutdown = true;
try {
	// Policy correctness accepts resident decode; upload budgets belong to M10/4.
	report.nativeDiagnostics = await inspectNativeDiagnostics(native.app.pid, native.log(), {
		requireSoftwareUpload: false,
		requireVideoFrames: true
	});
} catch (error) {
	await writeFile(
		path.join(directory, 'diagnostic-failure.json'),
		JSON.stringify({ ...report, result: 'failed', error: String(error) }, null, 2)
	);
	throw error;
}
await writeFile(path.join(directory, 'matrix-video-policy.json'), JSON.stringify(report, null, 2));
console.log(
	JSON.stringify({
		result: report.result,
		adapter: report.adapter,
		pairs: phases.length,
		pixels: observations.length,
		normalShutdown: true
	})
);
