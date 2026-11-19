// One ordinary Windows Screen, observed by a separate process using the public NDI SDK.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-ndi-screen');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const native = await openNativeEditor();
const { page } = native;
const pageErrors = [];
const observations = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
let token, receiver, stage, failure;

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
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
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

function observe(label, name, ...colors) {
	const receipt = path.join(directory, `${label}.jsonl`);
	const app = path.resolve(process.env.MAPGYVER_TEST_BINARY ?? 'target/debug/MapGyver.exe');
	const child = spawn(
		'python',
		[
			'-B',
			path.resolve('apps/mapgyver/tests/native/ndi_receiver.py'),
			'--name',
			name,
			'--runtime-dir',
			path.join(path.dirname(app), 'ndi'),
			'--receipt',
			receipt,
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
	return {
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
}

function checkPicture(picture, extent, rate) {
	assert.equal(picture.kind, 'picture');
	assert.equal(picture.width, extent.width);
	assert.equal(picture.height, extent.height);
	assert.deepEqual(picture.rate, [rate, 1]);
	assert.equal(picture.pixels.length, 9);
}

try {
	token = (await post('/api/ui/session', {})).token;
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
	await configure({ kind: 'Rectangle', bounds: { left: 0, top: 0, extent } }, false, 30);

	stage = 'choose NDI from the ordinary Screen Inspector';
	await tab('Screens').click();
	await page
		.getByRole('region', { name: 'Screens', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
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
	await configure(automatic, true, 30);

	stage = 'receive the automatic sender at 320×240 and 30 fps';
	receiver = observe('automatic', defaultName, '255,0,0', '0,255,0');
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
	await configure(automatic, false, 30);
	assert.equal((await receiver.at(2)).kind, 'withdrawn');
	await receiver.done();

	stage = 'publish a custom name at 240×160 and 24 fps';
	const custom = { kind: 'Ndi', sender: `Custom ${label}`, extent: { width: 240, height: 160 } };
	await configure(custom, true, 24);
	receiver = observe('custom', custom.sender, '0,255,0');
	checkPicture(await receiver.at(0), custom.extent, 24);
	await edit('screens', { kind: 'RemoveScreen', screen });
	assert.equal((await receiver.at(1)).kind, 'withdrawn');
	await receiver.done();
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	await receiver?.stop();
	await native.close();
	const report = path.join(directory, 'result.json');
	await writeFile(
		report,
		JSON.stringify(
			{
				result: failure ? 'fail' : 'pass',
				stage,
				observations,
				pageErrors,
				failure,
				appLog: failure ? native.log() : undefined
			},
			null,
			2
		)
	);
	assert(!failure, `${report}\n${failure}`);
	console.log(report);
}
