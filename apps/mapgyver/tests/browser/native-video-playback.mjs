// Bundled app controls, public authoring and independent composited Screen pixels.
// Build MapGyver with video-files and put the pinned FFmpeg DLL directory on PATH.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

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
					path.resolve('target/debug/native_qualification-m1110.exe'),
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
for (const stream of [native.app.stdout, native.app.stderr]) {
	stream.on('data', (data) => {
		playbackLog = (playbackLog + data).slice(-131072);
	});
}
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const { rejected, observed } = await observeNativeRejectedRequests(page, () => 'Video playback');
const observations = [];
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
async function seek(reference, position_ms, index, phase) {
	await transport(reference.media, { kind: 'Seek', position_ms });
	await wait(
		() => frame(reference),
		(value) => value && Math.abs(value.position_ms - position_ms) < 0.0001,
		`${phase} decoder timeline`
	);
	await pixels(phase, index);
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
		['prores-alpha.mov', 25, 'alpha']
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
			rate: 1,
			looping: false
		});
		await route(reference);
		await wait(
			() => sourceState(reference),
			(value) => value.state === 'Ready',
			`${name} decoded`
		);
		await checkPixels(`${name} initial`, 0);
		await parameter('Playback').locator('select').selectOption('Playing');
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
		await parameter('Playback').locator('select').selectOption('Stopped');
		await wait(
			() => state(reference.media),
			(value) => value.settings.mode === 'Stopped' && value.position_ms === 0,
			`${name} UI Stop`
		);
		await checkPixels(`${name} Stop`, 0);
		const seekInput = parameter('Seek to').locator('input.time-field');
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
			(value) => value.settings.position_ms === 0,
			`${name} Undo seek`
		);
		await checkPixels(`${name} Undo seek`, 0);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => state(reference.media),
			(value) => value.settings.position_ms === 125,
			`${name} Redo seek`
		);
		await checkPixels(`${name} Redo seek`, Math.floor(0.125 * rate));
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
	const alpha = codecs.find((codec) => codec.profile === 'alpha').reference;
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
	assert.notEqual(copied.media, alpha.media);
	assert.equal((await state(copied.media)).settings.position_ms, 0);
	await route(composition);
	await wait(
		() => sourceState(copied),
		(value) => value.state === 'Ready',
		'copied alpha File'
	);
	await transport(copied.media, { kind: 'Seek', position_ms: 40 });
	await pixels('Composition independent alpha copy', 1, 'alpha');
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
	await transport(alpha.media, { kind: 'Seek', position_ms: 120 });
	await route(matrix);
	await pixels('Matrix shared alpha source', 3, 'alpha');
	const routing = { composition, copied, matrix, alpha };
	const first = codecs[0].reference;
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
	await seek(first, 125, 3, 'Reference mirrors shared seek');
	await transport(first.media, { kind: 'Rate', rate: 0.5 });
	await transport(first.media, { kind: 'Loop', looping: true });
	await transport(first.media, { kind: 'Play' });
	const moving = [];
	for (let i = 0; i < 8; i++) {
		await delay(65);
		moving.push({ transport: await state(first.media), windows: windows() });
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
	await seek(first, 125, 3, 'Reference paused exact seek');
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
	await seek(first, 125, 3, 'Longer shared-clock Reference after variant hold');
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
		position_ms: 125,
		rate: 0.5,
		looping: true
	});
	await pixels('Loaded Reference output', 3);
	assert.equal((await state(copied.media)).settings.position_ms, 40);
	await route(composition);
	await pixels('Loaded Composition independent alpha copy', 1, 'alpha');
	await matrixEdit({ kind: 'LaunchColumn', column, mode: 'Cut' });
	await route(matrix);
	await pixels('Loaded Matrix shared alpha source', 3, 'alpha');
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	report = {
		result: 'pass',
		pid: native.app.pid,
		adapter: runtime.gpu_adapter,
		gpu: runtime.gpu,
		codecs,
		routing,
		mixedRateVariant: variant,
		observations,
		moving,
		pageErrors: errors,
		rejected,
		uiReceipts: observed,
		projectReload: true,
		backendEvents: playbackLog.split(/\r?\n/).filter((line) => line.includes('[mapgyver-video]')),
		normalShutdown: false,
		scope:
			'Seven SDR common-codec profiles, chromatic 8/10-bit and ProRes4444 alpha, exact controls, independent Composition copy and shared Matrix/Reference routing with project reload. Backend events distinguish hardware from software fallback. Not sustained performance, arbitrary HDR/profile support or combined device recovery'
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				error: String(error),
				observations,
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
