import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual, promisify } from 'node:util';
import { openInitialScreenDesktop } from '../headless/initial-screen-host.mjs';
import {
	linuxOutputCenter,
	linuxOutputWindows,
	linuxRootBounds
} from '../native_output/linux_x11_windows.mjs';
import { westonOutputs } from '../native_output/weston_x11_outputs.mjs';

const onMac = process.platform === 'darwin';
const onLinux = process.platform === 'linux';
const onWayland = onLinux && process.argv.includes('--wayland');
assert(
	onMac || onLinux || process.platform === 'win32',
	'Select a desktop with an owned-window observer'
);
if (onLinux)
	assert(process.env.DISPLAY, 'The native Linux Screen fixture requires X11 observation');
if (onWayland) assert.equal(process.env.GDK_BACKEND, 'wayland');
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the portable GPU MapGyver executable');
const program = promisify(execFile);
const observer = (name) => fileURLToPath(new URL(`../native_output/${name}`, import.meta.url));
const directory = await mkdtemp(join(tmpdir(), 'mapgyver-portable-rectangle-'));
const macObserver = join(directory, 'macos-windows');
const macCapture = join(directory, 'output.png');
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
const compositorOutputs = onWayland ? await westonOutputs() : null;
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
		const windows = await observedWindows();
		if (await accept(windows)) return windows;
		assert(
			performance.now() < deadline,
			`Native Screen did not update: ${JSON.stringify(windows)}`
		);
		await delay(120);
	}
}

async function observedWindows() {
	if (onLinux) return linuxOutputWindows(native.app.pid);
	const pid = String(native.app.pid);
	const { stdout } = await program(
		onMac ? macObserver : 'python',
		onMac ? [pid] : [observer('window_state.py'), pid, '--title', 'MapGyver Screen Output'],
		{ windowsHide: true, timeout: 4000 }
	);
	return JSON.parse(stdout);
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

function matchesRgb(actual, expected) {
	return (
		actual.length === expected.length &&
		actual.every((value, index) => Math.abs(value - expected[index]) < 16)
	);
}

async function waitForMacOutput(geometry, rgb) {
	let actual;
	try {
		return await waitForOutput(async (window) => {
			if (!geometry(window)) return false;
			await program('/usr/sbin/screencapture', ['-x', '-o', '-l', String(window.id), macCapture], {
				timeout: 4000
			});
			const { stdout } = await program(macObserver, ['--sample-center', macCapture], {
				timeout: 4000
			});
			actual = JSON.parse(stdout);
			return matchesRgb(actual, rgb);
		});
	} catch (error) {
		throw new Error(`Window capture expected RGB ${rgb}, last sampled ${actual ?? 'none'}`, {
			cause: error
		});
	}
}

async function waitForWindowPixels(geometry, rgb) {
	if (onMac) return waitForMacOutput(geometry, rgb);
	let actual;
	try {
		return await waitForOutput(async (window) => {
			if (!window?.visible || !geometry(window)) return false;
			const pixels = onLinux ? await linuxOutputCenter(window) : window.center_rgb;
			actual = pixels[0] ? Array.from(pixels[0]) : undefined;
			return pixels.length > 0 && pixels.every((pixel) => matchesRgb(pixel, rgb));
		});
	} catch (error) {
		throw new Error(`Window capture expected RGB ${rgb}, last sampled ${actual ?? 'none'}`, {
			cause: error
		});
	}
}

async function selectDisplay(screen, prefix) {
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
				const display = options.find(({ variant_id }) => variant_id.startsWith(prefix))?.variant_id;
				if (display) {
					if (onMac) assert.match(display, /^macos:[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i);
					return display;
				}
				break;
			}
			pending.push(...(node.children ?? []).map((id) => byId.get(id)));
		}
		assert(performance.now() < deadline, `The Screen Inspector received no ${prefix} displays`);
		await delay(120);
	}
}

async function waitForWaylandPixels(expected) {
	const deadline = performance.now() + 12000;
	for (;;) {
		const outputs = await Promise.all(compositorOutputs.map(linuxOutputCenter));
		const colors = outputs.map((pixels) => pixels[0]);
		if (
			outputs.every(
				(pixels, index) =>
					pixels.length > 0 &&
					pixels.every(
						(pixel) => matchesRgb(pixel, expected[index].rgb) === expected[index].visible
					)
			)
		) {
			return;
		}
		assert(
			performance.now() < deadline,
			`Wayland displays did not match: ${JSON.stringify(colors)}`
		);
		await delay(120);
	}
}

async function testWaylandFullscreen(screen, color) {
	const [primary, secondary] = await Promise.all([
		selectDisplay(screen, 'wayland:screen0:'),
		selectDisplay(screen, 'wayland:screen1:')
	]);
	assert.notEqual(primary, secondary, 'Weston outputs must have distinct saved identities');
	const fullscreen = (display) =>
		edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled: true, target: display ? { kind: 'Fullscreen', display } : null }
		});
	const check = (rgb, primary, secondary) =>
		waitForWaylandPixels([
			{ rgb, visible: primary },
			{ rgb, visible: secondary }
		]);
	await fullscreen(secondary);
	await check([255, 0, 0], false, true);
	await color([0, 1, 0, 1]);
	const green = [0, 255, 0];
	await check(green, false, true);
	for (let attempt = 0; attempt < 4; attempt += 1) {
		await fullscreen(primary);
		await check(green, true, false);
		await fullscreen(secondary);
		await check(green, false, true);
	}
	await fullscreen(null);
	await check(green, false, false);
	await fullscreen(secondary);
	await check(green, false, true);
	console.log('Weston showed the authored Fullscreen pixels on each selected Wayland output');
}

async function fullscreenGeometry() {
	if (onLinux) {
		// Xvfb has one active output occupying the entire X11 root.
		const expected = await linuxRootBounds();
		return (window) => window && isDeepStrictEqual(window.bounds, expected);
	}
	const { stdout } = await program(macObserver, ['--primary-display'], { timeout: 4000 });
	const expected = JSON.parse(stdout);
	return (window) =>
		window && Object.entries(expected).every(([key, value]) => Math.abs(window[key] - value) <= 1);
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
	if (onWayland) {
		await testWaylandFullscreen(screen, color);
	} else {
		const bounds = { left: 160, top: 120, extent: { width: 320, height: 240 } };
		const output = (enabled) =>
			edit('screens', {
				kind: 'ConfigureScreen',
				screen,
				definition: { enabled, target: { kind: 'Rectangle', bounds } }
			});
		await output(true);
		const rectangle = (rgb) =>
			waitForWindowPixels(
				(window) => window && (!onMac || Math.abs(window.width / window.height - 4 / 3) < 0.01),
				rgb
			);
		const red = await rectangle([255, 0, 0]);
		if (!onMac) assert.deepEqual(red.bounds, bounds);
		await color([0, 1, 0, 1]);
		const green = await rectangle([0, 255, 0]);
		assert.equal(green.id ?? green.hwnd, red.id ?? red.hwnd);
		await output(false);
		await waitForOutput((window) => window === undefined);
		await output(true);
		const reopened = await rectangle([0, 255, 0]);
		if (!onMac) assert.deepEqual(reopened.bounds, bounds);
		console.log(
			onMac
				? 'WindowServer capture measured red and green Rectangle pixels across edit and reopen'
				: 'Native Rectangle showed both colors, retired on disable and reopened at its authored bounds'
		);
		if (onMac || onLinux) {
			await output(false);
			await waitForOutput((window) => window === undefined);
			const display = await selectDisplay(screen, onMac ? 'macos:' : 'x11:');
			const fullscreen = (target) =>
				edit('screens', {
					kind: 'ConfigureScreen',
					screen,
					definition: { enabled: true, target }
				});
			const selected = { kind: 'Fullscreen', display };
			const coversDisplay = await fullscreenGeometry();
			await fullscreen(selected);
			const initial = await waitForWindowPixels(coversDisplay, [0, 255, 0]);
			await color([0, 0, 1, 1]);
			assert.equal((await waitForWindowPixels(coversDisplay, [0, 0, 255])).id, initial.id);
			await fullscreen(null);
			await waitForOutput((window) => window === undefined);
			await fullscreen(selected);
			await waitForWindowPixels(coversDisplay, [0, 0, 255]);
			console.log(
				`${onMac ? 'WindowServer' : 'XRandR'} output measured green and blue Fullscreen pixels at the selected display bounds`
			);
		}
	}
} catch (error) {
	console.error(native.log().slice(-6000));
	throw error;
} finally {
	await native.close();
}
