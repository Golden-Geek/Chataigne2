// M11/10: one finite actual-app cohort through public authoring and independent native pixels.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey as hardwareAdapterKey } from './native-adapter.mjs';

const execute = promisify(execFile);
const directory = path.resolve(process.env.MAPGYVER_CALIBRATION_DIRECTORY);
await mkdir(directory, { recursive: true });
const observer = path.resolve(process.env.MAPGYVER_CALIBRATION_OBSERVER);
const antiAliasing = process.env.MAPGYVER_CALIBRATION_AA ?? 'Raw';
assert(['Raw', 'Smooth2x'].includes(antiAliasing), 'unknown calibration anti-aliasing mode');
const selectedAdapter = process.env.MAPGYVER_CALIBRATION_GPU;
let appDataDirectory;
if (selectedAdapter) {
	const hardware = JSON.parse(
		(await execute(observer, ['--role', 'hardware'], { windowsHide: true })).stdout
	);
	assert(
		hardware.adapters.some(
			(adapter) => !adapter.software && hardwareAdapterKey(adapter.id) === selectedAdapter
		),
		'requested hardware GPU must exist'
	);
	appDataDirectory = path.join(directory, 'app-data');
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
			selectedAdapter
		],
		{ windowsHide: true }
	);
}
const native = await openNativeEditor({ appDataDirectory });
let retirementCount = 0,
	outputBuffer = '';
native.app.stderr.on('data', (chunk) => {
	outputBuffer += chunk.toString();
	const lines = outputBuffer.split('\n');
	outputBuffer = lines.pop();
	for (const line of lines)
		if (line.trim() === '[mapgyver-output] native session retired') retirementCount++;
});
const observations = [];
const routingOnly = process.argv.includes('--routing-only');
let token;
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
const screens = (payload) => edit('screens', payload);
const media = (payload) => edit('media', payload);
async function status() {
	return native.page.evaluate(async () =>
		JSON.parse(
			await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
				request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
			})
		)
	);
}
async function waitFor(read, predicate, label) {
	const deadline = Date.now() + 12000;
	while (true) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}\n${native.log()}`);
		await delay(100);
	}
}
async function fixture(args) {
	const { stdout } = await execute(observer, args, {
		windowsHide: true,
		timeout: 15000,
		maxBuffer: 1024 * 1024
	});
	return JSON.parse(stdout);
}
try {
	// The first WebView document can still be navigating when CDP attaches.
	await native.page.getByRole('tab').first().waitFor();
	token = (await post('/api/ui/session', {})).token;
	await post('/api/ui/project-new', {});
	const errors = [];
	native.page.on('pageerror', (error) => errors.push(String(error)));
	const { screen, surface } = await screens({
		kind: 'CreateScreen',
		label: 'Calibration acceptance'
	});
	const sender = `MapGyver-M1110-${native.app.pid}`;
	const original = {
		source: null,
		crop: { x: 0, y: 0, width: 1, height: 1 },
		quad: {
			corners: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 1, y: 1 },
				{ x: 0, y: 1 }
			]
		},
		opacity: 1,
		enabled: true,
		calibration: 'Grid'
	};
	await screens({ kind: 'ConfigureSurface', reference: surface, definition: original });
	async function target(width, height, enabled = true) {
		await screens({
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled,
				anti_aliasing: antiAliasing,
				target: { kind: 'Spout', sender, extent: { width, height } }
			}
		});
	}
	await target(1920, 1080);
	const running = await waitFor(
		status,
		(value) => value.state === 'Running' && value.submitted_fps > 0,
		'running source-free calibration'
	);
	if (selectedAdapter)
		assert.equal(running.gpu_adapter, selectedAdapter, 'exact GPU preference must be honored');
	const hardware = await fixture(['--role', 'hardware']);
	const planLine = () =>
		native
			.log()
			.split('\n')
			.filter((line) => line.includes('[mapgyver-presentation] plan installed;'))
			.at(-1) ?? '';
	const sourceFreePlan = await waitFor(
		planLine,
		(line) => /channels=0 screens=1 outputs=1/.test(line),
		'source-free calibration demand'
	);
	function adapterKey(id) {
		return typeof id === 'string'
			? id
			: `dxgi:${(id.high >>> 0).toString(16).padStart(8, '0')}:${(id.low >>> 0).toString(16).padStart(8, '0')}`;
	}
	const adapter = hardware.adapters.findIndex(
		(item) => adapterKey(item.id) === running.gpu_adapter
	);
	assert(adapter >= 0, JSON.stringify({ hardware, running }));
	async function capture(label, width, height) {
		await waitFor(status, (value) => value.state === 'Running' && value.error === null, label);
		// Allow source replacement to publish past already-presented generations; no process restart.
		await delay(300);
		const before = Date.now();
		const pixels = await fixture([
			'--role',
			'calibration-pixels',
			'--adapter',
			String(adapter),
			'--name',
			sender,
			'--directory',
			path.join(directory, label)
		]);
		assert.deepEqual(pixels, { width, height, retired: true });
		const runtime = await status();
		assert.equal(runtime.error, null, JSON.stringify(runtime));
		const observation = { label, before, after: Date.now(), pixels, runtime };
		observations.push(observation);
		await writeFile(
			path.join(directory, 'observations.json'),
			JSON.stringify(
				{ pid: native.app.pid, gpu: running.gpu_adapter, observations, sourceFreePlan },
				null,
				2
			)
		);
		console.log(JSON.stringify(observation));
	}
	if (!routingOnly) {
		for (const [label, width, height] of [
			['landscape', 1920, 1080],
			['portrait', 1080, 1920],
			['square', 1024, 1024],
			['4k', 3840, 2160],
			['resize', 1919, 1079]
		]) {
			await target(width, height);
			await capture(label, width, height);
		}
		await target(1920, 1080);
		await capture('live-a', 1920, 1080);
		await delay(1200);
		await capture('live-b', 1920, 1080);
		const warped = {
			...original,
			quad: {
				corners: [
					{ x: 0.12, y: 0.14 },
					{ x: 0.84, y: 0.08 },
					{ x: 0.91, y: 0.86 },
					{ x: 0.06, y: 0.94 }
				]
			}
		};
		await screens({ kind: 'ConfigureSurface', reference: surface, definition: warped });
		await capture('warped', 1920, 1080);
	}
	const { reference } = await media({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Restored red routing'
	});
	await media({
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const routed = {
		...original,
		source: reference,
		crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 }
	};
	await screens({ kind: 'ConfigureSurface', reference: surface, definition: routed });
	await capture('routed-calibration', 1920, 1080);
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...routed, calibration: 'Off' }
	});
	const inspected = await screens({ kind: 'InspectSurface', reference: surface });
	assert.deepEqual(inspected.snapshot.definition.source, reference);
	assert.deepEqual(
		inspected.snapshot.definition.crop,
		Object.fromEntries(Object.entries(routed.crop).map(([key, value]) => [key, Math.fround(value)]))
	);
	await capture('restored-source', 1920, 1080);
	const retirementCountBefore = retirementCount;
	await target(1920, 1080, false);
	// A host may retain its transparent presenter. Observe this output's withdrawal, not global idleness.
	const withdrawn = await waitFor(
		status,
		(value) => value.screens.length === 0,
		'final output withdrawal'
	);
	await delay(1200);
	const afterWithdrawal = await status();
	assert.equal(afterWithdrawal.screens.length, 0);
	assert.equal(afterWithdrawal.error, null);
	await native.page.evaluate(async () =>
		window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
			request: JSON.stringify({ kind: 'mapgyverSenders', refresh: true })
		})
	);
	const senders = await waitFor(
		() =>
			native.page.evaluate(async () =>
				JSON.parse(
					await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
						request: JSON.stringify({ kind: 'mapgyverSenders', refresh: false })
					})
				)
			),
		(value) => !value.refreshing && !value.names.includes(sender),
		'owned calibration sender retirement'
	);
	// The media preview can retain its independently selected red source; the calibration output is gone.
	await waitFor(
		() => retirementCount,
		(count) => count > retirementCountBefore,
		'calibration output retirement'
	);
	const withdrawnPlan =
		'Owned output session retired after disable; sender absent. Independent Media Preview may retain its red source.';
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'results.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				adapter,
				anti_aliasing: antiAliasing,
				retirementCountBefore,
				retirementCountAfter: retirementCount,
				gpu: running.gpu_adapter,
				observations,
				withdrawn,
				afterWithdrawal,
				senders,
				sourceFreePlan,
				withdrawnPlan,
				errors,
				sourceRestoration: inspected
			},
			null,
			2
		)
	);
} finally {
	await writeFile(path.join(directory, 'app.log'), native.log());
	await native.close();
}
