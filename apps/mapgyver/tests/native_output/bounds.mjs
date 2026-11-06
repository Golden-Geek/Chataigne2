// Ordinary bundled app, public authoring/history and read-only owned native observations.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from '../browser/native-editor.mjs';
import { observeNativeAdapter } from '../browser/native-adapter.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-output-bounds-${Date.now()}`);
await mkdir(directory, { recursive: true });
const gpu = process.argv.find((value) => value.startsWith('--gpu='))?.slice('--gpu='.length);
assert(!gpu || /^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu), 'Exact GPU LUID required');
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = gpu
	? JSON.parse(
			execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
		)
	: undefined;
const native = await openNativeEditor({
	prepareProfile: gpu
		? async (root) => {
				const preferences = path.join(root, 'MapGyver');
				await mkdir(preferences, { recursive: true });
				execFileSync(
					observer,
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(preferences, 'preferences.json'),
						'--selected',
						gpu
					],
					{ windowsHide: true }
				);
			}
		: undefined
});
let activeAdapter;
const observations = [];
const outputEvents = [];
let outputBuffer = '';
native.app.stderr.on('data', (chunk) => {
	outputBuffer += chunk.toString();
	const lines = outputBuffer.split('\n');
	outputBuffer = lines.pop();
	for (const line of lines) if (line.startsWith('[mapgyver-output]')) outputEvents.push(line);
});
let token;
const errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function edit(namespace, payload) {
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
	native.page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
const windows = () =>
	JSON.parse(
		execFileSync(
			'python',
			['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid)],
			{ windowsHide: true, encoding: 'utf8', timeout: 5000 }
		)
	);
const creations = () =>
	outputEvents.filter((line) => line.startsWith('[mapgyver-output] window id=')).length;
const retirements = () =>
	outputEvents.filter((line) => line.includes('native session retired')).length;
let screen, identity;
const configure = (target) =>
	edit('screens', { kind: 'ConfigureScreen', screen, definition: { enabled: true, target } });
async function observe(phase, bounds) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const found = windows();
		assert(found.length <= 1, 'one Screen must never acquire overlapping native windows');
		const window = found[0];
		if (identity && window) {
			assert.equal(
				window.hwnd,
				identity,
				`${phase}: output HWND must survive: ${JSON.stringify(window)}`
			);
			assert(window.visible && !window.minimized, `${phase}: window must stay visible`);
		}
		if (
			window &&
			JSON.stringify(window.bounds) === JSON.stringify(bounds) &&
			window.center_rgb.every((rgb) => rgb[0] >= 250 && rgb[1] <= 2 && rgb[2] <= 2)
		) {
			identity ??= window.hwnd;
			const status = await control({ kind: 'mapgyverRuntimeStatus' });
			assert(!status.error, JSON.stringify(status));
			observations.push({ phase, ...window, retirements: retirements(), creations: creations() });
			return;
		}
		assert(
			Date.now() < deadline,
			JSON.stringify({ phase, expected: bounds, found, log: native.log() })
		);
		await delay(100);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.waitFor();
	let catalog;
	const deadline = Date.now() + 10000;
	do {
		catalog = await control({ kind: 'mapgyverDisplays', refresh: !catalog });
		assert(Date.now() < deadline, JSON.stringify(catalog));
		if (!catalog.displays.length) await delay(100);
	} while (!catalog.displays.length);
	assert(
		catalog.displays.length >= 2,
		'display reassignment acceptance requires both local displays'
	);
	const negativeOriginObserved = catalog.displays.some((display) => display.bounds.left < 0);
	if (process.argv.includes('--require-negative-origin'))
		assert(negativeOriginObserved, 'An actual negative monitor origin is required');
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Window bounds red'
	});
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const created = await edit('screens', { kind: 'CreateScreen', label: 'Stable bounds' });
	screen = created.screen;
	const { snapshot } = await edit('screens', {
		kind: 'InspectSurface',
		reference: created.surface
	});
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference: created.surface,
		definition: { ...snapshot.definition, source }
	});
	const origin = catalog.displays[0].bounds;
	const initial = {
		left: origin.left + 80,
		top: origin.top + 100,
		extent: { width: 320, height: 180 }
	};
	await configure({ kind: 'Rectangle', bounds: initial });
	await observe('initial', initial);
	if (gpu) {
		activeAdapter = await observeNativeAdapter(native.page, hardware);
		assert.equal(activeAdapter.key, gpu);
	}
	const beforeMove = retirements();
	const moved = { ...initial, left: initial.left + 140, top: initial.top + 60 };
	await configure({ kind: 'Rectangle', bounds: moved });
	await observe('position-only', moved);
	assert.equal(retirements(), beforeMove, 'position changes must preserve GPU session');
	const resized = { ...moved, extent: { width: 480, height: 270 } };
	await configure({ kind: 'Rectangle', bounds: resized });
	await observe('resize', resized);
	assert(retirements() > beforeMove, 'changed backing extent must cross actual GPU retirement');
	await native.page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await observe('undo-resize', moved);
	// Fullscreen and rectangle are policies over the same stable window, even across DPI.
	for (const [index, display] of catalog.displays.slice(0, 2).entries()) {
		await configure({ kind: 'Fullscreen', display: display.id });
		await observe(`fullscreen-display-${index}`, display.bounds);
	}
	const other = catalog.displays[1].bounds;
	const returnBounds = {
		left: other.left + 100,
		top: other.top + 100,
		extent: { width: 400, height: 225 }
	};
	await configure({ kind: 'Rectangle', bounds: returnBounds });
	await observe('return-rectangle', returnBounds);
	assert.equal(creations(), 1, 'all bounds/display changes retain the original window');
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: { enabled: false, target: { kind: 'Rectangle', bounds: returnBounds } }
	});
	const closeDeadline = Date.now() + 10000;
	while (windows().length) {
		assert(
			Date.now() < closeDeadline,
			'disabled output must close after actual renderer retirement'
		);
		await delay(100);
	}
	assert.deepEqual(errors, []);
	const report = {
		result: 'pass',
		pid: native.app.pid,
		identity,
		adapter: activeAdapter,
		negativeOriginObserved,
		displays: catalog.displays,
		observations,
		outputEvents,
		disabledWindowRetired: true,
		pageErrors: errors,
		scope:
			'actual native HWND/visibility/physical bounds and composited red pixel checkpoints; not continuous scanout or latency qualification'
	};
	await writeFile(path.join(directory, 'bounds.json'), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), observations, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
