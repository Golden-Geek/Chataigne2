import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { openInitialScreenDesktop } from '../headless/initial-screen-host.mjs';

const onMac = process.platform === 'darwin';
assert(onMac || process.platform === 'win32', 'Select a desktop with an owned-window observer');
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the portable GPU MapGyver executable');
const program = promisify(execFile);
const observer = (name) => fileURLToPath(new URL(`../native_output/${name}`, import.meta.url));
const directory = await mkdtemp(join(tmpdir(), 'mapgyver-portable-rectangle-'));
const macObserver = join(directory, 'macos-windows');
if (onMac) {
	await program(
		'swiftc',
		['-swift-version', '5', observer('macos_windows.swift'), '-o', macObserver],
		{
			timeout: 60000
		}
	);
}
process.env.MAPGYVER_AUTOMATION_DISABLE_DISPLAY_OUTPUTS = '0';
const native = await openInitialScreenDesktop(directory);
let token;
const existing = new Set();

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

async function waitForWindows(accept) {
	const deadline = performance.now() + 12000;
	for (;;) {
		const pid = String(native.app.pid);
		const { stdout } = await program(
			onMac ? macObserver : 'python',
			onMac ? [pid] : [observer('window_state.py'), pid, '--title', 'MapGyver Screen Output'],
			{ windowsHide: true, timeout: 4000 }
		);
		const windows = JSON.parse(stdout);
		if (accept(windows)) return windows;
		assert(performance.now() < deadline, `Rectangle Screen did not update: ${stdout}`);
		await delay(120);
	}
}

async function waitForOutput(accept) {
	const outputs = (windows) =>
		onMac
			? windows.filter(
					(window) => !existing.has(window.id) && window.layer >= 0 && window.height > 1
				)
			: windows;
	const windows = await waitForWindows((windows) => {
		const active = outputs(windows);
		assert(active.length < 2, `Duplicate output: ${JSON.stringify(active)}`);
		return accept(active[0]);
	});
	return outputs(windows)[0];
}

async function selectPrimaryDisplay(screen) {
	const deadline = performance.now() + 8000;
	for (;;) {
		const graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		const byId = new Map(graph.nodes.map((node) => [node.node_id, node]));
		const root = graph.nodes.find((node) => node.uuid === screen);
		assert(root, 'Screen is absent from the workbench snapshot');
		const pending = [root];
		while (pending.length) {
			const node = pending.pop();
			if (node.decl_id === 'parameters/display') {
				const param = node.data.param;
				const definition = graph.schema.enums?.find(
					(item) => item.enum_id === param.enum_options_id
				);
				const options = definition?.variants ?? param.constraints?.enum_options ?? [];
				const display = options.find(({ variant_id }) =>
					variant_id.startsWith('macos:')
				)?.variant_id;
				if (display) {
					assert.match(display, /^macos:[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i);
					return display;
				}
				break;
			}
			pending.push(...(node.children ?? []).map((id) => byId.get(id)));
		}
		assert(performance.now() < deadline, 'The Screen Inspector received no macOS displays');
		await delay(120);
	}
}

try {
	if (onMac) {
		const baseline = await waitForWindows((windows) =>
			windows.some((window) => window.layer === 0 && window.height >= 100)
		);
		for (const window of baseline) existing.add(window.id);
	}
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
	const expectedOutput = (rgb) => (window) =>
		window &&
		(onMac
			? Math.abs(window.width / window.height - 4 / 3) < 0.01
			: window.visible &&
				window.center_rgb.every((pixel) =>
					pixel.every((value, index) => Math.abs(value - rgb[index]) < 16)
				));
	const red = await waitForOutput(expectedOutput([255, 0, 0]));
	if (!onMac) assert.deepEqual(red.bounds, bounds);
	await color([0, 1, 0, 1]);
	const green = await waitForOutput(expectedOutput([0, 255, 0]));
	assert.equal(onMac ? green.id : green.hwnd, onMac ? red.id : red.hwnd);
	await output(false);
	await waitForOutput((window) => window === undefined);
	await output(true);
	const reopened = await waitForOutput(expectedOutput([0, 255, 0]));
	if (!onMac) assert.deepEqual(reopened.bounds, bounds);
	console.log(
		onMac
			? 'WindowServer reported the Rectangle across live edit, disable and reopen'
			: 'Native Rectangle showed both colors, retired on disable and reopened at its authored bounds'
	);
	if (onMac) {
		await output(false);
		await waitForOutput((window) => window === undefined);
		const display = await selectPrimaryDisplay(screen);
		const fullscreen = (target) =>
			edit('screens', {
				kind: 'ConfigureScreen',
				screen,
				definition: { enabled: true, target }
			});
		const selected = { kind: 'Fullscreen', display };
		const { stdout } = await program(macObserver, ['--primary-display'], { timeout: 4000 });
		const expected = JSON.parse(stdout);
		const coversDisplay = (window) =>
			window &&
			Object.entries(expected).every(([key, value]) => Math.abs(window[key] - value) <= 1);
		await fullscreen(selected);
		const initial = await waitForOutput(coversDisplay);
		await color([0, 0, 1, 1]);
		assert.equal((await waitForOutput(coversDisplay)).id, initial.id);
		await fullscreen(null);
		await waitForOutput((window) => window === undefined);
		await fullscreen(selected);
		await waitForOutput(coversDisplay);
		console.log('The Inspector UUID opened Fullscreen at the selected macOS display bounds');
	}
} catch (error) {
	console.error(native.log().slice(-6000));
	throw error;
} finally {
	await native.close();
}
