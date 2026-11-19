// Supply disk-backed File objects through CDP to the bundled Windows WebView2.
import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-native-file-drop');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const original = path.join(directory, 'Projector café.png');
const replacement = path.join(directory, 'Replacement.png');
const unsupported = path.join(directory, 'unsupported.txt');
for (const file of [original, replacement]) await copyFile('apps/mapgyver/icons/icon.png', file);
await writeFile(unsupported, 'This is a local unsupported file fixture.\n');
const native = await openNativeEditor();
const { page } = native;
const errors = [];
let observed;
page.on('pageerror', (error) => errors.push(String(error)));
let token, stage, failure, media;

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
async function intent(request) {
	const result = await post('/api/ui/intent', request);
	assert(result.success, JSON.stringify(result));
	return result.result?.value;
}
async function inspect() {
	const result = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload: { kind: 'InspectManager' }
	});
	return result.snapshot;
}
async function wait(read, accepts, timeout = 5000) {
	const deadline = Date.now() + timeout;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(60);
	}
}

const manager = page.locator('section[data-panel-id="media-manager"]');
const active = manager.locator('.file-drop-feedback.active');
const showMedia = () =>
	page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();

try {
	token = (await post('/api/ui/session', {})).token;
	await showMedia();
	await page.evaluate(() => {
		window.observedFileDrop = [];
		document.addEventListener(
			'drop',
			(event) => {
				window.observedFileDrop.push({
					files: Array.from(event.dataTransfer?.files ?? [], (file) => file.name),
					types: Array.from(event.dataTransfer?.types ?? [])
				});
			},
			{ capture: true }
		);
		window.chrome?.webview?.addEventListener('message', (event) => {
			if (event.data?.kind === 'golden:file-drop')
				window.observedFileDrop.push({ pathCount: event.data.paths.length });
		});
	});
	const cdp = await page.context().newCDPSession(page);
	async function drop(locator, file, label, background = false) {
		const box = await locator.boundingBox();
		assert(box);
		const target = {
			x: box.x + box.width / 2,
			y: box.y + box.height * (background ? 0.85 : 0.5),
			data: { items: [], files: [file], dragOperationsMask: 1 }
		};
		for (const type of ['dragEnter', 'dragOver']) {
			await cdp.send('Input.dispatchDragEvent', { ...target, type });
		}
		assert.equal(await active.innerText(), label);
		await cdp.send('Input.dispatchDragEvent', { ...target, type: 'drop' });
		await active.waitFor({ state: 'detached' });
	}
	const label = 'Projector café';
	const authored = async () => (await inspect()).media.find((entry) => entry.label === label);
	const currentFile = (entry) => entry?.channels[0].definition.source.file?.path;
	const row = manager.locator('.outliner-item-content').filter({
		has: page.getByRole('button', { name: label, exact: true })
	});

	stage = 'native path and background import';
	assert.equal(await authored(), undefined);
	await drop(manager, original, 'Import Media', true);
	media = await wait(
		async () => {
			const message = await manager
				.locator('.file-drop-error')
				.textContent({ timeout: 60 })
				.catch(() => null);
			assert(!message, message);
			return authored();
		},
		(entry) => currentFile(entry) === original
	);
	assert.equal(media.media_kind, 'Picture');
	await intent({ kind: 'undo' });
	assert.equal(await authored(), undefined);
	await intent({ kind: 'redo' });
	assert.equal((await authored()).id, media.id);

	stage = 'replace file on the exact Media row with one undo step';
	await row.waitFor();
	await drop(row, replacement, `Replace file in ${label}`);
	await wait(authored, (entry) => currentFile(entry) === replacement);
	assert.equal((await authored()).id, media.id);
	await intent({ kind: 'undo' });
	assert.equal(currentFile(await authored()), original);
	await intent({ kind: 'redo' });
	assert.equal(currentFile(await authored()), replacement);

	stage = 'reject an unsupported file and preserve history';
	const readHistory = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).history;
	const history = await readHistory();
	await drop(row, unsupported, `Replace file in ${label}`);
	await manager.getByText(/This file is not a supported picture or video/).waitFor();
	assert.deepEqual(await readHistory(), history);
	assert.equal(currentFile(await authored()), replacement);

	stage = 'save and reload the same path and Media identity';
	const project = path.join(directory, 'Native files.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	assert.equal(await authored(), undefined);
	await post('/api/ui/project-load', { path: project });
	assert.equal((await authored()).id, media.id);
	assert.equal(currentFile(await authored()), replacement);
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	observed = await page.evaluate(() => window.observedFileDrop).catch(() => null);
}

try {
	await native.close();
} catch (error) {
	failure = [failure, String(error.stack ?? error)].filter(Boolean).join('\n');
}
const result = path.join(directory, 'result.json');
await writeFile(
	result,
	JSON.stringify(
		{
			result: failure ? 'fail' : 'pass',
			stage,
			media: media?.id,
			errors,
			observed,
			failure,
			log: failure ? native.log() : undefined
		},
		null,
		2
	)
);
assert(!failure, `${result}\n${failure}`);
console.log(result);
