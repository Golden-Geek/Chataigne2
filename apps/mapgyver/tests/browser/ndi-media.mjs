// Real NDI sender, native MapGyver authoring, and exact-channel native preview status.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { access, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

assert(
	process.env.MAPGYVER_TEST_BINARY,
	'Set MAPGYVER_TEST_BINARY to the compiled MapGyver executable'
);
await access(path.resolve(process.env.MAPGYVER_TEST_BINARY));
const output = path.resolve(process.argv[2] ?? 'target/mapgyver-ndi-media');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const readyPath = path.join(directory, 'sender-ready.json');
const stopPath = path.join(directory, 'sender-stop');
const providedName = `MapGyver-NDI-Media-${randomUUID()}`;
const senderProcess = spawn(
	'python',
	[
		'-B',
		path.resolve('apps/mapgyver/tests/native/ndi_sender.py'),
		'--name',
		providedName,
		'--ready',
		readyPath,
		'--stop',
		stopPath,
		'--max-seconds',
		'90'
	],
	{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }
);
let senderLog = '',
	senderErrors = '',
	exit;
senderProcess.stdout.setEncoding('utf8').on('data', (value) => {
	senderLog = (senderLog + value).slice(-16000);
});
senderProcess.stderr.setEncoding('utf8').on('data', (value) => {
	senderErrors = (senderErrors + value).slice(-16000);
});
const exited = new Promise((resolve) => {
	senderProcess.once('error', (error) => resolve({ error: String(error) }));
	senderProcess.once('close', (code, signal) => resolve({ code, signal }));
}).then((value) => (exit = value));

const pageErrors = [];
let native, token, sender, reference, initialSender, selected, availableOptions, preview, failure;
let stage = 'start owned NDI sender';
async function wait(read, accepts, milliseconds = 12000) {
	const deadline = Date.now() + milliseconds;
	for (;;) {
		assert(!exit, `owned NDI sender exited: ${JSON.stringify(exit)} ${senderErrors}`);
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(75);
	}
}
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function inspect(payload) {
	const reply = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(reply.success, JSON.stringify(reply));
	return reply.result.value.snapshot;
}
async function stopSender() {
	let timer;
	try {
		await writeFile(stopPath, 'stop\n', { flag: 'wx' });
		const status = await Promise.race([
			exited,
			new Promise((resolve) => {
				timer = setTimeout(() => resolve(null), 8000);
			})
		]);
		assert(status, 'owned NDI sender did not stop within eight seconds');
		assert.equal(status.code, 0, JSON.stringify({ status, senderErrors }));
		const stopped = JSON.parse(senderLog.trim().split(/\r?\n/).at(-1));
		assert.equal(stopped.kind, 'stopped');
		assert.equal(stopped.reason, 'marker');
		assert(stopped.frames > 0);
		return stopped;
	} finally {
		clearTimeout(timer);
		if (!exit) {
			senderProcess.kill();
			await exited;
		}
	}
}

try {
	sender = await wait(
		async () => {
			try {
				return JSON.parse(await readFile(readyPath, 'utf8'));
			} catch (error) {
				if (error.code === 'ENOENT' || error instanceof SyntaxError) return null;
				throw error;
			}
		},
		Boolean,
		8000
	);
	assert.equal(sender.kind, 'ready');
	assert.equal(sender.pid, senderProcess.pid);
	assert.equal(sender.provided_name, providedName);
	assert(sender.name.endsWith(`(${providedName})`), JSON.stringify(sender));

	stage = 'start MapGyver';
	native = await openNativeEditor({ development: process.env.MAPGYVER_TEST_DEV === '1' });
	const { page } = native;
	page.on('pageerror', (error) => pageErrors.push(String(error)));
	token = (await post('/api/ui/session', {})).token;

	stage = 'create NDI from Medias Add menu';
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	await tab('Medias').click();
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	await manager
		.locator('.manager-list-header')
		.getByRole('button', { name: 'Add child item' })
		.click();
	await page.getByRole('menuitem', { name: 'NDI', exact: true }).click();
	const media = await wait(
		async () =>
			(await inspect({ kind: 'InspectManager' })).media.find((item) => item.media_kind === 'Ndi'),
		Boolean
	);
	assert.equal(media.sender_selection, 'Auto');
	assert.equal(media.channels.length, 1);
	({ reference } = media.channels[0]);
	initialSender = media.channels[0].definition.source.sender;
	await manager.getByRole('button', { name: media.label, exact: true }).click();

	stage = 'select the exact owned sender in the generic Inspector';
	const picker = page.locator('.channels-section select.dropdown-editor');
	await picker.waitFor();
	availableOptions = await wait(
		() =>
			picker
				.locator('option')
				.evaluateAll((options) =>
					options.map((option) => ({ value: option.value, label: option.textContent }))
				),
		(options) =>
			options.some((option) => option.value === sender.name && option.label === sender.name)
	);
	if ((await picker.inputValue()) !== sender.name) await picker.selectOption(sender.name);
	selected = await wait(
		() => inspect({ kind: 'InspectChannel', reference }),
		(channel) =>
			channel?.definition.source.kind === 'Ndi' && channel.definition.source.sender === sender.name
	);
	assert.equal(await picker.inputValue(), sender.name);

	stage = 'verify native Media Preview source, availability and decoded extent';
	await page.evaluate(async (expected) => {
		const host = window.__TAURI_INTERNALS__;
		const handler = host.transformCallback(({ payload }) => {
			const status = payload.find(
				(item) =>
					item.source.kind === 'Channel' &&
					item.source.reference.media === expected.media &&
					item.source.reference.channel === expected.channel
			);
			if (status) window.__mapgyverNdiSmokeStatus = status;
		}, false);
		await host.invoke('plugin:event|listen', {
			event: 'mapgyver-preview-status',
			target: { kind: 'Any' },
			handler
		});
	}, reference);
	await tab('Media Preview').click();
	const panel = page.locator('[data-panel-id="media-preview"]');
	const nativeView = panel.locator('.native-preview');
	await nativeView.waitFor();
	preview = await wait(
		() =>
			nativeView.evaluate((view) => ({
				source: JSON.parse(view.dataset.previewSource),
				state: view.dataset.previewState,
				installed: view.dataset.previewInstalled === 'true',
				nativeStatus: window.__mapgyverNdiSmokeStatus ?? null
			})),
		(value) =>
			value.state === 'available' &&
			value.installed &&
			!value.nativeStatus?.error &&
			value.nativeStatus?.extent?.width === sender.width &&
			value.nativeStatus?.extent?.height === sender.height,
		15000
	);
	assert.deepEqual(preview.source, { kind: 'Channel', reference });
	assert.deepEqual(preview.nativeStatus.source, preview.source);
	assert.equal(await panel.getAttribute('data-preview-media'), reference.media);
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	const [senderCleanup, nativeCleanup] = await Promise.allSettled([stopSender(), native?.close()]);
	const cleanupErrors = [senderCleanup, nativeCleanup]
		.filter((result) => result.status === 'rejected')
		.map((result) => String(result.reason));
	const resultPath = path.join(directory, 'result.json');
	await writeFile(
		resultPath,
		JSON.stringify(
			{
				result: failure || cleanupErrors.length ? 'fail' : 'pass',
				stage,
				failure,
				cleanupErrors,
				sender,
				senderStopped: senderCleanup.value,
				senderErrors,
				reference,
				initialSender,
				availableOptions,
				selected,
				preview,
				pageErrors,
				appLog: failure ? native?.log() : undefined
			},
			null,
			2
		)
	);
	assert(
		!failure && !cleanupErrors.length,
		`${resultPath}\n${failure ?? cleanupErrors.join('\n')}`
	);
	console.log(
		`NDI Media native preview available at ${sender.width}×${sender.height}. ${resultPath}`
	);
}
