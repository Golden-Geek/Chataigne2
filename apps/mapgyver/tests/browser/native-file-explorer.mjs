import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-file-explorer');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const projectFolder = path.join(directory, 'Project');
const treeFolder = path.join(projectFolder, 'Still images');
const settingsFolder = path.join(directory, 'Settings override');
const pickedFolder = path.join(directory, 'Chosen folder');
const project = path.join(projectFolder, 'Explorer project.mapgyver');
const treeImage = path.join(treeFolder, 'Project still.png');
const importedImage = path.join(pickedFolder, 'Projector café.png');
await Promise.all(
	[treeFolder, settingsFolder, pickedFolder].map((folder) => mkdir(folder, { recursive: true }))
);
await Promise.all(
	[treeImage, importedImage].map((file) => copyFile('apps/mapgyver/icons/icon.png', file))
);

const native = await openNativeEditor();
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const observations = [];
let token, stage, failure, workbench;

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

async function intent(body) {
	const ack = await post('/api/ui/intent', body);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}

function edit(payload, namespace = 'mapgyver.media') {
	return intent({ kind: 'invokeAppOperation', namespace, operation: 'edit', payload });
}

async function wait(read, accepts) {
	const deadline = Date.now() + 6000;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(60);
	}
}

const explorer = page.locator('section[aria-label="File Explorer"]');
const medias = page.locator('section[data-panel-id="media-manager"]');
const folderLabel = explorer.locator('[aria-label="Current folder"]');
const file = (name) => explorer.locator('[data-explorer-file]').filter({ hasText: name });

async function expectFolder(folder) {
	await wait(
		() => folderLabel.textContent(),
		(text) => text?.trim() === folder
	);
}

async function loadProject() {
	assert.equal((await post('/api/ui/project-load', { path: project })).path, project);
	await wait(
		() => page.locator('.project-title').textContent(),
		(text) => text?.trim().replace(/\*$/, '') === path.basename(project)
	);
}

try {
	token = (await post('/api/ui/session', {})).token;
	stage = 'open File Explorer in a fresh Windows workbench';
	await explorer.getByText('Save the project or choose a folder.', { exact: true }).waitFor();
	const { snapshot } = await edit({ kind: 'InspectManager' }, 'mapgyver.screens');
	const screen = snapshot.screens[0];
	await edit(
		{
			kind: 'ConfigureScreen',
			screen: screen.id,
			definition: { ...screen.definition, enabled: false }
		},
		'mapgyver.screens'
	);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await loadProject();
	await expectFolder(projectFolder);

	stage = 'expand a nested folder in Tree View';
	await explorer.getByRole('button', { name: 'Tree View', exact: true }).click();
	const branch = explorer.getByRole('button', { name: 'Still images', exact: true });
	await branch.click();
	await file('Project still.png').waitFor();
	assert.equal(await branch.getAttribute('aria-expanded'), 'true');
	assert.equal(await file('Project still.png').getAttribute('data-explorer-file'), treeImage);
	observations.push(
		'Opening a saved project uses its containing folder and lists nested files on expansion'
	);

	stage = 'browse the same subfolder in Thumbnail View and decode its PNG';
	await explorer.getByRole('button', { name: 'Thumbnail View', exact: true }).click();
	await explorer.getByRole('button', { name: 'Still images', exact: true }).click();
	await expectFolder(treeFolder);
	const preview = file('Project still.png').locator('[data-image-thumbnail] img');
	await preview.waitFor();
	await wait(
		() => preview.evaluate((image) => image.naturalWidth),
		(width) => width > 0 && width <= 144
	);
	await explorer.getByRole('button', { name: 'Up one folder', exact: true }).click();
	await expectFolder(projectFolder);
	observations.push(
		'Thumbnail View displayed a decoded local image and navigated back to the project folder'
	);

	stage = 'change the File Explorer root through the typed Project Settings editor';
	await page.getByRole('button', { name: 'Project Settings', exact: true }).click();
	const field = page.getByRole('textbox', { name: 'Project Folder', exact: true });
	await field.fill(settingsFolder);
	await field.press('Enter');
	await expectFolder(settingsFolder);

	stage = 'choose a folder from the panel and save its typed value';
	await page.evaluate((folder) => {
		const endpoint = window.__TAURI_INTERNALS__.convertFileSrc('open_file_dialog', 'ipc');
		const original = window.fetch;
		window.fetch = (resource, options) => {
			if (resource === endpoint) {
				window.fetch = original;
				window.observedExplorerFolderDialog = JSON.parse(options.body);
				return Promise.resolve(
					new Response(JSON.stringify(folder), {
						headers: { 'Content-Type': 'application/json', 'Tauri-Response': 'ok' }
					})
				);
			}
			return original(resource, options);
		};
	}, pickedFolder);
	await explorer.getByRole('button', { name: 'Choose project folder', exact: true }).click();
	await expectFolder(pickedFolder);
	assert.equal(await page.evaluate(() => window.observedExplorerFolderDialog?.directory), true);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	await explorer.getByText('Save the project or choose a folder.', { exact: true }).waitFor();
	await loadProject();
	await expectFolder(pickedFolder);
	observations.push(
		'The panel dispatched a directory picker and saved the returned override in the project'
	);

	stage = 'drag a real local file from File Explorer into Medias';
	await explorer.getByRole('button', { name: 'Tree View', exact: true }).click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	const source = file('Projector café.png');
	await source.waitFor();
	const bounds = await medias.boundingBox();
	assert(bounds);
	await source.dragTo(medias, { targetPosition: { x: bounds.width / 2, y: bounds.height * 0.75 } });
	const imported = await wait(
		async () =>
			(await edit({ kind: 'InspectManager' })).snapshot.media.find(
				(media) => media.label === 'Projector café'
			),
		Boolean
	);
	assert.equal(imported.media_kind, 'Picture');
	assert.equal(imported.channels[0].definition.source.file.path, importedImage);
	await intent({ kind: 'undo' });
	assert(!(await edit({ kind: 'InspectManager' })).snapshot.media.length);
	observations.push(
		'Dragging an Explorer file created owned Picture Media with the exact native path and one Undo'
	);
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	workbench = await page
		.evaluate(() => ({
			tabs: Array.from(document.querySelectorAll('[role=tab]'), (tab) => tab.textContent),
			visibleText: document.body.innerText.slice(0, 2000)
		}))
		.catch(() => undefined);
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
			observations,
			errors,
			failure,
			workbench,
			log: failure ? native.log() : undefined
		},
		null,
		2
	)
);
assert(!failure, `${result}\n${failure}`);
console.log(result);
