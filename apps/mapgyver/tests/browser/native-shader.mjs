import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const outputDir = path.resolve(process.argv[2] ?? 'target/mapgyver-native-shader');
await mkdir(outputDir, { recursive: true });
const directory = await mkdtemp(path.join(outputDir, 'run-'));
const native = await openNativeEditor({ appDataDirectory: path.join(directory, 'profile') });
const { page } = native;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token;
let stage = 'create Shader Media';

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
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}

async function graph() {
	return (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
}

async function waitFor(check) {
	const deadline = performance.now() + 6000;
	while (!(await check())) {
		assert(performance.now() < deadline, stage);
		await delay(25);
	}
}

const blue = `void main() {
    if (iResolution.x < 1.0 || gl_FragCoord.x < 0.5 || iTime < 0.0) {
        fragColor = vec4(1.0, 0.0, 0.0, 1.0);
    } else {
        fragColor = vec4(0.0, 0.0, 1.0, 0.5);
    }
}`;
const green = 'void main() { fragColor = vec4(0.0, 1.0, 0.0, 1.0); }';
const red = 'void main() { fragColor = vec4(1.0, 0.0, 0.0, 1.0); }';
const invalid = 'void main() { fragColor = unknownColor; }';

try {
	token = (await post('/api/ui/session', {})).token;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	await manager
		.locator('.manager-list-header')
		.getByRole('button', { name: 'Add child item' })
		.click();
	await page.getByRole('menuitem', { name: 'Shader', exact: true }).click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	const nodes = await graph();
	const media = nodes.find((node) => node.node_type === 'mapgyver.media.v11');
	const channel = nodes.find(
		(node) =>
			media.children.includes(node.node_id) && node.node_type === 'mapgyver.media.channel.v2'
	);
	const parameters = nodes.find(
		(node) => channel.children.includes(node.node_id) && node.decl_id === 'parameters'
	);
	const code = nodes.find(
		(node) =>
			parameters.children.includes(node.node_id) && node.decl_id === 'parameters/shader_code'
	);
	const header = page.locator(
		`.node-inspector[data-node-id="${channel.node_id}"] > .node-header > .node-title`
	);
	await header.waitFor();
	if ((await header.getAttribute('aria-checked')) === 'false') await header.press('Enter');
	const editor = page.locator(`.parameter-inspector[data-node-id="${code.node_id}"] textarea`);
	assert.match(await editor.inputValue(), /iTime/);
	const observer = await createPreviewOutputObserver(post, directory, page);
	const reference = { media: media.uuid, channel: channel.uuid };
	const screenCatalog = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.screens',
		operation: 'edit',
		payload: { kind: 'InspectManager' }
	});
	assert(screenCatalog.success, JSON.stringify(screenCatalog));
	const [screen] = screenCatalog.result.value.snapshot.screens;

	async function edit(value) {
		await editor.fill(value);
		await editor.press('Control+Enter');
		await waitFor(async () => {
			const parameter = (await graph()).find((node) => node.uuid === code.uuid);
			return parameter?.data.param.value.Str === value;
		});
	}

	stage = 'render edited GLSL with resolution, position, time and alpha';
	await edit(blue);
	await observer.demand(reference, { screen: screen.id, surface: screen.surfaces[0].reference });
	await delay(500);
	// The downstream Screen blend and UNORM conversion may round half alpha by one code.
	await observer.observePixels('half-alpha-blue', [0, 0, 255, 128], 1);
	for (const [r, g, b] of observer.observations.at(-1).actualColors) {
		assert.deepEqual([r, g, b], [0, 0, 255]);
	}

	stage = 'replace an invalid shader without losing the output';
	await edit(invalid);
	await waitFor(() => native.log().includes('unknownColor'));
	await delay(500);
	await observer.observePixels('invalid-clears-output', [0, 0, 0, 0]);
	await edit(green);
	await delay(500);
	await observer.observePixels('recovered-green', [0, 255, 0, 255]);

	stage = 'reload the project and render saved GLSL';
	const project = path.join(directory, 'Shader.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await edit(red);
	await delay(500);
	await observer.observePixels('unsaved-red', [255, 0, 0, 255]);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await waitFor(async () => {
		const parameter = (await graph()).find((node) => node.uuid === code.uuid);
		return parameter?.data.param.value.Str === green;
	});
	await delay(500);
	await observer.observePixels('reloaded-green', [0, 255, 0, 255]);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{ result: 'pass', adapter: observer.adapter, pixels: observer.observations, errors },
			null,
			2
		)
	);
	console.log(`Shader Media output verified in ${directory}`);
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), log: native.log(), errors }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
