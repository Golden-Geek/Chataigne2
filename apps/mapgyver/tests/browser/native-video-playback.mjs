// Bundled app controls, public authoring and independent composited Screen pixels.
// Use the default bundled Video build; optional local-DLL checks verify the loaded pin.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, writeFile } from 'node:fs/promises';
import { StringDecoder } from 'node:string_decoder';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';
import { inspectLocalVideoLibraries } from './video-runtime-libraries.mjs';
import { inspectNativeDiagnostics } from './native-diagnostics.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-video-app-${Date.now()}`);
await mkdir(directory, { recursive: true });
const selectedAdapter = process.env.MAPGYVER_VIDEO_ADAPTER;
if (selectedAdapter) assert(/^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(selectedAdapter));
const native = await openNativeEditor({
	prepareProfile: selectedAdapter
		? async (root) => {
				const preferences = path.join(root, 'MapGyver');
				await mkdir(preferences, { recursive: true });
				execFileSync(
					path.resolve('target/debug/native_qualification.exe'),
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(preferences, 'preferences.json'),
						'--selected',
						selectedAdapter
					],
					{ windowsHide: true }
				);
			}
		: undefined
});
const { page } = native;
let playbackLog = '';
const playbackEvents = [];
for (const stream of [native.app.stdout, native.app.stderr]) {
	const decoder = new StringDecoder('utf8');
	let pending = '';
	stream.on('data', (data) => {
		playbackLog = (playbackLog + data).slice(-131072);
		const lines = (pending + decoder.write(data)).split(/\r?\n/);
		pending = lines.pop().slice(-16384);
		for (const line of lines) {
			if (!line.includes('[mapgyver-video]')) continue;
			playbackEvents.push(line);
			if (playbackEvents.length > 512) playbackEvents.shift();
		}
	});
}
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const { rejected, observed } = await observeNativeRejectedRequests(page, () => 'Video playback');
const observations = [];
const moving = [];
const seekBursts = [];
const fileRecovery = [];
let localRuntimeLibraries = null;
const fixtures = path.resolve('crates/golden_video/src/decode/tests/fixtures');
let token, screen, surface, surfaceDefinition;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
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
async function edit(payload, namespace = 'media') {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
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
async function wait(read, predicate, label) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(40);
	}
}
const state = async (media) => (await edit({ kind: 'InspectVideo', media })).snapshot;
const transport = async (media, operation) =>
	(await edit({ kind: 'Video', media, edit: operation })).snapshot;
const sourceState = async (reference) =>
	(await edit({ kind: 'InspectVideoSource', reference })).status;
const frame = async (reference) => (await edit({ kind: 'InspectVideoFrame', reference })).frame;
const parameter = (label) =>
	page.locator('.parameter-inspector').filter({
		has: page.getByText(label, { exact: true })
	});
const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
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
function windows() {
	return JSON.parse(
		execFileSync(
			'python',
			['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid)],
			{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
		)
	);
}
function expectedLinear(index, profile = 'gray') {
	if (profile.startsWith('hap')) {
		const alpha = profile === 'hap-alpha' ? index / 5 : 1;
		let rgb;
		if (profile === 'hap-q') {
			const y = (32 + index * 24) / 255;
			rgb = [y + 2 / 255, y + 2 / 255, y - 6 / 255];
		} else {
			const r = 4 + index * 3,
				g = 8 + index * 6;
			rgb = [((r << 3) | (r >> 2)) / 255, ((g << 2) | (g >> 4)) / 255, ((r << 3) | (r >> 2)) / 255];
		}
		return rgb.map((v) => alpha * (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
	}
	// Independent generator YUV/matrix/transfer/straight-alpha oracle, over black.
	const y = (16 + index * 24) / 219;
	const cb = profile === 'gray' ? 0 : -32 / 224;
	const cr = profile === 'gray' ? 0 : 32 / 224;
	const alpha = profile === 'alpha' ? Math.min(1, (index * 256) / 1023) : 1;
	return [y + 1.5748 * cr, y - 0.187324 * cb - 0.468124 * cr, y + 1.8556 * cb].map((value) => {
		const encoded = Math.max(0, Math.min(1, value));
		return alpha * (encoded < 0.081 ? encoded / 4.5 : ((encoded + 0.099) / 1.099) ** (1 / 0.45));
	});
}
function displayLinear(value) {
	const encoded = value / 255;
	return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4;
}
async function pixels(phase, index, profile = 'gray') {
	const expected = expectedLinear(index, profile);
	const found = await wait(
		async () => windows(),
		(all) =>
			all.length === 1 &&
			all[0].visible &&
			!all[0].minimized &&
			all[0].center_rgb.length &&
			all[0].center_rgb.every((rgb) =>
				rgb.every((v, component) => Math.abs(displayLinear(v) - expected[component]) <= 4 / 255)
			),
		phase
	);
	observations.push({
		phase,
		frame: index,
		expectedLinear: expected,
		linearTolerance: 4 / 255,
		...found[0]
	});
}
async function seek(reference, position_ms, index, phase, profile = 'gray') {
	await transport(reference.media, { kind: 'Seek', position_ms });
	await wait(
		() => frame(reference),
		(value) => value && Math.abs(value.position_ms - position_ms) < 0.0001,
		`${phase} decoder timeline`
	);
	await pixels(phase, index, profile);
}
let report;
try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	await tab('Medias').waitFor();
	const runtime = await wait(
		() => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) => value.state === 'Running' && value.gpu_adapter,
		'native renderer'
	);
	const displays = await control({ kind: 'mapgyverDisplays', refresh: true });
	localRuntimeLibraries =
		process.env.MAPGYVER_VIDEO_LOCAL_DLLS === '1'
			? await inspectLocalVideoLibraries(
					native.app.pid,
					process.env.MAPGYVER_TEST_BINARY,
					process.env.MAPGYVER_VIDEO_RUNTIME_PIN
				)
			: null;
	if (selectedAdapter) assert.equal(runtime.gpu_adapter, selectedAdapter);
	assert(displays.displays.length >= 2, 'Physical Screen check uses the external display');
	const display = displays.displays.find((display) => !display.primary) ?? displays.displays[1];
	const created = await edit(
		{ kind: 'CreateScreen', label: 'Video integration output' },
		'screens'
	);
	({ screen, surface } = created);
	surfaceDefinition = (await edit({ kind: 'InspectSurface', reference: surface }, 'screens'))
		.snapshot.definition;
	const bounds = {
		left: display.bounds.left + 80,
		top: display.bounds.top + 120,
		extent: { width: 320, height: 240 }
	};
	await edit(
		{
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled: true, target: { kind: 'Rectangle', bounds } }
		},
		'screens'
	);
	const codecs = [];
	for (const [name, rate, profile = 'gray'] of [
		['h264.mp4', 24],
		['hevc.mp4', 30000 / 1001],
		['hevc-main.mp4', 30000 / 1001],
		['prores.mov', 25],
		['h264-color.mp4', 24, 'color'],
		['hevc-main10.mp4', 30000 / 1001, 'color'],
		['prores-alpha.mov', 25, 'alpha'],
		['hap.mov', 24, 'hap'],
		['hap-alpha.mov', 25, 'hap-alpha'],
		['hap-q.mov', 30000 / 1001, 'hap-q']
	]) {
		const checkPixels = (phase, index) => pixels(phase, index, profile);
		const label = `Video integration ${name}`;
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label });
		await select(label);
		const file = parameter('File').locator('input.path-input');
		await file.fill(path.join(fixtures, name));
		await file.press('Tab');
		await wait(
			() => sourceState(reference),
			(value) => value.duration_ms != null && !value.error,
			`${name} File metadata`
		);
		assert.deepEqual((await state(reference.media)).settings, {
			mode: 'Paused',
			position_ms: 0,
			speed: 1,
			looping: false,
			play_at_load: false
		});
		const speedInput = parameter('Speed').locator('input.number-field');
		for (const speed of [0.5, 1]) {
			await speedInput.fill(String(speed));
			await speedInput.press('Enter');
			await wait(
				() => state(reference.media),
				(value) => value.settings.speed === speed,
				`${name} Inspector Speed ${speed}`
			);
		}
		await route(reference);
		await wait(
			() => sourceState(reference),
			(value) => value.state === 'Ready',
			`${name} decoded`
		);
		await checkPixels(`${name} initial`, 0);
		await parameter('Play').getByRole('button', { name: 'Trigger', exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.mode === 'Playing',
			`${name} UI Play`
		);
		await wait(
			() => state(reference.media),
			(value) => !value.playing,
			`${name} EOF`
		);
		await checkPixels(`${name} EOF hold`, 5);
		await parameter('Stop').getByRole('button', { name: 'Trigger', exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.mode === 'Stopped' && value.position_ms === 0,
			`${name} UI Stop`
		);
		await checkPixels(`${name} Stop`, 0);
		await speedInput.fill('0.5');
		await speedInput.press('Enter');
		await wait(
			() => state(reference.media),
			(value) => value.settings.speed === 0.5,
			`${name} authored Speed before seek`
		);
		const seekInput = parameter('Current Position').locator('input.time-field');
		await seekInput.fill('00:00:00.125');
		await seekInput.press('Enter');
		await wait(
			() => state(reference.media),
			(value) => value.settings.position_ms === 125 && value.settings.mode === 'Paused',
			`${name} UI seek`
		);
		await checkPixels(`${name} UI seek`, Math.floor(0.125 * rate));
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.speed === 1 && value.settings.position_ms === 125,
			`${name} Undo Speed preserves runtime position`
		);
		await checkPixels(`${name} Undo Speed preserves seek`, Math.floor(0.125 * rate));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.speed === 0.5 && value.settings.position_ms === 125,
			`${name} Redo Speed preserves runtime position`
		);
		await checkPixels(`${name} Redo Speed preserves seek`, Math.floor(0.125 * rate));
		await speedInput.fill('1');
		await speedInput.press('Enter');
		await wait(
			() => state(reference.media),
			(value) => value.settings.speed === 1,
			`${name} restore Speed`
		);
		await transport(reference.media, { kind: 'Stop' });
		await wait(
			() => frame(reference),
			(value) => value?.position_ms === 0 && value.next_ms != null,
			`${name} next-frame prepared`
		);
		await page.getByRole('button', { name: 'Next Video frame', exact: true }).click();
		const stepped = await wait(
			() => state(reference.media),
			(value) => value.settings.position_ms > 0 && value.settings.mode === 'Paused',
			`${name} UI Next`
		);
		assert(Math.abs(stepped.settings.position_ms - 1000 / rate) < 0.002);
		await checkPixels(`${name} UI Next`, 1);
		await wait(
			() => frame(reference),
			(value) => value?.previous_ready,
			`${name} previous-frame prepared`
		);
		await page.getByRole('button', { name: 'Previous Video frame', exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.position_ms === 0,
			`${name} UI Previous`
		);
		await checkPixels(`${name} UI Previous`, 0);
		const started = performance.now();
		for (const position_ms of [180, 0, 160, 40, 120, 80, 200, 0, 125]) {
			await transport(reference.media, { kind: 'Seek', position_ms });
		}
		const finalIndex = Math.floor(0.125 * rate);
		await wait(
			() => frame(reference),
			(value) => value && Math.abs(value.position_ms - (finalIndex * 1000) / rate) < 0.002,
			`${name} latest burst seek`
		);
		const acknowledgedMs = performance.now() - started;
		await checkPixels(`${name} superseding seek`, finalIndex);
		await delay(230);
		await checkPixels(`${name} superseding seek held`, finalIndex);
		seekBursts.push({ name, requests: 9, finalPositionMs: 125, finalIndex, acknowledgedMs });
		await transport(reference.media, { kind: 'Stop' });
		await tab('Media Preview').click();
		await page
			.locator(
				'.media-preview-panel .native-preview[data-preview-state="available"]' +
					'[data-preview-installed="true"]'
			)
			.waitFor();
		codecs.push({
			name,
			rate,
			profile,
			reference,
			steppedTimestampMs: stepped.settings.position_ms
		});
	}
	for (const name of ['h264-color.mp4', 'hevc-main10.mp4', 'prores-alpha.mov', 'hap-alpha.mov']) {
		const codec = codecs.find((codec) => codec.name === name);
		const { reference, profile, rate } = codec;
		await route(reference);
		const recoveredPath = path.join(directory, `recovered-${name}`);
		await edit({ kind: 'ConfigureVideoFile', reference, file: recoveredPath });
		const missing = await wait(
			() => sourceState(reference),
			(value) => value.error?.kind === 'Missing' && value.retrying,
			`${name} missing/retry`
		);
		await writeFile(recoveredPath, 'not a Video file');
		await wait(
			() => sourceState(reference),
			(value) => value.error?.kind === 'Decode' && value.retrying,
			`${name} corrupt/retry`
		);
		await copyFile(path.join(fixtures, name), recoveredPath);
		await wait(
			() => sourceState(reference),
			(value) => value.state === 'Ready' && !value.error && !value.retrying,
			`${name} same-path recovery`
		);
		await seek(reference, (3 * 1000) / rate, 3, `${name} recovered exact frame`, profile);
		await edit({ kind: 'ConfigureVideoFile', reference, file: path.join(fixtures, name) });
		await wait(
			() => sourceState(reference),
			(value) => value.state === 'Ready' && !value.error,
			`${name} authored relink`
		);
		assert.equal(
			(await state(reference.media)).position_ms,
			0,
			'a replacement File resets unknown duration'
		);
		await seek(
			reference,
			(3 * 1000) / rate,
			3,
			`${name} authored relink accepts a fresh seek`,
			profile
		);
		await transport(reference.media, { kind: 'Stop' });
		fileRecovery.push({ name, missing: missing.state, retryRecovered: true, relinked: true });
	}
	const alpha = codecs.find((codec) => codec.profile === 'hap-alpha').reference;
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Video alpha composition'
	});
	const dropped = await edit({
		kind: 'Composition',
		reference: composition,
		edit: {
			kind: 'DropSource',
			source: alpha.media,
			instance: null,
			copy: true,
			position: { x: 960, y: 540 }
		}
	});
	const copied = (
		await edit({
			kind: 'InspectChannels',
			query: { media: dropped.instances[0].instance, start: 0, count: 1 }
		})
	).page.channels[0].reference;
	const owned = (
		await edit({ kind: 'Composition', reference: composition, edit: { kind: 'Inspect' } })
	).snapshot.instances.find((instance) => instance.reference.instance === copied.media);
	assert(owned);
	// The odd 17x9 fixture must cover the physical sample area; dark alpha cannot prove pixels.
	await edit({
		kind: 'Composition',
		reference: composition,
		edit: {
			kind: 'ConfigureInstance',
			instance: owned.reference.instance,
			definition: {
				...owned.definition,
				transform: { ...owned.definition.transform, scale: [64, 64] }
			}
		}
	});
	assert.notEqual(copied.media, alpha.media);
	assert.equal((await state(copied.media)).settings.position_ms, 0);
	await route(composition);
	await wait(
		() => sourceState(copied),
		(value) => value.state === 'Ready',
		'copied alpha File'
	);
	await transport(copied.media, { kind: 'Seek', position_ms: 120 });
	await pixels('Composition independent HAP alpha copy', 3, 'hap-alpha');
	assert.equal((await state(alpha.media)).position_ms, 0, 'copy owns an independent clock');
	const matrix = (
		await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Video matrix route' })
	).reference;
	const matrixEdit = (operation) => edit({ kind: 'Matrix', media: matrix.media, edit: operation });
	const matrixState = (await matrixEdit({ kind: 'Inspect' })).snapshot;
	const row = matrixState.items.find((item) => item.id.kind === 'Layer').id.id;
	const column = matrixState.columns[0].id;
	await matrixEdit({ kind: 'DropSource', layer: row, column, source: alpha.media, copy: false });
	await matrixEdit({
		kind: 'ConfigureCell',
		layer: row,
		column,
		settings: {
			position: [0, 0],
			size: { width: 1920, height: 1080 },
			fit_mode: 'Fit',
			opacity: 1,
			enabled: true
		}
	});
	await matrixEdit({ kind: 'LaunchColumn', column, mode: 'Cut' });
	await transport(alpha.media, { kind: 'Seek', position_ms: 160 });
	await route(matrix);
	await pixels('Matrix shared HAP alpha source', 4, 'hap-alpha');
	const routing = { composition, copied, matrix, alpha };
	const first = codecs.find((codec) => codec.profile === 'hap').reference;
	const { media: mirror } = await edit({
		kind: 'CreateReference',
		target: first.media,
		label: 'Video mirror'
	});
	const mirrored = (
		await edit({ kind: 'InspectChannels', query: { media: mirror, start: 0, count: 1 } })
	).page.channels;
	assert.equal(mirrored.length, 1);
	assert.deepEqual(mirrored[0].definition.source, { kind: 'Reference', target: first });
	await route(mirrored[0].reference);
	await seek(first, 125, 3, 'HAP Reference mirrors shared seek', 'hap');
	await transport(first.media, { kind: 'Speed', speed: 0.5 });
	await transport(first.media, { kind: 'Loop', looping: true });
	await transport(first.media, { kind: 'Play' });
	// A fixed observer delay can alias the six-frame loop, especially with process-start cost.
	for (const delayMs of [17, 53, 89, 127, 173, 211, 257, 307]) {
		await delay(delayMs);
		moving.push({
			observedAtMs: Date.now(),
			delayMs,
			transport: await state(first.media),
			windows: windows()
		});
	}
	assert(moving.every((sample) => sample.transport.playing));
	assert(new Set(moving.map((sample) => Math.round(sample.transport.position_ms))).size >= 4);
	assert(
		new Set(moving.map((sample) => JSON.stringify(sample.windows[0]?.center_rgb))).size >= 3,
		'Playing shared Reference must change actual Screen pixels'
	);
	await transport(first.media, { kind: 'Pause' });
	const paused = await state(first.media);
	await delay(100);
	assert.equal((await state(first.media)).position_ms, paused.position_ms);
	await seek(first, 125, 3, 'HAP Reference paused exact seek', 'hap');
	const variant = (
		await edit({ kind: 'AddChannel', media: first.media, label: 'Shorter mixed-rate HEVC10' })
	).reference;
	await edit({
		kind: 'ConfigureVideoFile',
		reference: variant,
		file: path.join(fixtures, 'hevc-main10.mp4')
	});
	await route(variant);
	await wait(
		() => sourceState(variant),
		(value) => value.state === 'Ready',
		'shorter mixed-rate variant'
	);
	await transport(first.media, { kind: 'Seek', position_ms: 230 });
	await pixels('Shorter mixed-rate variant holds final frame', 5, 'color');
	assert.equal((await state(first.media)).position_ms, 230);
	await route(mirrored[0].reference);
	await seek(first, 125, 3, 'Longer HAP shared-clock Reference after variant hold', 'hap');
	const project = path.join(native.directory, 'video-playback.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await wait(
		async () => windows(),
		(all) => !all.length,
		'New retires Video output'
	);
	await post('/api/ui/project-load', { path: project });
	await wait(
		() => sourceState(first),
		(value) => value.state === 'Ready',
		'Loaded File automatically decodes'
	);
	assert.deepEqual((await state(first.media)).settings, {
		mode: 'Paused',
		position_ms: 0,
		speed: 0.5,
		looping: true,
		play_at_load: false
	});
	await pixels('Loaded HAP Reference resets runtime position', 0, 'hap');
	assert.equal((await state(copied.media)).settings.position_ms, 0);
	assert.equal((await state(alpha.media)).settings.position_ms, 0);
	await transport(copied.media, { kind: 'Seek', position_ms: 120 });
	await route(composition);
	await pixels('Loaded Composition independent HAP alpha copy', 3, 'hap-alpha');
	assert.equal(
		(await state(alpha.media)).position_ms,
		0,
		'reloaded copy keeps an independent clock'
	);
	await matrixEdit({ kind: 'LaunchColumn', column, mode: 'Cut' });
	await transport(alpha.media, { kind: 'Seek', position_ms: 160 });
	await route(matrix);
	await pixels('Loaded Matrix shared HAP alpha source', 4, 'hap-alpha');
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	for (const name of ['h264-color.mp4', 'hevc-main.mp4', 'hevc-main10.mp4']) {
		assert(
			playbackEvents.some(
				(line) => line.includes('D3D11 resident playback:') && line.includes(name)
			),
			`${name} must retain the hardware path in the qualified source SDK`
		);
	}
	report = {
		result: 'pass',
		pid: native.app.pid,
		adapter: runtime.gpu_adapter,
		gpu: runtime.gpu,
		localRuntimeLibraries,
		codecs,
		routing,
		mixedRateVariant: variant,
		observations,
		moving,
		seekBursts,
		fileRecovery,
		pageErrors: errors,
		rejected,
		uiReceipts: observed,
		projectReload: true,
		projectRuntimePositionReset: true,
		backendEvents: playbackEvents,
		normalShutdown: false,
		scope:
			'Ten common/HAP profiles, chromatic 8/10-bit, ProRes4444/HAP alpha and YCoCg, superseding seeks, missing/corrupt-file retry/relink, exact controls, independent HAP Composition copy and shared HAP Matrix/Reference routing with project reload. Backend events distinguish hardware/BC upload from software fallback. Not sustained performance, high-resolution concurrency or combined device recovery'
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				error: String(error),
				observations,
				moving,
				seekBursts,
				fileRecovery,
				localRuntimeLibraries,
				backendEvents: playbackEvents,
				pageErrors: errors,
				rejected,
				log: native.log(),
				body: await page.locator('body').innerText()
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
report.nativeDiagnostics = await inspectNativeDiagnostics(native.app.pid, native.log());
await writeFile(path.join(directory, 'video-playback.json'), JSON.stringify(report, null, 2));
console.log(
	JSON.stringify({
		result: report.result,
		pid: report.pid,
		adapter: report.adapter,
		codecs: report.codecs.length,
		physicalCheckpoints: observations.length,
		projectReload: true,
		normalShutdown: true
	})
);
