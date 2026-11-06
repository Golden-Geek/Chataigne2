// A finite, manually manipulated topology check using an ordinary owned application.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from '../browser/native-editor.mjs';
import { observeNativeAdapter } from '../browser/native-adapter.mjs';

const directory = path.resolve(process.argv[2]);
const gpu = process.argv[3];
const arrangementOnly = process.argv.includes('--arrangement-only');
assert(/^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu));
await mkdir(directory, { recursive: true });
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = () =>
	JSON.parse(
		execFileSync(observer, ['--role', 'hardware'], {
			encoding: 'utf8',
			windowsHide: true,
			timeout: 10000
		})
	);
const before = hardware();
await writeFile(path.join(directory, 'hardware-before.json'), JSON.stringify(before, null, 2));
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const profile = path.join(root, 'MapGyver');
		await mkdir(profile, { recursive: true });
		execFileSync(
			observer,
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(profile, 'preferences.json'),
				'--selected',
				gpu
			],
			{ windowsHide: true }
		);
	}
});
const errors = [];
await writeFile(
	path.join(directory, 'ready.json'),
	JSON.stringify({ pid: native.app.pid, base: native.base, cdp: native.cdp }, null, 2)
);
native.page.on('pageerror', (error) => errors.push(String(error)));
let token, screen, original, adapter;
const observations = [];
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
}
async function edit(payload, namespace = 'screens') {
	const reply = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(reply.success, JSON.stringify(reply));
	return reply.result.value;
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
			{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
		)
	);
async function progress(next) {
	const value = { pid: native.app.pid, gpu, next, observations: observations.map((o) => o.stage) };
	await writeFile(path.join(directory, 'progress.json'), JSON.stringify(value, null, 2));
	console.log(JSON.stringify(value));
}
async function observe(stage, predicate, expected, colour = true) {
	const deadline = Date.now() + 150000;
	let last;
	while (Date.now() < deadline) {
		const catalog = await control({ kind: 'mapgyverDisplays', refresh: false });
		if (!catalog.error && predicate(catalog.displays)) {
			const target = expected(catalog.displays);
			const found = windows();
			assert(found.length <= 1, 'one Screen must not create overlapping output windows');
			const window = found[0];
			last = { catalog, found, target };
			if (
				window &&
				window.visible &&
				!window.minimized &&
				JSON.stringify(window.bounds) === JSON.stringify(target.bounds) &&
				(!colour || window.center_rgb.some((rgb) => rgb[0] >= 250 && rgb[1] <= 2 && rgb[2] <= 2))
			) {
				const authored = await edit({ kind: 'InspectScreen', screen });
				assert.deepEqual(
					authored.definition,
					original,
					'topology recovery must preserve authored destination'
				);
				const status = await control({ kind: 'mapgyverRuntimeStatus' });
				if (!status.gpu_adapter || status.state !== 'Running') {
					last.status = status;
					await delay(300);
					continue;
				}
				assert.equal(status.gpu_adapter, gpu, 'topology changes must not silently migrate the GPU');
				const result = { stage, time: new Date().toISOString(), catalog, window, authored, status };
				observations.push(result);
				await writeFile(path.join(directory, `${stage}.json`), JSON.stringify(result, null, 2));
				return;
			}
		}
		await delay(300);
	}
	throw new Error(`${stage} timed out: ${JSON.stringify(last)}`);
}
try {
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.waitFor();
	token = (await post('/api/ui/session', {})).token;
	let catalog;
	for (let i = 0; i < 50; i++) {
		catalog = await control({ kind: 'mapgyverDisplays', refresh: i === 0 });
		if (catalog.displays.length === 2) break;
		await delay(100);
	}
	assert.equal(catalog.displays.length, 2, 'start with both original monitors connected');
	const external = catalog.displays.find((d) => !d.primary);
	const internal = catalog.displays.find((d) => d.primary);
	assert(external && internal);
	assert(
		external.edid && external.edid !== internal.edid,
		'this fixture requires distinct monitor EDIDs'
	);
	const intended = (ds) => ds.find((d) => d.edid === external.edid);
	const created = await edit(
		{ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Hotplug red' },
		'media'
	);
	await edit(
		{
			kind: 'ConfigureChannel',
			reference: created.reference,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		},
		'media'
	);
	const output = await edit({ kind: 'CreateScreen', label: 'Physical recovery check' });
	screen = output.screen;
	const surface = await edit({ kind: 'InspectSurface', reference: output.surface });
	await edit({
		kind: 'ConfigureSurface',
		reference: output.surface,
		definition: { ...surface.snapshot.definition, source: created.reference }
	});
	await edit({
		kind: 'ConfigureScreen',
		screen,
		definition: { enabled: true, target: { kind: 'Fullscreen', display: external.id } }
	});
	original = (await edit({ kind: 'InspectScreen', screen })).definition;
	await observe('baseline', (ds) => ds.length === 2, intended);
	adapter = await observeNativeAdapter(native.page, before);
	assert.equal(adapter.key, gpu);
	if (!arrangementOnly) {
		await progress('unplug-external');
		await observe(
			'disconnected',
			(ds) => ds.length === 1 && !intended(ds),
			(ds) => ds[0],
			false
		);
		await progress('reconnect-external');
		await observe('reconnected', (ds) => ds.length === 2 && intended(ds), intended);
	}
	await progress('move-external-left');
	await observe('negative-live', (ds) => intended(ds)?.bounds.left < 0, intended);
	await progress('restore-original-arrangement');
	await observe(
		'restored',
		(ds) =>
			ds.length === 2 &&
			ds.every((d) => {
				const initial = catalog.displays.find((v) => v.edid === d.edid);
				return (
					initial &&
					JSON.stringify(d.bounds) === JSON.stringify(initial.bounds) &&
					d.primary === initial.primary
				);
			}),
		intended
	);
	await edit({ kind: 'ConfigureScreen', screen, definition: { ...original, enabled: false } });
	const deadline = Date.now() + 10000;
	while (windows().length) {
		assert(Date.now() < deadline, 'output must retire after disabling');
		await delay(100);
	}
	assert.deepEqual(errors, []);
	const after = hardware();
	const physical = (displays) =>
		displays
			.map(({ edid, bounds, primary }) => ({ edid, bounds, primary }))
			.sort((a, b) => a.edid.localeCompare(b.edid));
	assert.deepEqual(
		physical(after.displays),
		physical(before.displays),
		'restore physical monitor identities and topology; OS paths may change on hotplug'
	);
	await writeFile(path.join(directory, 'hardware-after.json'), JSON.stringify(after, null, 2));
	await native.close();
	await writeFile(
		path.join(directory, 'normal-exit.json'),
		JSON.stringify({ pid: native.app.pid, normalExit: 0 })
	);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				pass: true,
				pid: native.app.pid,
				gpu,
				adapter,
				observations,
				pageErrors: errors,
				outputRetired: true,
				normalExit: 0
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ pass: true, directory }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), observations, log: native.log() }, null, 2)
	);
	await native.close();
	await writeFile(
		path.join(directory, 'normal-exit.json'),
		JSON.stringify({ pid: native.app.pid, normalExit: 0 })
	);
	throw error;
}
