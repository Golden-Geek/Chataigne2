// Ordinary Windows Screen outputs, observed by separate processes using the public NDI SDK.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-ndi-screen');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
let retirements = 0;
const logTails = { stdout: '', stderr: '' };
const nativeOptions = {
	appDataDirectory: path.join(directory, 'profile'),
	environment: { NDI_RUNTIME_DIR_V6: undefined, GOLDEN_NDI_AUTOMATION_SKIP_SYSTEM_RUNTIME: '1' },
	onLogData(data, stream) {
		const lines = (logTails[stream] + data).split('\n');
		logTails[stream] = lines.pop().slice(-4096);
		retirements += lines.filter(
			(line) => line.trim() === '[mapgyver-output] native session retired'
		).length;
	}
};
let native = await openNativeEditor(nativeOptions);
const firstPid = native.app.pid;
const { page } = native;
const pageErrors = [];
const observations = [];
const checks = [];
const receivers = [];
const exerciseAudio = process.argv.includes('--audio');
const exerciseConcurrent = process.argv.includes('--concurrent');
page.on('pageerror', (error) => pageErrors.push(String(error)));
let token, receiver, sender, stage, failure, reopenedPid;

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
	const result = await intent({
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	return result.result.value;
}

async function intent(request) {
	const result = await post('/api/ui/intent', request);
	assert(result.success, JSON.stringify(result));
	return result;
}

async function wait(read, accepts, milliseconds) {
	const deadline = Date.now() + milliseconds;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(100);
	}
}

const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });

function runtimeDirectory() {
	const app = path.resolve(process.env.MAPGYVER_TEST_BINARY ?? 'target/debug/MapGyver.exe');
	return path.join(path.dirname(app), 'ndi');
}

function observe(label, name, colors, tones, { sampleOnly = false } = {}) {
	const receipt = path.join(directory, `${label}.jsonl`);
	const child = spawn(
		'python',
		[
			'-B',
			path.resolve('apps/mapgyver/tests/native/ndi_receiver.py'),
			'--name',
			name,
			'--runtime-dir',
			runtimeDirectory(),
			'--receipt',
			receipt,
			...(tones ? ['--tones', tones] : []),
			...(sampleOnly ? ['--sample-only'] : []),
			...colors
		],
		{ windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] }
	);
	let errors = '';
	let outcome;
	child.stderr.setEncoding('utf8').on('data', (chunk) => (errors += chunk));
	const finished = new Promise((resolve) => {
		child.once('close', (code, signal) => resolve({ code, signal }));
		child.once('error', (error) => resolve({ error: String(error) }));
	}).then((result) => (outcome = result));
	const reader = {
		async at(index) {
			const record = await wait(
				async () => {
					assert(!outcome || outcome.code === 0, `${JSON.stringify(outcome)} ${errors}`);
					const records = await readFile(receipt, 'utf8').catch((error) => {
						if (error.code === 'ENOENT') return '';
						throw error;
					});
					return records.split('\n').slice(0, -1).map(JSON.parse)[index];
				},
				Boolean,
				19500
			);
			observations.push({ label, ...record });
			return record;
		},
		async done() {
			assert.deepEqual(await finished, { code: 0, signal: null }, errors);
		},
		async stop() {
			if (!outcome) child.kill();
			await finished;
		}
	};
	receivers.push(reader);
	return reader;
}

async function attachAudioSource(reference, label) {
	const ready = path.join(directory, 'audio-sender.json');
	const stop = path.join(directory, 'audio-sender.stop');
	const child = spawn(
		'python',
		[
			'-B',
			path.resolve('apps/mapgyver/tests/native/ndi_sender.py'),
			'--name',
			`Audio from ${label}`,
			'--runtime-dir',
			runtimeDirectory(),
			'--ready',
			ready,
			'--stop',
			stop,
			'--pulse-frames',
			'11',
			'--max-seconds',
			'85'
		],
		{ windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] }
	);
	let errors = '';
	let outcome;
	child.stderr.setEncoding('utf8').on('data', (chunk) => (errors += chunk));
	const finished = new Promise((resolve) => {
		child.once('close', (code) => resolve({ code }));
		child.once('error', (error) => resolve({ error: String(error) }));
	}).then((result) => (outcome = result));
	sender = async () => {
		if (!outcome) await writeFile(stop, 'stop\n');
		assert.deepEqual(await finished, { code: 0 }, errors);
	};
	const source = await wait(
		async () => {
			assert(!outcome, `${JSON.stringify(outcome)} ${errors}`);
			try {
				return JSON.parse(await readFile(ready, 'utf8'));
			} catch (error) {
				if (error.code === 'ENOENT' || error instanceof SyntaxError) return null;
				throw error;
			}
		},
		Boolean,
		8000
	);
	await edit('media', {
		kind: 'ConfigureChannel',
		reference,
		source: { kind: 'Ndi', sender: source.name }
	});
	const audio = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.audio',
		operation: 'inspect-output',
		payload: null
	});
	assert.equal(audio.result.value.enabled, false, 'the physical output must remain disabled');
}

async function configureAudio(reference, screen, label, selectScreen) {
	await attachAudioSource(reference, label);
	await selectScreen();
	const inspector = page.locator('.inspector');
	const audioChoices = (title) =>
		inspector
			.locator('.parameter-inspector')
			.filter({ has: page.getByText(title, { exact: true }) })
			.locator('select.dropdown-editor');
	assert.equal(await audioChoices('Right source').inputValue(), 'Unassigned');
	await inspector.getByRole('button', { name: 'Add NDI audio route', exact: true }).click();
	const destination = await wait(
		async () => {
			const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
			const owner = nodes.find((entry) => entry.uuid === screen);
			const route = nodes.find(
				(entry) =>
					owner?.children.includes(entry.node_id) && entry.node_type === 'mapgyver.ndi.audio_route'
			);
			return nodes.find(
				(entry) => route?.children.includes(entry.node_id) && entry.decl_id === 'destination'
			);
		},
		Boolean,
		5000
	);
	const select = inspector.locator(
		`.parameter-inspector[data-node-id="${destination.node_id}"] select`
	);
	await select.selectOption({ label: 'Left' }).catch(async (error) => {
		throw new Error(
			`Screen route ${destination.node_id} is missing from the Inspector: ${await inspector.innerText()}`,
			{ cause: error }
		);
	});
	await audioChoices('Left source').selectOption({ label: 'Right' });
}

function checkPicture(picture, extent, rate) {
	assert.equal(picture.kind, 'picture');
	assert.equal(picture.width, extent.width);
	assert.equal(picture.height, extent.height);
	assert.deepEqual(picture.rate, [rate, 1]);
	assert.equal(picture.pixels.length, 9);
}

async function qualifyConcurrent() {
	const fixture = randomUUID().slice(0, 8);
	const { reference: audio } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'Ndi',
		label: 'NDI stereo fixture'
	});
	stage = 'start the shared stereo NDI source';
	await attachAudioSource(audio, fixture);

	const configure = (screen, target, fps, enabled = true) =>
		edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: { target, enabled, frame_rate: { kind: 'Explicit', fps } }
		});
	const paint = (reference, rgba) =>
		edit('media', { kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
	async function makeOutput(prefix, rgba, extent, fps) {
		const label = `${prefix} ${fixture}`;
		const { reference } = await edit('media', {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label
		});
		await paint(reference, rgba);
		const { screen, surface } = await edit('screens', { kind: 'CreateScreen', label });
		const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference: surface });
		await edit('screens', {
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...snapshot.definition, source: reference }
		});
		const target = { kind: 'Ndi', sender: null, extent };
		await configure(screen, target, fps);
		return { label, screen, reference, target, fps, name: `MapGyver - ${label}` };
	}
	async function route(output, [left, right]) {
		await tab('Screens').click();
		await page
			.getByRole('region', { name: 'Screens', exact: true })
			.getByRole('button', { name: output.label, exact: true })
			.click();
		for (const [parameter, source] of [
			['Left source', left],
			['Right source', right]
		]) {
			await page
				.locator('.inspector')
				.locator('.parameter-inspector')
				.filter({ has: page.getByText(parameter, { exact: true }) })
				.locator('select.dropdown-editor')
				.selectOption({ label: source });
		}
	}
	function checkTones(record, expected) {
		assert.equal(record.kind, 'audio');
		assert.equal(record.channels, expected.length);
		assert.deepEqual(
			record.tones.map(({ hz }) => hz),
			expected
		);
	}
	async function sample(label, output, rgb, tones) {
		const reader = observe(label, output.name, [rgb], tones.join(','), { sampleOnly: true });
		checkPicture(await reader.at(0), output.target.extent, output.fps);
		checkTones(await reader.at(1), tones);
		await reader.done();
	}

	stage = 'receive two differently configured NDI outputs at the same time';
	const first = await makeOutput('NDI first', [1, 0, 0, 1], { width: 320, height: 240 }, 30);
	const second = await makeOutput('NDI second', [0, 0, 1, 1], { width: 240, height: 160 }, 24);
	await route(first, ['Left', 'Right']);
	await route(second, ['Right', 'Left']);
	await Promise.all([
		sample('first-initial', first, '255,0,0', [440, 880]),
		sample('second-initial', second, '0,0,255', [880, 440])
	]);

	stage = 'edit the first stereo route while both senders are active';
	const beforeRoute = retirements;
	await route(first, ['Right', 'Left']);
	await Promise.all([
		sample('first-rerouted', first, '255,0,0', [880, 440]),
		sample('second-after-route', second, '0,0,255', [880, 440])
	]);
	assert.equal(retirements, beforeRoute, 'editing a route must preserve the native renderer');
	checks.push({ stage, rendererRetirements: retirements - beforeRoute });

	stage = 'change the first output format with the second receiver connected';
	const peer = observe(
		'peer-during-format-change',
		second.name,
		['0,0,255', '255,255,0'],
		'880,440',
		{ sampleOnly: true }
	);
	const before = await peer.at(0);
	checkPicture(before, second.target.extent, second.fps);
	const beforeFormat = retirements;
	const updated = {
		...first,
		target: { ...first.target, extent: { width: 384, height: 216 } },
		fps: 25
	};
	await configure(updated.screen, updated.target, updated.fps);
	await paint(first.reference, [0, 1, 0, 1]);
	await paint(second.reference, [1, 1, 0, 1]);
	await Promise.all([
		sample('first-reformatted', updated, '0,255,0', [880, 440]),
		(async () => {
			const after = await peer.at(1);
			checkPicture(after, second.target.extent, second.fps);
			assert(BigInt(after.timecode) > BigInt(before.timecode));
			checkTones(await peer.at(2), [880, 440]);
			await peer.done();
		})()
	]);
	checks.push({
		stage,
		rendererRetirements: retirements - beforeFormat,
		peerReceiverRetained: true
	});

	stage = 'receive fresh video and audio while the app is minimized';
	const minimizedWindow = () =>
		wait(
			() =>
				JSON.parse(
					execFileSync(
						'python',
						['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid), '--main'],
						{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
					)
				),
			(found) => found.length === 1 && found[0].minimized,
			8000
		);
	await page.getByRole('button', { name: 'Minimize app', exact: true }).click();
	const [minimized] = await minimizedWindow();
	await paint(first.reference, [1, 0, 1, 1]);
	await paint(second.reference, [0, 1, 1, 1]);
	await Promise.all([
		sample('first-minimized', updated, '255,0,255', [880, 440]),
		sample('second-minimized', second, '0,255,255', [880, 440])
	]);
	assert.equal((await minimizedWindow())[0].hwnd, minimized.hwnd);
	checks.push({ stage, minimizedWindow: minimized.hwnd });

	stage = 'withdraw the first sender and verify the second remains usable';
	const retired = observe('first-withdrawn', first.name, ['255,0,255']);
	await retired.at(0);
	await configure(updated.screen, updated.target, updated.fps, false);
	assert.equal((await retired.at(1)).kind, 'withdrawn');
	await retired.done();
	await sample('second-after-withdrawal', second, '0,255,255', [880, 440]);
}

async function qualifySingle() {
	const label = `NDI Screen ${randomUUID().slice(0, 8)}`;
	const { screen, surface } = await edit('screens', { kind: 'CreateScreen', label });
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'NDI Screen color'
	});
	const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference: surface });
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...snapshot.definition, source }
	});
	const color = (rgba) =>
		edit('media', {
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba }
		});
	await color([1, 0, 0, 1]);
	const extent = { width: 320, height: 240 };
	const configure = (target, enabled, fps) =>
		edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: { target, enabled, frame_rate: { kind: 'Explicit', fps } }
		});
	await configure({ kind: 'AutomaticSpout', extent }, true, 30);

	stage = 'choose NDI from the ordinary Screen Inspector';
	const selectScreen = async () => {
		await tab('Screens').click();
		await page
			.getByRole('region', { name: 'Screens', exact: true })
			.getByRole('button', { name: label, exact: true })
			.click();
	};
	await selectScreen();
	const picker = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Target', { exact: true }) })
		.locator('select.dropdown-editor');
	await picker.selectOption('NDI');
	assert.equal(await picker.inputValue(), 'NDI');
	assert.equal(
		await page.getByRole('link', { name: 'NDI website' }).getAttribute('href'),
		'https://ndi.video/'
	);
	const automatic = { kind: 'Ndi', sender: null, extent };
	const defaultName = `MapGyver - ${label}`;
	const definition = await wait(
		async () => (await edit('screens', { kind: 'InspectScreen', screen })).definition,
		(value) => value.target?.kind === 'Ndi',
		5000
	);
	assert.equal(definition.enabled, true);
	assert.deepEqual(definition.target, automatic);

	stage = 'receive the automatic sender at 320×240 and 30 fps';
	receiver = observe(
		'automatic',
		defaultName,
		['255,0,0', '0,255,0'],
		exerciseAudio ? '880,0,440' : null
	);
	const red = await receiver.at(0);
	checkPicture(red, extent, 30);

	stage = 'label only the app-owned sender internal in the Media Inspector';
	const { reference: ndiMedia } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'Ndi',
		label: 'Own NDI check'
	});
	await tab('Medias').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Own NDI check', exact: true })
		.click();
	const senderPicker = page
		.locator('.channels-section .parameter-inspector')
		.filter({ has: page.getByText('Sender', { exact: true }) })
		.locator('select.dropdown-editor');
	await senderPicker
		.locator('option', { hasText: `${red.name} (internal)` })
		.waitFor({ state: 'attached' });
	assert.notEqual(await senderPicker.inputValue(), red.name);
	const selected = await edit('media', { kind: 'InspectChannel', reference: ndiMedia });
	assert.notEqual(selected.snapshot.definition.source.sender, red.name);
	if (exerciseAudio) {
		stage = 'route virtual audio from the Screen Inspector with physical output disabled';
		await configureAudio(ndiMedia, screen, label, selectScreen);
	}

	stage = 'keep publishing through editor closure and a live color change';
	for (const panel of [
		'Screen Warping',
		'Surface Source',
		'Composition',
		'Matrix',
		'Media Preview'
	]) {
		while (await tab(panel).count())
			await tab(panel).first().getByRole('button', { name: 'Close panel', exact: true }).click();
	}
	await page.waitForFunction(() => !document.querySelector('.native-preview'));
	await color([0, 1, 0, 1]);
	const green = await receiver.at(1);
	checkPicture(green, extent, 30);
	assert(BigInt(green.timecode) > BigInt(red.timecode));
	if (exerciseAudio) {
		stage = 'receive swapped named routes and unassigned silence on the picture timeline';
		const audio = await receiver.at(2);
		assert.equal(audio.kind, 'audio');
		assert.equal(audio.channels, 3);
		assert.deepEqual(
			audio.tones.map(({ hz }) => hz),
			[880, 440]
		);
		assert.equal(audio.silent_peak, 0);
	}
	const project = path.join(directory, 'NDI Screen on startup.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	await configure(automatic, false, 30);
	assert.equal((await receiver.at(exerciseAudio ? 3 : 2)).kind, 'withdrawn');
	await receiver.done();

	stage = 'publish a custom name at 240×160 and 24 fps';
	const custom = { kind: 'Ndi', sender: `Custom ${label}`, extent: { width: 240, height: 160 } };
	await configure(custom, true, 24);
	receiver = observe('custom', custom.sender, ['0,255,0']);
	checkPicture(await receiver.at(0), custom.extent, 24);
	await edit('screens', { kind: 'RemoveScreen', screen });
	assert.equal((await receiver.at(1)).kind, 'withdrawn');
	await receiver.done();

	stage = 'restart MapGyver and reopen the saved NDI project';
	try {
		await native.close();
	} finally {
		native = undefined;
	}
	native = await openNativeEditor(nativeOptions);
	reopenedPid = native.app.pid;
	assert.notEqual(reopenedPid, firstPid);
	native.page.on('pageerror', (error) => pageErrors.push(String(error)));
	token = undefined;
	token = (await post('/api/ui/session', {})).token;
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	receiver = observe('reopened', defaultName, ['0,255,0'], exerciseAudio ? '880,0,440' : null);
	checkPicture(await receiver.at(0), extent, 30);
	if (exerciseAudio) {
		const audio = await receiver.at(1);
		assert.equal(audio.kind, 'audio');
		assert.deepEqual(
			audio.tones.map(({ hz }) => hz),
			[880, 440]
		);
		assert.equal(audio.silent_peak, 0);
	}
	await edit('screens', { kind: 'RemoveScreen', screen });
	assert.equal((await receiver.at(exerciseAudio ? 2 : 1)).kind, 'withdrawn');
	await receiver.done();
}

try {
	token = (await post('/api/ui/session', {})).token;
	if (exerciseConcurrent) await qualifyConcurrent();
	else await qualifySingle();
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	const cleanup = await Promise.allSettled([
		...receivers.map((reader) => reader.stop()),
		sender?.(),
		native?.close()
	]);
	const cleanupErrors = cleanup
		.filter((result) => result.status === 'rejected')
		.map((result) => String(result.reason));
	const report = path.join(directory, 'result.json');
	await writeFile(
		report,
		JSON.stringify(
			{
				result: failure || cleanupErrors.length ? 'fail' : 'pass',
				stage,
				firstPid,
				reopenedPid,
				observations,
				checks,
				pageErrors,
				failure,
				cleanupErrors,
				appLog: failure ? native?.log() : undefined
			},
			null,
			2
		)
	);
	assert(!failure, `${report}\n${failure}`);
	assert.deepEqual(cleanupErrors, [], report);
	console.log(report);
}
