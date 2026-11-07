// Ordinary app, independently uploaded native source textures and independent SDK output pixels.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { contentLock } from './content-lock.mjs';
import { liveCases } from './native-source-live-cases.mjs';

const directory = path.resolve(process.argv[2]);
const index = Number(process.argv[3]);
const continuous = process.argv[4] === '--continuous';
const deviceMode = process.argv.find((argument) =>
	['--device-reset', '--reset-preflight'].includes(argument)
);
assert(!deviceMode || continuous, 'Device recovery retains the continuous producer until reset');
assert(Number.isInteger(index) && index >= 0);
await mkdir(directory);
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
const adapter = hardware.adapters[index];
assert(adapter && !adapter.software);
const key = adapterKey(adapter.id);
const sender = `MapGyverFormat-${randomUUID()}`;
const child = spawn(
	path.resolve('target/debug/deps/golden_video-fa94e9e28cc1568b.exe'),
	[
		'--exact',
		'native::inputs::tests::formats::source_fixture::owned_native_source_for_app_format_checks',
		'--ignored',
		'--nocapture',
		'--test-threads=1'
	],
	{
		windowsHide: true,
		stdio: ['pipe', 'pipe', 'pipe'],
		env: {
			...process.env,
			GOLDEN_VIDEO_APP_SOURCE_ADAPTER: String(index),
			GOLDEN_VIDEO_APP_SOURCE_NAME: sender
		}
	}
);
let sourceError = '';
child.stderr.on('data', (data) => (sourceError += data));
const exited = new Promise((resolve, reject) => {
	child.once('error', reject);
	child.once('close', resolve);
});
const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
async function reply() {
	const deadline = Date.now() + 12000;
	for (;;) {
		const row = await Promise.race([
			lines.next(),
			delay(Math.max(1, deadline - Date.now()), undefined, { ref: false }).then(() => {
				throw Error(`source reply expired: ${sourceError}`);
			})
		]);
		assert(!row.done, `source exited: ${sourceError}`);
		if (row.value.includes('OK ')) return row.value;
		assert(Date.now() < deadline, sourceError);
	}
}
await reply();
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const preferences = path.join(root, 'MapGyver');
		await mkdir(preferences, { recursive: true });
		execFileSync(
			probe,
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(preferences, 'preferences.json'),
				'--selected',
				key
			],
			{ windowsHide: true }
		);
	}
});
const page = native.page;
let token, output;
const errors = [],
	stages = [];
let deviceRecovery;
page.on('pageerror', (error) => errors.push(String(error)));
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
const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}(?::|$)`) });
const encode = (value) =>
	Math.round(255 * (value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055));
const float = (values) => {
	const buffer = Buffer.alloc(16);
	values.forEach((v, i) => buffer.writeFloatLE(v, 4 * i));
	return [...buffer];
};
const halves = (values) => {
	const buffer = Buffer.alloc(8);
	values.forEach((v, i) => buffer.writeUInt16LE(v, 2 * i));
	return [...buffer];
};
const uint = (values) => {
	const buffer = Buffer.alloc(16);
	values.forEach((v, i) => buffer.writeUInt32LE(v, 4 * i));
	return [...buffer];
};
const expected = [0.125, 0.5, 0.75].map(encode);
const cases = [
	{ label: 'bgra8', format: 87, pixel: [0, 0, 255, 255], rgba: [255, 0, 0, 255] },
	{
		label: 'rgba32f',
		format: 2,
		pixel: float([0.125, 0.5, 0.75, 1]),
		rgba: [...expected, 255],
		visual: true
	},
	{
		label: 'rgba16f',
		format: 10,
		pixel: halves([0x3000, 0x3800, 0x3a00, 0x3c00]),
		rgba: [...expected, 255]
	},
	{
		label: 'rgba32uint',
		format: 3,
		pixel: uint([0x20000000, 0x80000000, 0xc0000000, 0xffffffff]),
		rgba: [...expected, 255]
	},
	{
		label: 'nv12',
		format: 103,
		bytes: [...Array(16).fill(126), ...Array(8).fill(128)],
		rgba: [140, 140, 140, 255]
	},
	{ label: 'rgba8-srgb', format: 29, pixel: [128, 64, 192, 255], rgba: [128, 64, 192, 255] },
	{
		label: 'rgba32f-alpha',
		format: 2,
		pixel: float([0.125, 0.5, 0.75, 0.5]),
		rgba: [...expected, 128]
	}
];
async function checkpoint(label, view) {
	const frame = await page
		.locator(
			view === 'media'
				? '.media-preview-panel .native-preview'
				: '[aria-label="Screen Warping"] .native-preview'
		)
		.first()
		.evaluate((element) => ({
			dataset: { ...element.dataset },
			rect: element.getBoundingClientRect().toJSON()
		}));
	const ready = {
		pid: native.app.pid,
		adapter: { index, key, description: adapter.description },
		label,
		view,
		frame
	};
	await writeFile(
		path.join(directory, `ready-${label}-${view}.json`),
		JSON.stringify(ready, null, 2)
	);
	console.log(JSON.stringify({ ready }));
	const deadline = Date.now() + 120000;
	while (!(await stat(path.join(directory, `continue-${label}-${view}`)).catch(() => null))) {
		assert(Date.now() < deadline, `visual checkpoint expired: ${label}/${view}`);
		assert.equal(native.app.exitCode, null, native.log());
		await delay(100);
	}
}
async function waitForDeviceCycle() {
	const ready = { pid: native.app.pid, adapter: { index, key }, mode: deviceMode };
	await writeFile(path.join(directory, 'reset-ready.json'), JSON.stringify(ready));
	console.log(JSON.stringify({ reset_ready: ready }));
	const samples = [];
	const deadline = Date.now() + 90000;
	let loss;
	do {
		assert(Date.now() < deadline, 'Device reset controller/loss observation expired');
		assert.equal(native.app.exitCode, null, native.log());
		const status = await page.evaluate(async () =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
				})
			)
		);
		if (!samples.length || samples.at(-1).status.sequence !== status.sequence)
			samples.push({ utc: new Date().toISOString(), status });
		if (status.error?.includes('render device was lost'))
			loss ??= { utc: new Date().toISOString(), status };
		await writeFile(
			path.join(directory, 'reset-observations.json'),
			JSON.stringify({ ready, loss, samples })
		);
		if (await stat(path.join(directory, 'reset-complete')).catch(() => null)) break;
		await delay(100);
	} while (true);
	if (deviceMode === '--device-reset') assert(loss, 'Actual app must observe real device loss');
	else assert.equal(loss, undefined, 'No-reset preflight must not claim removal');
	deviceRecovery = { genuine: deviceMode === '--device-reset', loss, samples };
}
try {
	await page.getByRole('tab').first().waitFor();
	await page.evaluate(async () => {
		const host = window.__TAURI_INTERNALS__;
		window.__formatPreviewStatuses = [];
		await host.invoke('plugin:event|listen', {
			event: 'mapgyver-preview-status',
			target: { kind: 'Any' },
			handler: host.transformCallback((event) => {
				window.__formatPreviewStatuses = event.payload;
			})
		});
	});
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Format verification source'
	});
	await edit('media', {
		kind: 'ConfigureMediaSenderSelection',
		media: reference.media,
		mode: 'Manual'
	});
	await edit('media', { kind: 'ConfigureChannel', reference, source: { kind: 'Spout', sender } });
	output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	const { screen } = await output.demand(reference);
	assert.equal((await observeNativeAdapter(page, hardware)).key, key);
	let session;
	const verifySelection = async () => {
		const { page: channels } = await edit('media', {
			kind: 'InspectChannels',
			query: { media: reference.media, start: 0, count: 8 }
		});
		assert.deepEqual(
			channels.channels.find((channel) => channel.reference.channel === reference.channel)
				.definition.source,
			{ kind: 'Spout', sender }
		);
		assert.equal((await observeNativeAdapter(page, hardware)).key, key);
	};
	const selectedCases = deviceMode
		? [
				liveCases.find((item) => item.label === (index === 0 ? 'float-cyan' : 'nv12')),
				liveCases.find((item) => item.restart),
				liveCases.find((item) => item.label === 'half-wide'),
				liveCases.find((item) => item.label === 'sint-tall'),
				liveCases.find((item) => item.label === 'p010-same-canonical-layout')
			]
		: continuous
			? liveCases
			: cases;
	for (const item of selectedCases) {
		if (item.restart) {
			if (deviceMode) await waitForDeviceCycle();
			child.stdin.write('stop\n');
			assert((await reply()).includes('OK stopped'));
			await page.waitForFunction(
				(reference) =>
					window.__formatPreviewStatuses.some(
						(status) =>
							status.source.kind === 'Channel' &&
							status.source.reference.channel === reference.channel &&
							(status.error || status.message)
					),
				reference
			);
			await verifySelection();
		}
		const width = item.width ?? 4,
			height = item.height ?? 4;
		const bytes = item.bytes ?? Array.from({ length: width * height }, () => item.pixel).flat();
		child.stdin.write(
			JSON.stringify({ format: item.format, width, height, bytes, continuous }) + '\n'
		);
		const acknowledgment = await reply();
		const nextSession = acknowledgment.trim().split(' ').at(-1);
		if (continuous && session) {
			if (item.restart) assert.notEqual(nextSession, session, 'restart creates a new producer');
			else assert.equal(nextSession, session, 'live updates retain the producer SDK/device');
		}
		session = nextSession;
		await delay(700);
		await output.observePixels(item.label, item.rgba, 2);
		await tab('Medias').click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Format verification source', exact: true })
			.click();
		await tab('Media Preview').click();
		const media = page.locator('.media-preview-panel');
		await media
			.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
			.waitFor();
		if ((await media.getAttribute('data-preview-locked')) !== 'true')
			await (await contentLock(media, false)).click();
		await page.waitForFunction(
			({ reference, width, height }) =>
				window.__formatPreviewStatuses.some(
					(status) =>
						status.source.kind === 'Channel' &&
						status.source.reference.channel === reference.channel &&
						!status.error &&
						!status.message &&
						status.extent?.width === width &&
						status.extent?.height === height
				),
			{ reference, width, height }
		);
		await verifySelection();
		if (item.visual) await checkpoint(item.label, 'media');
		await tab('Screens').click();
		await page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: 'Independent preview acceptance', exact: true })
			.click();
		await tab('Screen Warping').click();
		await page
			.locator(
				'section[aria-label="Screen Warping"] .native-preview[data-preview-state="available"][data-preview-installed="true"]'
			)
			.first()
			.waitFor();
		if (item.visual) await checkpoint(item.label, 'screen');
		stages.push({
			label: item.label,
			native_format: item.format,
			expected: item.rgba,
			output: output.observations.at(-1),
			preview_installed: true,
			intrinsic_extent: { width, height },
			producer_session: session,
			sender_restarted: item.restart ?? false,
			screen
		});
	}
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				adapter: { index, key, description: adapter.description },
				stages,
				errors,
				source_replacement: !continuous,
				continuous_producer: continuous,
				device_recovery: deviceRecovery
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ passed: true, cases: stages.length, pid: native.app.pid }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{ error: String(error), stack: error.stack, stages, log: native.log(), sourceError },
			null,
			2
		)
	);
	throw error;
} finally {
	try {
		await output?.finish();
	} finally {
		try {
			child.stdin.end('exit\n');
			assert.equal(await exited, 0, sourceError);
		} finally {
			await native.close();
		}
	}
}
