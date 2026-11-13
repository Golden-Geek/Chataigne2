// One independent Q06 sample in the ordinary bundled app; repeat three times for a series.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { randomUUID, createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { inspectNativeDiagnostics } from './native-diagnostics.mjs';
import { inspectLocalVideoLibraries } from './video-runtime-libraries.mjs';
import { SourceBindingCollector } from './source-upload-accounting.mjs';

const directory = path.resolve(process.argv[2]);
const fixtures = JSON.parse(await readFile(process.argv[3], 'utf8'));
const executable = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const selected = process.env.MAPGYVER_VIDEO_ADAPTER;
assert(selected && fixtures.cases.length === 2);
await mkdir(directory, { recursive: true });
process.env.MAPGYVER_NATIVE_DIAGNOSTICS = path.join(directory, 'native');
const command = (args) =>
	execFileSync(executable, args, { encoding: 'utf8', windowsHide: true, timeout: 15000 });
const hardware = JSON.parse(command(['--role', 'hardware']));
const sourceBindings = new SourceBindingCollector();
const native = await openNativeEditor({
	onLogData: (data, stream) => sourceBindings.consume(data, stream),
	prepareProfile: async (root) => {
		const preferences = path.join(root, 'MapGyver');
		await mkdir(preferences, { recursive: true });
		command([
			'--role',
			'gpu-preferences',
			'--file',
			path.join(preferences, 'preferences.json'),
			'--selected',
			selected
		]);
	}
});
const phases = [],
	observations = [],
	outputs = [],
	consumers = [],
	errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
let token, media, report;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(8000),
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
async function edit(payload, namespace = 'media') {
	const value = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(value.success, JSON.stringify(value));
	return value.result.value;
}
async function wait(read, accept, label) {
	const deadline = Date.now() + 20000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(50);
	}
}
const transport = (operation) => edit({ kind: 'Video', media, edit: operation });
async function digest(file) {
	const hash = createHash('sha256');
	for await (const chunk of createReadStream(file)) hash.update(chunk);
	return hash.digest('hex');
}
function qpc() {
	return JSON.parse(
		execFileSync(
			'python',
			[
				'-c',
				'import ctypes,json; c=ctypes.c_longlong(); f=ctypes.c_longlong(); ctypes.windll.kernel32.QueryPerformanceFrequency(ctypes.byref(f)); ctypes.windll.kernel32.QueryPerformanceCounter(ctypes.byref(c)); print(json.dumps([c.value,f.value]))'
			],
			{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
		)
	);
}
async function phase(name, seconds) {
	const [begin_qpc, frequency] = qpc();
	const window = { name, begin_qpc, frequency, seconds, end_qpc: null };
	phases.push(window);
	console.log(JSON.stringify({ phase: name, pid: native.app.pid, seconds }));
	const until = Date.now() + seconds * 1000;
	while (Date.now() < until) {
		assert.equal(native.app.exitCode, null, 'owned app remains live');
		for (const consumer of consumers.filter((value) => !value.stopped))
			assert.equal(consumer.child.exitCode, null);
		const sources = await Promise.all(
			outputs
				.slice(0, 2)
				.map(
					async ({ reference }) => (await edit({ kind: 'InspectVideoSource', reference })).status
				)
		);
		assert(
			sources.every((source) => source.state === 'Ready' && !source.error),
			JSON.stringify(sources)
		);
		const playback = (await edit({ kind: 'InspectVideo', media })).snapshot;
		assert(playback.playing, 'Video must keep playing throughout the sample');
		observations.push({ phase: name, utc: new Date().toISOString(), sources, transport: playback });
		await delay(Math.min(2000, Math.max(0, until - Date.now())));
	}
	window.end_qpc = qpc()[0];
}
async function makeOutput(reference, fixture, label) {
	const { screen, surface } = await edit({ kind: 'CreateScreen', label }, 'screens');
	const definition = (await edit({ kind: 'InspectSurface', reference: surface }, 'screens'))
		.snapshot.definition;
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...definition, source: reference }
		},
		'screens'
	);
	const sender = `MapGyver-Video-${randomUUID()}`;
	await edit(
		{
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				anti_aliasing: 'Raw',
				frame_rate: { kind: 'Explicit', fps: 60 },
				target: { kind: 'Spout', sender, extent: { width: fixture.width, height: fixture.height } }
			}
		},
		'screens'
	);
	return { reference, sender, fixture, screen };
}
async function consumer(output, adapter) {
	const root = path.join(directory, `consumer-${consumers.length}`);
	await mkdir(root);
	const child = spawn(
		executable,
		[
			'--role',
			'consumer',
			'--adapter',
			String(adapter.index),
			'--name',
			output.sender,
			'--width',
			String(output.fixture.width),
			'--height',
			String(output.fixture.height),
			'--instance',
			randomUUID(),
			'--directory',
			root,
			'--max-seconds',
			'600',
			'--max-frames',
			'65536',
			'--binary-frame-code',
			JSON.stringify({
				bits: fixtures.frame_code.bits,
				origin: [fixtures.frame_code.x, fixtures.frame_code.y],
				step: fixtures.frame_code.step,
				sentinel: [fixtures.frame_code.sentinel_x, fixtures.frame_code.y],
				threshold: fixtures.frame_code.threshold
			})
		],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	let stdout = '',
		stderr = '';
	child.stdout.on('data', (value) => {
		stdout = (stdout + value).slice(-16384);
	});
	child.stderr.on('data', (value) => {
		stderr = (stderr + value).slice(-16384);
	});
	const result = {
		child,
		root,
		stopped: false,
		exit: new Promise((resolve) => {
			child.once('error', (error) => {
				stderr += String(error);
				resolve(-1);
			});
			child.once('close', resolve);
		})
	};
	consumers.push(result);
	await wait(
		async () => {
			assert(child.pid && child.exitCode === null, stderr);
			return stdout;
		},
		(value) => value.includes('"kind":"ready"'),
		'independent SDK receiver'
	);
	result.stop = async () => {
		if (result.stopped) return;
		result.stopped = true;
		child.stdin.end('stop\n');
		let timer;
		try {
			const code = await Promise.race([
				result.exit,
				new Promise((_, reject) => {
					timer = setTimeout(() => reject(new Error(`receiver stop expired: ${root}`)), 15000);
				})
			]);
			assert.equal(code, 0, stderr);
		} finally {
			clearTimeout(timer);
		}
	};
}
async function pixels(output, adapter, label, expected) {
	const root = path.join(directory, label);
	const result = JSON.parse(
		command([
			'--role',
			'video-pixels',
			'--adapter',
			String(adapter.index),
			'--name',
			output.sender,
			'--directory',
			root
		])
	);
	assert.equal(result.width, output.fixture.width);
	assert.equal(result.height, output.fixture.height);
	assert.equal(result.retired, true);
	const code = JSON.parse(
		execFileSync(
			'python',
			[
				'-c',
				'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGB"); px=lambda x:sum(im.getpixel((x,24)))/3; assert px(4)>200; print(json.dumps(sum((px(24+b*24)>128)<<b for b in range(12))))',
				path.join(root, 'pixels.png')
			],
			{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
		)
	);
	assert.equal(code, expected, `${label}: independent output frame code`);
	return { label, code, ...result };
}
try {
	token = (await post('/api/ui/session', {})).token;
	await native.page.waitForURL(`${native.base}/**`);
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.waitFor();
	const adapter = await observeNativeAdapter(native.page, hardware);
	assert.equal(adapter.key, selected);
	const libraries = await inspectLocalVideoLibraries(
		native.app.pid,
		process.env.MAPGYVER_TEST_BINARY
	);
	const created = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'Q06 ProRes transport'
	});
	media = created.reference.media;
	await edit({ kind: 'ConfigureMediaResolution', media, size: { kind: 'Automatic' } });
	for (const [index, fixture] of fixtures.cases.entries()) {
		assert.equal(await digest(fixture.file), fixture.sha256);
		const reference =
			index === 0
				? created.reference
				: (await edit({ kind: 'AddChannel', media, label: 'Q06 4K30' })).reference;
		await edit({ kind: 'ConfigureVideoFile', reference, file: fixture.file });
		outputs.push(await makeOutput(reference, fixture, `Q06 ${fixture.width} output`));
	}
	const mirror = (
		await edit({ kind: 'CreateReference', target: media, label: 'Q06 shared Reference' })
	).media;
	const mirrored = (
		await edit({ kind: 'InspectChannels', query: { media: mirror, start: 0, count: 2 } })
	).page.channels;
	assert(mirrored.length >= 1);
	const mirrorHD = mirrored.find(
		(channel) => channel.definition.source.target?.channel === created.reference.channel
	);
	assert(mirrorHD);
	outputs.push(
		await makeOutput(mirrorHD.reference, fixtures.cases[0], 'Q06 independent shared output')
	);
	await transport({ kind: 'Loop', looping: true });
	await transport({ kind: 'Seek', position_ms: 1000 });
	for (const output of outputs.slice(0, 2))
		await wait(
			async () => (await edit({ kind: 'InspectVideoSource', reference: output.reference })).status,
			(value) => value.state === 'Ready' && !value.error,
			'full-size File decoded'
		);
	for (const output of outputs.slice(0, 2))
		await wait(
			async () => (await edit({ kind: 'InspectVideoFrame', reference: output.reference })).frame,
			(value) => value && Math.abs(value.position_ms - 1000) < 0.0001,
			'exact held frame1000ms within20s'
		);
	await delay(100);
	const checkpoints = [];
	for (const [index, output] of outputs.entries())
		checkpoints.push(await pixels(output, adapter, `held-${index}`, output.fixture.rate));
	for (const output of outputs.slice(0, 2)) await consumer(output, adapter);
	await transport({ kind: 'Play' });
	await phase('warmup', 30);
	await phase('sample', 120);
	await native.page.getByRole('button', { name: 'Minimize app', exact: true }).click();
	await phase('minimized', 60);
	await native.page.evaluate(() => window.__TAURI_INTERNALS__.invoke('window_toggle_maximize'));
	for (const current of consumers) await current.stop();
	await phase('receiver-disconnected', 60);
	await transport({ kind: 'Pause' });
	assert.deepEqual(errors, []);
	report = {
		result: 'completed baseline',
		pid: native.app.pid,
		controllerPid: process.pid,
		adapter,
		hardware,
		libraries,
		fixtures,
		binarySha256: await digest(process.env.MAPGYVER_TEST_BINARY),
		probeSha256: await digest(executable),
		cargoLockSha256: await digest('Cargo.lock'),
		nodeVersion: process.version,
		phases,
		observations,
		outputs,
		checkpoints,
		errors,
		consumerDirectories: consumers.map((value) => value.root),
		scope:
			'One independent 30s-warmed120s sample per invocation plus60s minimized and60s receiver disconnected. Continuous SDK consumers record exact shared-image versions; their legacy CardStamp checksum is not a Video frame-code oracle. Held full-resolution output frame codes are checked separately. Three independent samples and complete age/resource/format qualification remain required.'
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), phases, observations, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	for (const current of consumers) {
		try {
			await current.stop?.();
		} finally {
			if (current.child.exitCode === null) {
				current.child.kill();
				await current.exit;
			}
		}
	}
	await native.close();
}
try {
	report.videoSourceBindings = sourceBindings.finish();
	report.nativeDiagnostics = await inspectNativeDiagnostics(native.app.pid, native.log(), {
		requireVideoFrames: true,
		requireSourceBudget: true,
		requireSourceUploads: true
	});
	if (fixtures.codec === 'ProRes HQ') {
		const budget = report.nativeDiagnostics.sourceGpu;
		const groups = budget.maximum_observed_classes;
		const expected = 1920 * 1080 * 4 * 3;
		assert.equal(groups.software_producers.allocations, 1, 'one HD publisher for shared users');
		assert.equal(groups.software_producers.bytes, expected);
		assert.equal(budget.peak_admitted_bytes, expected, 'no unobserved extra source-GPU allocation');
		for (const kind of ['decoder_pools', 'hardware_producers', 'hap_producers'])
			assert.deepEqual(groups[kind], { allocations: 0, bytes: 0 });
		const uploads = budget.upload_budgets.filter((entry) => entry.uploads.software.calls > 0);
		assert.equal(uploads.length, 1, 'one counted HD software publisher budget');
		assert.equal(
			uploads[0].uploads.software.bytes,
			uploads[0].uploads.software.calls * 1920 * 1080 * 4,
			'actual lifetime bytes include uploads that no renderer sampled'
		);
		for (const entry of budget.upload_budgets)
			assert.deepEqual(entry.uploads.hap, { calls: 0, bytes: 0 });
	}
} catch (error) {
	await writeFile(
		path.join(directory, 'diagnostic-failure.json'),
		JSON.stringify(
			{ ...report, result: 'failed', error: String(error), backendLog: native.log() },
			null,
			2
		)
	);
	throw error;
}
report.backendLog = native.log();
await writeFile(path.join(directory, 'sample.json'), JSON.stringify(report, null, 2));
console.log(
	JSON.stringify({ result: report.result, pid: report.pid, adapter: report.adapter.key })
);
