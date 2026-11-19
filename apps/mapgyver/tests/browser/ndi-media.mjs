// Real NDI sender, native MapGyver authoring, and exact-channel native preview status.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
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
const providedName = `MapGyver-NDI-Media-${randomUUID()}`;
const exerciseAudio = process.argv.includes('--audio');
const expectBundledRuntime = process.argv.includes('--bundled-runtime');
const initialFeed = launchSender('sender');
let resumedFeed;

const pageErrors = [];
const audioChecks = [];
let native, token, sender, reference, initialSender, selected, availableOptions, preview, failure;
let runtimeModule;
let stage = 'start owned NDI sender';
async function wait(read, accepts, milliseconds = 12000) {
	const deadline = Date.now() + milliseconds;
	for (;;) {
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
function launchSender(label) {
	const readyPath = path.join(directory, `${label}-ready.json`);
	const stopPath = path.join(directory, `${label}-stop`);
	const child = spawn(
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
			'90',
			...(expectBundledRuntime
				? ['--runtime-dir', path.resolve(path.dirname(process.env.MAPGYVER_TEST_BINARY), 'ndi')]
				: []),
			...(exerciseAudio ? ['--pulse-frames', '11'] : [])
		],
		{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }
	);
	let log = '',
		errors = '',
		exit,
		outcome;
	child.stdout.setEncoding('utf8').on('data', (value) => {
		log = (log + value).slice(-16000);
	});
	child.stderr.setEncoding('utf8').on('data', (value) => {
		errors = (errors + value).slice(-16000);
	});
	const exited = new Promise((resolve) => {
		child.once('error', (error) => resolve({ error: String(error) }));
		child.once('close', (code, signal) => resolve({ code, signal }));
	}).then((value) => (exit = value));
	return {
		async ready() {
			const record = await wait(
				async () => {
					assert(!exit, `owned NDI sender exited: ${JSON.stringify(exit)} ${errors}`);
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
			assert.equal(record.kind, 'ready');
			assert.equal(record.pid, child.pid);
			assert.equal(record.provided_name, providedName);
			assert(record.name.endsWith(`(${providedName})`));
			return record;
		},
		async stop() {
			if (outcome) return outcome;
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
				assert.equal(status.code, 0, JSON.stringify({ status, errors }));
				outcome = JSON.parse(log.trim().split(/\r?\n/).at(-1));
				assert.equal(outcome.kind, 'stopped');
				assert.equal(outcome.reason, 'marker');
				assert(outcome.frames > 0);
				return outcome;
			} finally {
				clearTimeout(timer);
				if (!exit) {
					child.kill();
					await exited;
				}
			}
		}
	};
}

async function verifyAudio(page, media, reference, tab) {
	const intent = async (request) => {
		const result = await post('/api/ui/intent', request);
		assert(result.success, JSON.stringify(result));
		return result;
	};
	const snapshot = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
	const inspectAudio = async () => {
		const result = await intent({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.audio',
			operation: 'inspect-output',
			payload: null
		});
		assert.equal(result.result.value.enabled, false, 'fixture must not open a physical output');
		return Object.fromEntries(
			result.result.value.output_levels.map(({ channel, rms_linear }) => [channel, rms_linear])
		);
	};
	const lookup = (nodes, parent, decl) => {
		const children = (parent.children ?? []).map((id) =>
			nodes.find((entry) => entry.node_id === id)
		);
		return (
			children.find((entry) => entry.decl_id.split('/').at(-1) === decl) ??
			children.map((entry) => lookup(nodes, entry, decl)).find(Boolean)
		);
	};
	const prefs = (await snapshot()).find(
		(entry) => entry.node_type === 'mapgyver.preferences.audio'
	);
	const created = await intent({
		kind: 'createUserItem',
		parent: prefs.node_id,
		node_type: 'mapgyver.preferences.audio.output',
		label: 'NDI auxiliary',
		initial_params: []
	});
	const nodes = await snapshot();
	const auxNode = nodes.find((entry) => entry.node_id === created.created_node.node_id);
	assert.equal(auxNode.meta.label, 'NDI auxiliary');
	const first = '8fe83692-442b-4bc0-92b8-87256dfe0001';
	const second = '8fe83692-442b-4bc0-92b8-87256dfe0002';
	const channel = nodes.find((entry) => entry.uuid === reference.channel);
	const folder = lookup(nodes, channel, 'video_audio');
	const gain = lookup(nodes, channel, 'audio_gain');
	await tab('Medias').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: media.label, exact: true })
		.click();
	for (const { node_id } of [channel, folder]) {
		const title = page.locator(
			`.channels-section .node-inspector[data-node-id="${node_id}"] > .node-header > .node-title`
		);
		if ((await title.getAttribute('aria-checked')) === 'false') await title.press('Enter');
	}
	const left = lookup(nodes, channel, 'audio_left');
	const choice = page.locator(`.parameter-inspector[data-node-id="${left.node_id}"] select`);
	const namedOutput = choice.locator('option', { hasText: 'NDI auxiliary' });
	await namedOutput.waitFor({ state: 'attached' });
	const auxiliary = await namedOutput.getAttribute('value');
	assert(auxiliary);
	await choice.selectOption(auxiliary);
	assert.equal(await choice.inputValue(), auxiliary);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Warping\b/ })
		.click();
	await page.locator('[data-panel-id="media-preview"]').waitFor({ state: 'hidden' });
	await delay(500);
	const record = async (label) => {
		const peak = {};
		const end = Date.now() + 2800;
		while (Date.now() < end) {
			for (const [lane, value] of Object.entries(await inspectAudio()))
				peak[lane] = Math.max(peak[lane] ?? 0, value);
			await delay(75);
		}
		audioChecks.push({ label, peak });
		return peak;
	};
	const normal = await record('independent left and right pulses with preview closed');
	assert(normal[auxiliary] > 0.003 && normal[second] > 0.003, JSON.stringify(normal));
	assert(normal[first] < 0.0001, JSON.stringify(normal));
	await intent({
		kind: 'setParam',
		node: gain.node_id,
		value: { Float: -12 },
		behaviour: 'Append'
	});
	await delay(450);
	const quieter = await record('channel gain');
	for (const lane of [auxiliary, second]) {
		assert(quieter[lane] > 0.0005, JSON.stringify(quieter));
		assert(quieter[lane] / normal[lane] < 0.4, JSON.stringify({ normal, quieter }));
	}
	const toggle = page
		.locator(`.channels-section .node-inspector[data-node-id="${folder.node_id}"] > .node-header`)
		.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	await toggle.click();
	await wait(
		() => toggle.getAttribute('aria-checked'),
		(checked) => checked === 'false'
	);
	await delay(850);
	const muted = await record('audio folder mute');
	assert(muted[auxiliary] < 0.0001 && muted[second] < 0.0001, JSON.stringify(muted));
	await toggle.click();
	await wait(inspectAudio, (levels) => levels[auxiliary] > 0.0005 || levels[second] > 0.0005, 6500);
	await initialFeed.stop();
	await delay(1200);
	const missing = await record('sender loss');
	assert(missing[auxiliary] < 0.0001 && missing[second] < 0.0001, JSON.stringify(missing));
	assert.equal(
		(await inspect({ kind: 'InspectChannel', reference })).definition.source.sender,
		sender.name
	);
	stage = 'recover audio after the same sender name returns';
	resumedFeed = launchSender('sender-reconnected');
	const resumed = await resumedFeed.ready();
	assert.equal(resumed.name, sender.name);
	assert.notEqual(resumed.pid, sender.pid);
	await wait(inspectAudio, (levels) => levels[auxiliary] > 0.0005 || levels[second] > 0.0005);
	const recovered = await record('same sender recovers both routes');
	assert(recovered[auxiliary] > 0.0005 && recovered[second] > 0.0005, JSON.stringify(recovered));
}

try {
	sender = await initialFeed.ready();

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
	await page.getByRole('link', { name: 'NDI website' }).waitFor();
	assert.equal(
		await page.getByRole('link', { name: 'NDI website' }).getAttribute('href'),
		'https://ndi.video/'
	);
	const picker = page
		.locator('.channels-section .parameter-inspector')
		.filter({ has: page.getByText('Sender', { exact: true }) })
		.locator('select.dropdown-editor');
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
	if (expectBundledRuntime) {
		stage = 'verify the Windows loader uses the app-local NDI runtime';
		const filename = 'Processing.NDI.Lib.x64.dll';
		runtimeModule = execFileSync(
			'powershell.exe',
			[
				'-NoProfile',
				'-NonInteractive',
				'-Command',
				`(Get-Process -Id ${native.app.pid} -Module -ErrorAction Stop | Where-Object ModuleName -eq '${filename}').FileName`
			],
			{ encoding: 'utf8', windowsHide: true }
		).trim();
		assert.equal(
			path.normalize(runtimeModule).toLowerCase(),
			path.resolve(path.dirname(process.env.MAPGYVER_TEST_BINARY), 'ndi', filename).toLowerCase()
		);
	}
	if (exerciseAudio) {
		stage = 'verify NDI audio gain, mute and sender loss on named virtual outputs';
		await verifyAudio(page, media, reference, tab);
	}
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	const [senderCleanup, resumedCleanup, nativeCleanup] = await Promise.allSettled([
		initialFeed.stop(),
		resumedFeed?.stop(),
		native?.close()
	]);
	const cleanupErrors = [senderCleanup, resumedCleanup, nativeCleanup]
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
				resumedSenderStopped: resumedCleanup.value,
				reference,
				initialSender,
				availableOptions,
				selected,
				preview,
				runtimeModule,
				audioChecks,
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
