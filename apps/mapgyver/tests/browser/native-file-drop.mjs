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
const editorDrops = process.argv.includes('--editors');
const editors = {};

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
const edit = (payload, namespace = 'mapgyver.media') =>
	intent({
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
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
const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
const showMedia = () => tab('Medias').click();

async function verifyEditors(drop) {
	const screenEdit = (payload) => edit(payload, 'mapgyver.screens');
	const readScreens = async () => (await screenEdit({ kind: 'InspectManager' })).snapshot.screens;
	const readFile = async (reference) =>
		(await edit({ kind: 'InspectChannel', reference })).snapshot?.definition.source.file?.path;
	const readSurface = async (reference) =>
		(await screenEdit({ kind: 'InspectSurface', reference })).snapshot?.definition;

	stage = 'prepare the Screen and one partial Surface';
	const [screen] = await readScreens();
	const [surface] = screen.surfaces;
	await screenEdit({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			enabled: false,
			target: {
				kind: 'Rectangle',
				bounds: { left: 0, top: 0, extent: { width: 640, height: 360 } }
			}
		}
	});
	await screenEdit({
		kind: 'ConfigureSurface',
		reference: surface.reference,
		definition: {
			...surface.definition,
			opacity: 0.6,
			quad: { corners: surface.definition.quad.corners.map(({ x, y }) => ({ x: x * 0.42, y })) }
		}
	});
	const before = await readSurface(surface.reference);
	assert.equal(before.source, null);
	await tab('Screens').click();
	await page
		.getByRole('region', { name: 'Screens', exact: true })
		.getByRole('button', { name: screen.label, exact: true })
		.click();
	await tab('Screen Warping').click();
	await page.locator('[data-panel-id="screen-warp"][aria-busy="false"]').waitFor();
	const warp = page.locator('[data-screen-drop-target]');
	const mapping = warp.locator('svg[aria-label="Surface mapping controls"]');
	await mapping.waitFor();

	stage = 'assign the dropped file to the hit Surface';
	await drop(warp, mapping, original, `Assign file to ${surface.label}`, [0.22, 0.55]);
	const assigned = await wait(
		() => readSurface(surface.reference),
		(value) => !!value?.source
	);
	assert.deepEqual(assigned, { ...before, source: assigned.source });
	assert.equal(await readFile(assigned.source), original);
	assert.equal((await readScreens())[0].surfaces.length, 1);
	await intent({ kind: 'undo' });
	assert.deepEqual(await readSurface(surface.reference), before);
	assert.equal(await readFile(assigned.source), undefined);
	await intent({ kind: 'redo' });
	assert.deepEqual(await readSurface(surface.reference), assigned);

	stage = 'add a Surface and its Media on the Screen background';
	await drop(warp, mapping, replacement, 'Add Surface from file', [0.77, 0.55]);
	const newSurface = await wait(async () => (await readScreens())[0].surfaces[1], Boolean);
	assert.equal(await readFile(newSurface.definition.source), replacement);
	assert.deepEqual(await readSurface(surface.reference), assigned);
	await intent({ kind: 'undo' });
	assert.equal((await readScreens())[0].surfaces.length, 1);
	assert.equal(await readFile(newSurface.definition.source), undefined);
	await intent({ kind: 'redo' });
	assert.deepEqual(await readSurface(newSurface.reference), newSurface.definition);
	editors.screen = { screen: screen.id, assigned: surface.reference, added: newSurface.reference };

	stage = 'prepare a Composition with an existing child';
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'File drop Composition'
	});
	const compositionEdit = (operation) =>
		edit({ kind: 'Composition', reference: composition, edit: operation });
	await compositionEdit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Existing child',
		position: { x: 960, y: 540 }
	});
	const readComposition = async () => (await compositionEdit({ kind: 'Inspect' })).snapshot;
	const originalComposition = await readComposition();
	await showMedia();
	await manager.getByRole('button', { name: 'File drop Composition', exact: true }).click();
	await tab('Composition').click();
	const spatial = page.locator(
		`[data-composition-channel="${composition.channel}"][data-composition-busy="false"]`
	);
	const compositionArea = spatial.locator('[data-composition-drop-target]');
	const controls = compositionArea.locator('svg[aria-label="Composition transform controls"]');
	await controls.waitFor();

	stage = 'drop over the existing child and append an independent Media';
	await drop(compositionArea, controls, original, 'Add Media to Composition', [0.5, 0.5]);
	const importedComposition = await wait(readComposition, (value) => value.instances.length === 2);
	const [existing, created] = importedComposition.instances;
	assert.deepEqual(existing, originalComposition.instances[0]);
	assert.equal(created.definition.source.media, created.reference.instance);
	assert.equal(await readFile(created.definition.source), original);
	const [x, y] = created.definition.transform.position;
	assert(Math.abs(x - importedComposition.canvas.width / 2) <= 4, JSON.stringify([x, y]));
	assert(Math.abs(y - importedComposition.canvas.height / 2) <= 4, JSON.stringify([x, y]));
	await intent({ kind: 'undo' });
	assert.deepEqual(await readComposition(), originalComposition);
	assert.equal(await readFile(created.definition.source), undefined);
	await intent({ kind: 'redo' });
	assert.deepEqual(await readComposition(), importedComposition);
	editors.composition = { composition, added: created.reference, position: [x, y] };

	stage = 'reload the assigned Screen and the owned Composition child';
	const project = path.join(directory, 'Editor file drops.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual(await readSurface(surface.reference), assigned);
	assert.deepEqual(await readSurface(newSurface.reference), newSurface.definition);
	assert.deepEqual(await readComposition(), importedComposition);
	assert.equal(await readFile(created.definition.source), original);
}

try {
	token = (await post('/api/ui/session', {})).token;
	await showMedia();
	await page.evaluate(() => {
		window.observedFileDrop = [];
		document.addEventListener(
			'dragover',
			(event) => {
				const area = event.target.closest(
					'[data-screen-drop-target], [data-composition-drop-target]'
				);
				if (area)
					window.observedFileDrop.push({
						editor: event.target.tagName,
						types: Array.from(event.dataTransfer?.types ?? []),
						point: [event.clientX, event.clientY]
					});
			},
			{ capture: true }
		);
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
	async function drop(area, locator, file, label, [x, y] = [0.5, 0.5]) {
		const active = area.locator('.file-drop-feedback.active');
		const box = await locator.boundingBox();
		assert(box);
		const target = {
			x: box.x + box.width * x,
			y: box.y + box.height * y,
			data: { items: [], files: [file], dragOperationsMask: 1 }
		};
		const hit = await area.evaluate(
			(element, { x, y }) => {
				const pointer = document.elementFromPoint(x, y);
				return {
					inside: element.contains(pointer),
					tag: pointer?.tagName,
					class: pointer?.getAttribute('class'),
					area: element.getBoundingClientRect().toJSON()
				};
			},
			{ x: target.x, y: target.y }
		);
		assert(hit.inside, `Drop point misses its editor: ${JSON.stringify({ box, hit })}`);
		await cdp.send('Input.dispatchDragEvent', { ...target, type: 'dragEnter' });
		await wait(
			async () => {
				await cdp.send('Input.dispatchDragEvent', { ...target, type: 'dragOver' });
				return (await active.count()) ? active.innerText() : null;
			},
			(text) => text === label
		);
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
	await drop(manager, manager, original, 'Import Media', [0.5, 0.85]);
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
	await drop(manager, row, replacement, `Replace file in ${label}`);
	await wait(authored, (entry) => currentFile(entry) === replacement);
	assert.equal((await authored()).id, media.id);
	await intent({ kind: 'undo' });
	assert.equal(currentFile(await authored()), original);
	await intent({ kind: 'redo' });
	assert.equal(currentFile(await authored()), replacement);

	stage = 'reject an unsupported file and preserve history';
	const readHistory = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).history;
	const history = await readHistory();
	await drop(manager, row, unsupported, `Replace file in ${label}`);
	await manager.getByText(/This file is not a supported picture or video/).waitFor();
	assert.deepEqual(await readHistory(), history);
	assert.equal(currentFile(await authored()), replacement);
	if (editorDrops) await verifyEditors(drop);

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
			editors: editorDrops ? editors : undefined,
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
