// M05/4: ordinary bundled app + real WebView controls + physical desktop + independent SDK receiver.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from '../browser/native-editor.mjs';
import { createPreviewOutputObserver } from '../browser/preview-output-observer.mjs';

const directory = path.resolve(
	process.argv[2] ?? `target/mapgyver-concurrent-output-${Date.now()}`
);
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
let token, observer;
const checkpoints = [];
const surfaces = [];
const identities = new Map();
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
const windows = (main = false) =>
	JSON.parse(
		execFileSync(
			'python',
			[
				'apps/mapgyver/tests/native_output/window_state.py',
				String(native.app.pid),
				...(main ? ['--main'] : [])
			],
			{ windowsHide: true, encoding: 'utf8', timeout: 5000 }
		)
	);
async function windowState(minimized) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const found = windows(true);
		assert.equal(found.length, 1, JSON.stringify(found));
		if (found[0].minimized === minimized) return found[0];
		assert(
			Date.now() < deadline,
			`OS minimized=${minimized} not observed: ${JSON.stringify(found)}`
		);
		await delay(100);
	}
}
async function physical(phase, rgb, expectedBounds) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const found = windows();
		assert(found.length <= 2, 'only the two requested physical outputs may exist');
		if (
			found.length === 2 &&
			found.every(
				(window) =>
					window.visible &&
					!window.minimized &&
					window.center_rgb.length === 1 &&
					JSON.stringify(window.center_rgb[0]) === JSON.stringify(rgb)
			)
		) {
			for (const bounds of expectedBounds) {
				const window = found.find(
					(window) => JSON.stringify(window.bounds) === JSON.stringify(bounds)
				);
				assert(window, JSON.stringify({ phase, bounds, found }));
				const key = JSON.stringify(bounds);
				if (identities.has(key))
					assert.equal(window.hwnd, identities.get(key), `${phase}: physical output identity`);
				identities.set(key, window.hwnd);
			}
			return found;
		}
		assert(Date.now() < deadline, JSON.stringify({ phase, rgb, found, log: native.log() }));
		await delay(100);
	}
}
let source;
async function checkpoint(phase, rgb, bounds, minimized = false) {
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [...rgb.map((x) => x / 255), 1] }
	});
	const physicalWindows = await physical(phase, rgb, bounds);
	await observer.observePixels(phase, [...rgb, 255]);
	const mainWindow = await windowState(minimized);
	checkpoints.push({
		phase,
		mainWindow,
		physicalWindows,
		spoutRgba: [...rgb, 255],
		nativePreviews: await native.page.locator('.native-preview').count()
	});
}
try {
	token = (await post('/api/ui/session', {})).token;
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screen Manager$/ })
		.waitFor();
	let catalog;
	const deadline = Date.now() + 10000;
	do {
		catalog = await control({ kind: 'mapgyverDisplays', refresh: !catalog });
		assert(Date.now() < deadline, JSON.stringify(catalog));
		if (catalog.displays.length < 2) await delay(100);
	} while (catalog.displays.length < 2);
	const fullscreen = catalog.displays.find((display) => !display.primary) ?? catalog.displays[0];
	const primary = catalog.displays.find((display) => display.id !== fullscreen.id);
	const rectangle = {
		left: primary.bounds.left + 80,
		top: primary.bounds.top + 80,
		extent: { width: 400, height: 240 }
	};
	const bounds = [fullscreen.bounds, rectangle];
	({ reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Concurrent live color'
	}));
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	for (const [label, target] of [
		['Concurrent fullscreen', { kind: 'Fullscreen', display: fullscreen.id }],
		['Concurrent rectangle', { kind: 'Rectangle', bounds: rectangle }]
	]) {
		const { screen, surface } = await edit('screens', { kind: 'CreateScreen', label });
		const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference: surface });
		await edit('screens', {
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...snapshot.definition, source }
		});
		await edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled: true, target }
		});
		surfaces.push({ reference: surface, label });
	}
	observer = await createPreviewOutputObserver(post, path.join(directory, 'spout'));
	await observer.observe('first-grid', { first: source });
	const tab = (name) => native.page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	await tab('Screen Manager').click();
	await native.page
		.locator('section[aria-label="Screen Manager"]')
		.getByRole('button', { name: surfaces[0].label, exact: true })
		.click();
	await tab('Screen Warping').click();
	await native.page
		.locator('[data-panel-id="screen-warp"] .native-preview[data-preview-installed="true"]')
		.waitFor();
	await checkpoint('editor-open-red', [255, 0, 0], bounds);
	// Close every panel that can own a native preview; the outputs retain demand themselves.
	for (const name of [
		'Screen Warping',
		'Surface Source',
		'Media Preview',
		'Composition',
		'Matrix'
	]) {
		while (await tab(name).count())
			await tab(name).first().getByRole('button', { name: 'Close panel', exact: true }).click();
	}
	await native.page.waitForFunction(() => !document.querySelector('.native-preview'));
	await checkpoint('all-editors-closed-green', [0, 255, 0], bounds);
	await native.page.getByRole('button', { name: 'Minimize app', exact: true }).click();
	await windowState(true);
	await checkpoint('minimized-blue', [0, 0, 255], bounds, true);
	// Existing product host command used by the titlebar; no new permission or test-only backdoor.
	await native.page.evaluate(() => window.__TAURI_INTERNALS__.invoke('window_toggle_maximize'));
	await windowState(false);
	await checkpoint('restored-red', [255, 0, 0], bounds);
	const consumer = await observer.finish();
	assert.deepEqual(errors, []);
	await post('/api/ui/project-new', {});
	const closeDeadline = Date.now() + 10000;
	while (windows().length) {
		assert(Date.now() < closeDeadline, 'New must retire physical output windows');
		await delay(100);
	}
	const report = {
		result: 'pass',
		pid: native.app.pid,
		displays: catalog.displays,
		checkpoints,
		consumer,
		physicalWindowsRetiredOnNew: true,
		pageErrors: errors,
		scope:
			'actual concurrent fullscreen/rectangle physical pixels and independent Spout SDK frames through closed editors and OS-confirmed main-window minimize/restore; wider physical/timing gates remain separate'
	};
	await writeFile(path.join(directory, 'concurrent.json'), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), checkpoints, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	try {
		await observer?.finish();
	} finally {
		await native.close();
	}
}
