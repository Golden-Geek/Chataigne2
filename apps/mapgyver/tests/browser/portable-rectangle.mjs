import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';

assert.equal(process.platform, 'win32', 'Select a desktop with the owned HWND pixel observer');
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the portable GPU MapGyver executable');
const native = await openNativeEditor({ allowPhysicalOutputs: true });
const inspectWindow = fileURLToPath(new URL('../native_output/window_state.py', import.meta.url));
let token;

async function post(route, payload) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'content-type': 'application/json',
			...(token ? { authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(payload),
		signal: AbortSignal.timeout(5000)
	});
	const outcome = await response.json();
	assert(response.ok, JSON.stringify(outcome));
	return outcome;
}

async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}

async function waitForOutput(accept) {
	const deadline = performance.now() + 12000;
	for (;;) {
		const { stdout } = await promisify(execFile)(
			'python',
			[inspectWindow, String(native.app.pid), '--title', 'MapGyver Screen Output'],
			{ windowsHide: true, timeout: 4000 }
		);
		const windows = JSON.parse(stdout);
		assert(windows.length < 2, `Duplicate output: ${stdout}`);
		if (accept(windows[0])) return windows[0];
		assert(performance.now() < deadline, `Rectangle Screen did not update: ${stdout}`);
		await delay(120);
	}
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit('media', { kind: 'CreateMedia', media_kind: 'SolidColor' });
	const color = (rgba) =>
		edit('media', { kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
	await color([1, 0, 0, 1]);
	const { screen, surface } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Portable Rectangle'
	});
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { source: reference }
	});
	const bounds = { left: 160, top: 120, extent: { width: 320, height: 240 } };
	const output = (enabled) =>
		edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled, target: { kind: 'Rectangle', bounds } }
		});
	await output(true);
	const pixels = (rgb) => (window) =>
		window?.visible &&
		window.center_rgb.every((pixel) =>
			pixel.every((value, index) => Math.abs(value - rgb[index]) < 16)
		);
	const red = await waitForOutput(pixels([255, 0, 0]));
	assert.deepEqual(red.bounds, bounds);
	await color([0, 1, 0, 1]);
	const green = await waitForOutput(pixels([0, 255, 0]));
	assert.equal(green.hwnd, red.hwnd);
	await output(false);
	await waitForOutput((window) => window === undefined);
	await output(true);
	const reopened = await waitForOutput(pixels([0, 255, 0]));
	assert.deepEqual(reopened.bounds, bounds);
	console.log(
		'Native Rectangle showed both colors, retired on disable and reopened at its authored bounds'
	);
} catch (error) {
	console.error(native.log().slice(-6000));
	throw error;
} finally {
	await native.close();
}
