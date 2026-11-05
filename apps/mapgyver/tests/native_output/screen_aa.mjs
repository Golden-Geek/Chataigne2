// Physical/SDK equivalence through public authoring and read-only owned-window pixels.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from '../browser/native-editor.mjs';
import { adapterKey } from '../browser/native-adapter.mjs';

const execute = promisify(execFile);
const directory = path.resolve(process.argv[2]);
const observer = path.resolve(process.env.MAPGYVER_CALIBRATION_OBSERVER);
const gpu = process.env.MAPGYVER_PARITY_GPU;
const hardware = JSON.parse(
	(await execute(observer, ['--role', 'hardware'], { windowsHide: true })).stdout
);
const adapter = hardware.adapters.findIndex(
	(entry) => !entry.software && adapterKey(entry.id) === gpu
);
assert(adapter >= 0, 'an exact available hardware GPU is required');
await mkdir(directory, { recursive: true });
const appDataDirectory = path.join(directory, 'app-data');
const preferencesDirectory = path.join(appDataDirectory, 'MapGyver');
await mkdir(preferencesDirectory, { recursive: true });
await execute(
	observer,
	[
		'--role',
		'gpu-preferences',
		'--file',
		path.join(preferencesDirectory, 'preferences.json'),
		'--selected',
		gpu
	],
	{ windowsHide: true }
);
const native = await openNativeEditor({ appDataDirectory });
const results = [],
	errors = [];
let token,
	retirements = 0,
	stderrBuffer = '',
	plans = 0,
	sharedChannelObserved = false;
native.page.on('pageerror', (error) => errors.push(String(error)));
native.app.stderr.on('data', (chunk) => {
	stderrBuffer += chunk.toString();
	const lines = stderrBuffer.split('\n');
	stderrBuffer = lines.pop();
	for (const line of lines) {
		if (line.trim() === '[mapgyver-output] native session retired') retirements++;
		if (line.startsWith('[mapgyver-presentation] plan installed;')) {
			plans++;
			sharedChannelObserved ||= line.includes('channels=1 screens=2 outputs=2');
		}
	}
});
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
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
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
async function control(request) {
	return native.page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
}
async function waitFor(read, predicate, label) {
	const deadline = Date.now() + 12000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(50);
	}
}
async function windows() {
	return JSON.parse(
		(
			await execute(
				'python',
				['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid)],
				{ windowsHide: true }
			)
		).stdout
	);
}
async function compare(label, mode, bounds) {
	const folder = path.join(directory, label);
	await mkdir(folder, { recursive: true });
	const pixels = JSON.parse(
		(
			await execute(
				observer,
				[
					'--role',
					'calibration-pixels',
					'--adapter',
					String(adapter),
					'--name',
					sender,
					'--directory',
					folder
				],
				{ windowsHide: true }
			)
		).stdout
	);
	assert.deepEqual(pixels, {
		width: bounds.extent.width,
		height: bounds.extent.height,
		retired: true
	});
	const capture = async () => {
		try {
			return JSON.parse(
				(
					await execute(
						'python',
						[
							'apps/mapgyver/tests/native_output/screen_aa_pixels.py',
							String(native.app.pid),
							path.join(folder, 'pixels.png'),
							folder,
							mode
						],
						{ windowsHide: true }
					)
				).stdout
			);
		} catch (error) {
			return { passed: false, detail: error.stdout || error.stderr || String(error) };
		}
	};
	let image = await capture();
	if (!image.passed) {
		// Preserve one live owned handle for supported desktop activation; never restart the app.
		const resume = path.join(directory, `${label}-continue`);
		const checkpoint = { label, pid: native.app.pid, windows: await windows(), resume, image };
		await writeFile(
			path.join(directory, 'activation-required.json'),
			JSON.stringify(checkpoint, null, 2)
		);
		console.log(JSON.stringify({ activation_required: checkpoint }));
		const deadline = Date.now() + 8 * 60 * 1000;
		for (;;) {
			try {
				await access(resume);
				break;
			} catch {
				/* One bounded owned-window handoff. */
			}
			assert(Date.now() < deadline, 'owned physical activation checkpoint expired');
			await delay(100);
		}
		image = await capture();
	}
	assert(image.passed, JSON.stringify(image));
	assert.deepEqual(image.window.bounds, bounds);
	assert.equal(image.dpi, 96, 'this cohort targets the current 1080p/100% monitor');
	results.push({
		label,
		...image,
		sdk: pixels,
		runtime: await control({ kind: 'mapgyverRuntimeStatus' })
	});
	console.log(JSON.stringify(results.at(-1)));
	await writeFile(path.join(directory, 'checkpoints.json'), JSON.stringify(results, null, 2));
	return image.window.hwnd;
}

const sender = `MapGyver-M1110-AA-Parity-${native.app.pid}`;
try {
	token = (await post('/api/ui/session', {})).token;
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.waitFor();
	let refreshDisplays = true;
	const catalog = await waitFor(
		() => {
			const refresh = refreshDisplays;
			refreshDisplays = false;
			return control({ kind: 'mapgyverDisplays', refresh });
		},
		(value) => value.displays.length >= 2,
		'local display catalog'
	);
	const display = catalog.displays.find(
		(entry) => entry.bounds.extent.width === 1920 && entry.bounds.extent.height === 1080
	);
	assert(display, 'current 1080p external display is required');
	const { reference } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Parity translucent purple'
	});
	await edit('media', {
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'SolidColor', rgba: [0.5, 0.125, 1, 0.5] }
	});
	await edit('media', {
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 8, height: 8 } }
	});
	const physical = await edit('screens', { kind: 'CreateScreen', label: 'AA physical parity' });
	const shared = await edit('screens', { kind: 'CreateScreen', label: 'AA SDK parity' });
	const definition = {
		source: reference,
		crop: { x: 0, y: 0, width: 1, height: 1 },
		quad: {
			corners: [
				{ x: 0.127, y: 0.093 },
				{ x: 0.913, y: 0.217 },
				{ x: 0.771, y: 0.887 },
				{ x: 0.073, y: 0.713 }
			]
		},
		opacity: 1,
		enabled: true,
		calibration: 'Off'
	};
	for (const item of [physical, shared])
		await edit('screens', { kind: 'ConfigureSurface', reference: item.surface, definition });
	for (const policy of ['Rectangle', 'Fullscreen']) {
		const bounds =
			policy === 'Rectangle'
				? {
						left: display.bounds.left + 80,
						top: display.bounds.top + 80,
						extent: { width: 512, height: 384 }
					}
				: display.bounds;
		let identity;
		for (const mode of ['Raw', 'Smooth2x']) {
			const before = plans,
				retirementBefore = retirements;
			await edit('screens', {
				kind: 'ConfigureScreen',
				screen: shared.screen,
				definition: {
					enabled: true,
					anti_aliasing: mode,
					target: { kind: 'Spout', sender, extent: bounds.extent }
				}
			});
			await edit('screens', {
				kind: 'ConfigureScreen',
				screen: physical.screen,
				definition: {
					enabled: true,
					anti_aliasing: mode,
					target:
						policy === 'Rectangle'
							? { kind: policy, bounds }
							: { kind: policy, display: display.id }
				}
			});
			await waitFor(
				() => plans,
				(value) => value > before,
				'current presentation plan'
			);
			await waitFor(
				windows,
				(value) => value.length === 1 && JSON.stringify(value[0].bounds) === JSON.stringify(bounds),
				'one owned physical target'
			);
			await waitFor(
				() => control({ kind: 'mapgyverRuntimeStatus' }),
				(value) => value.state === 'Running' && value.gpu_adapter === gpu && value.error === null,
				'exact active GPU'
			);
			await delay(500); // Settled parity only; no latency or scanout claim.
			const hwnd = await compare(`${policy}-${mode}`, mode, bounds);
			if (identity) {
				assert.equal(hwnd, identity, 'mode switching preserves the physical HWND');
				assert.equal(
					retirements,
					retirementBefore,
					'mode switching preserves native output bindings'
				);
			}
			identity = hwnd;
		}
	}
	assert(sharedChannelObserved, 'two Screens must share one captured channel');
	const beforeDisable = retirements;
	for (const item of [physical, shared]) {
		const current = await edit('screens', { kind: 'InspectScreen', screen: item.screen });
		await edit('screens', {
			kind: 'ConfigureScreen',
			screen: item.screen,
			definition: { ...current.definition, enabled: false }
		});
	}
	await waitFor(windows, (value) => value.length === 0, 'physical withdrawal');
	await waitFor(
		() => retirements,
		(value) => value > beforeDisable,
		'native retirement'
	);
	await control({ kind: 'mapgyverSenders', refresh: true });
	await waitFor(
		() => control({ kind: 'mapgyverSenders', refresh: false }),
		(value) => !value.refreshing && !value.names.includes(sender),
		'owned SDK sender withdrawal'
	);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'results.json'),
		JSON.stringify(
			{
				passed: true,
				pid: native.app.pid,
				gpu,
				display,
				results,
				errors,
				shared_channel: reference,
				shared_channel_observed: sharedChannelObserved,
				owned_sender_absent: true,
				retirements_before_disable: beforeDisable,
				retirements
			},
			null,
			2
		)
	);
} finally {
	await writeFile(path.join(directory, 'app.log'), native.log());
	await native.close();
}
