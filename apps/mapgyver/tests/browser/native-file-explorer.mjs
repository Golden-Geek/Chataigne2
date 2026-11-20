import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-file-explorer');
const onlineIsf = process.argv.includes('--online-isf');
const onlineFragCoord = process.argv.includes('--online-fragcoord');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const projectFolder = path.join(directory, 'Project');
const treeFolder = path.join(projectFolder, 'Still images');
const settingsFolder = path.join(directory, 'Settings override');
const pickedFolder = path.join(directory, 'Chosen folder');
const project = path.join(projectFolder, 'Explorer project.mapgyver');
const treeImage = path.join(treeFolder, 'Project still.png');
const importedImage = path.join(pickedFolder, 'Projector café.png');
const shaderFolder = path.join(pickedFolder, 'Local shaders');
const shaders = [
	{
		name: 'Local Glow',
		suffix: '.GLSL',
		format: 'GLSL',
		code: 'void main() { fragColor = vec4(1.0, 0.0, 1.0, 1.0); }',
		rgba: [255, 0, 255, 255]
	},
	{
		name: 'Local Grid',
		suffix: '.fs',
		format: 'ISF',
		code: '/*{"ISFVSN":"2"}*/ void main() { gl_FragColor = vec4(0.0, 1.0, 1.0, 1.0); }',
		rgba: [0, 255, 255, 255]
	},
	{
		name: 'Local Orbit',
		suffix: '.shadertoy.json',
		format: 'ShaderToy',
		code: JSON.stringify({
			renderpass: [
				{
					type: 'image',
					code: 'void mainImage(out vec4 c, in vec2 p) { c = vec4(1.0, 1.0, 0.0, 1.0); }'
				}
			]
		}),
		rgba: [255, 255, 0, 255]
	}
];
await Promise.all(
	[treeFolder, settingsFolder, shaderFolder].map((folder) => mkdir(folder, { recursive: true }))
);
await Promise.all(
	[treeImage, importedImage].map((file) => copyFile('apps/mapgyver/icons/icon.png', file))
);
await Promise.all([
	...shaders.map(({ name, suffix, code }) =>
		writeFile(path.join(shaderFolder, name + suffix), code)
	),
	writeFile(path.join(shaderFolder, 'Project notes.txt'), 'notes'),
	writeFile(path.join(shaderFolder, 'Configuration.json'), '{}')
]);

let appLog = '';
const native = await openNativeEditor({
	onLogData: (chunk) => (appLog += chunk)
});
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const observations = [];
let token, stage, failure, workbench, shaderObserver, isfCatalog, fragCoordCatalog;

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

async function wait(read, accepts, maximum = 6000) {
	const deadline = Date.now() + maximum;
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

	stage = 'browse only shader formats in the local Shader Library';
	await page
		.getByRole('tab')
		.filter({ hasText: /^Shader Library$/ })
		.click();
	const library = page.getByRole('region', { name: 'Shader Library', exact: true });
	await library.getByRole('button', { name: 'Tree View', exact: true }).click();
	await library.getByRole('button', { name: 'Local shaders', exact: true }).click();
	const shaderRows = library.locator('[data-explorer-file]');
	await wait(
		() => shaderRows.count(),
		(count) => count === shaders.length
	);
	assert.equal(await library.getByText('Configuration.json', { exact: true }).count(), 0);
	assert.equal(await library.getByText('Project notes.txt', { exact: true }).count(), 0);
	assert.equal(await library.getByText('Projector café.png', { exact: true }).count(), 0);
	shaderObserver = await createPreviewOutputObserver(
		post,
		path.join(directory, 'shader-pixels'),
		page
	);
	let outputBinding;
	for (const [index, shader] of shaders.entries()) {
		stage = `drag and render ${shader.format} from the Shader Library`;
		const nativePath = path.join(shaderFolder, shader.name + shader.suffix);
		const row = shaderRows.filter({ hasText: shader.name + shader.suffix });
		assert.equal(await row.getAttribute('data-explorer-file'), nativePath);
		await row.dragTo(medias, { targetPosition: { x: bounds.width / 2, y: bounds.height * 0.9 } });
		const readMedia = async () =>
			(await edit({ kind: 'InspectManager' })).snapshot.media.find(
				(media) => media.label === shader.name
			);
		const media = await wait(readMedia, Boolean);
		assert.equal(media.media_kind, 'Shader');
		const channel = media.channels[0];
		assert.equal(channel.definition.source.format, shader.format);
		assert.deepEqual(channel.definition.source.input.file, { kind: 'absolute', path: nativePath });
		if (index === 0) {
			await intent({ kind: 'undo' });
			assert.equal(await readMedia(), undefined);
			await intent({ kind: 'redo' });
			assert.equal((await readMedia()).channels[0].reference.channel, channel.reference.channel);
		}
		outputBinding = await shaderObserver.demand(channel.reference, outputBinding);
		await delay(750);
		await shaderObserver.observePixels(shader.format.toLowerCase(), shader.rgba);
	}
	observations.push(
		'The local library filtered non-shaders and rendered imported GLSL, ISF and ShaderToy files'
	);
	stage = 'reopen the exact local shader binding';
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await loadProject();
	await delay(750);
	await shaderObserver.observePixels('reopened-shader', shaders.at(-1).rgba);
	assert(!appLog.includes('[mapgyver-output] window id='));
	observations.push(
		'The imported file remained linked on project reopen. Shader import is one Undo step'
	);
	const librarySource = library.getByRole('combobox', { name: 'Shader library source' });
	if (onlineIsf) {
		stage = 'browse and search the live VIDVOX ISF catalog';
		const { folder } = await edit({ kind: 'CreateFolder', parent: null, label: 'Online shaders' });
		await librarySource.selectOption('isf');
		const catalog = library.getByRole('list', { name: 'ISF catalog' });
		const solid = catalog.getByText('Solid Color', { exact: true });
		await solid.waitFor({ timeout: 30000 });
		const listed = await catalog.getByRole('listitem').count();
		assert(listed > 100);
		const search = library.getByRole('searchbox', { name: 'Search ISF shaders' });
		await search.fill('no-matching-shader-93');
		await library.getByText('No shaders found', { exact: true }).waitFor();
		await search.fill('solid color');
		await solid.waitFor();
		assert((await catalog.getByRole('listitem').count()) < listed);

		stage = 'drop a catalog ISF into a Media folder and verify embedded source';
		const target = medias.getByRole('button', { name: 'Online shaders', exact: true });
		await solid.dragTo(target);
		const importedIsf = async () =>
			(await edit({ kind: 'InspectManager' })).snapshot.media.find(
				(media) => media.label === 'Solid Color'
			);
		const media = await wait(importedIsf, Boolean, 15000);
		const channel = media.channels[0];
		const source = channel.definition.source;
		assert.equal(media.media_kind, 'Shader');
		assert.equal(source.format, 'ISF');
		assert.equal(source.input.kind, 'Embedded');
		assert.match(
			source.input.code,
			/https:\/\/github\.com\/Vidvox\/ISF-Files, ISF\/Solid Color\.fs/
		);
		assert.match(source.input.code, /Permission is hereby granted/);
		const graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		assert(
			graph.nodes
				.find(({ uuid }) => uuid === folder)
				.children.includes(graph.nodes.find(({ uuid }) => uuid === channel.reference.media).node_id)
		);
		await intent({ kind: 'undo' });
		assert.equal(await importedIsf(), undefined);
		await intent({ kind: 'redo' });
		assert.deepEqual((await importedIsf()).channels[0], channel);

		stage = 'render the ISF generator and its reopened embedded source';
		outputBinding = await shaderObserver.demand(channel.reference, outputBinding);
		await delay(750);
		await shaderObserver.observePixels('online-isf', [255, 0, 0, 255]);
		assert((await post('/api/ui/project-save', { path: project })).ok);
		await loadProject();
		assert.deepEqual((await importedIsf()).channels[0], channel);
		await delay(750);
		await shaderObserver.observePixels('reopened-online-isf', [255, 0, 0, 255]);
		assert(!appLog.includes('[mapgyver-output] window id='));
		await librarySource.selectOption('local');
		await library.getByRole('button', { name: 'Local shaders', exact: true }).waitFor();
		isfCatalog = { listed, imported: media.label, mode: source.input.kind, folder, undoRedo: true };
	}
	if (onlineFragCoord) {
		stage = 'browse, page and search the live FragCoord gallery';
		await librarySource.selectOption('fragcoord');
		const catalog = library.getByRole('list', { name: 'FragCoord catalog' });
		const rows = catalog.locator('li[draggable]');
		const more = catalog.getByRole('button', { name: 'Load more' });
		await more.waitFor({ timeout: 30000 });
		const firstPage = await rows.count();
		assert(firstPage > 1);
		await more.click();
		const paged = await wait(
			() => rows.count(),
			(count) => count > firstPage,
			15000
		);
		const search = library.getByRole('searchbox', { name: 'Search FragCoord shaders' });
		await search.fill('q9fx31-no-fragcoord-match');
		await library.getByText('No shaders found', { exact: true }).waitFor({ timeout: 15000 });
		assert.equal(await rows.count(), 0);
		await search.fill('Neon Ghosts');
		const ghost = catalog.getByText('Neon Ghosts', { exact: true });
		await ghost.waitFor({ timeout: 15000 });

		stage = 'authorize only reusable source and drop a licensed FragCoord shader';
		await assert.rejects(
			page.evaluate(() =>
				window.__TAURI_INTERNALS__.invoke('plugin:mapgyver-shader-library|load_fragcoord', {
					id: 'gouyatcw'
				})
			),
			/no reuse license/
		);
		await ghost.dragTo(medias, { targetPosition: { x: bounds.width / 2, y: bounds.height * 0.9 } });
		const importedFragCoord = async () =>
			(await edit({ kind: 'InspectManager' })).snapshot.media.find(
				(media) => media.label === 'Neon Ghosts'
			);
		const media = await wait(importedFragCoord, Boolean, 15000);
		const channel = media.channels[0];
		const source = channel.definition.source;
		assert.equal(media.media_kind, 'Shader');
		assert.equal(source.format, 'FragCoord');
		assert.equal(source.input.kind, 'Embedded');
		assert.match(source.input.code, /https:\/\/fragcoord\.xyz\/s\/nn4e99ne by berzerkey_jo/);
		assert.match(
			source.input.code,
			/CC-BY-4\.0 \(https:\/\/creativecommons\.org\/licenses\/by\/4\.0\/\)/
		);
		await intent({ kind: 'undo' });
		assert.equal(await importedFragCoord(), undefined);
		await intent({ kind: 'redo' });
		assert.deepEqual((await importedFragCoord()).channels[0], channel);

		stage = 'render FragCoord output before and after reopening';
		await shaderObserver.demand(channel.reference, outputBinding);
		const verify = (colors) => {
			assert(colors.length > 16, `Expected a rendered pattern, got ${colors.length} colors`);
			assert(
				colors.every((rgba) => rgba[3] === 255),
				'The shader specifies opaque output'
			);
		};
		await delay(1000);
		await shaderObserver.observePattern('online-fragcoord', verify);
		assert((await post('/api/ui/project-save', { path: project })).ok);
		await loadProject();
		assert.deepEqual((await importedFragCoord()).channels[0], channel);
		await delay(1000);
		await shaderObserver.observePattern('reopened-online-fragcoord', verify);
		assert(!appLog.includes('[mapgyver-output] window id='));
		fragCoordCatalog = {
			firstPage,
			paged,
			imported: media.label,
			mode: source.input.kind,
			undoRedo: true,
			unlicensedRejected: true
		};
	}
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
			shaderPixels: shaderObserver?.observations,
			shaderAdapter: shaderObserver?.adapter,
			isfCatalog,
			fragCoordCatalog,
			physicalWindowsOpened: appLog.split('[mapgyver-output] window id=').length - 1,
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
