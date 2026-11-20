import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

const outputDir = path.resolve(process.argv[2] ?? 'target/mapgyver-native-shader');
await mkdir(outputDir, { recursive: true });
const directory = await mkdtemp(path.join(outputDir, 'run-'));
let appLog = '';
const native = await openNativeEditor({
	appDataDirectory: path.join(directory, 'profile'),
	onLogData: (chunk) => (appLog += chunk)
});
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

async function mediaOperation(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
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
const magenta = 'void main() { fragColor = vec4(1.0, 0.0, 1.0, 1.0); }';
const yellow = 'void main() { fragColor = vec4(1.0, 1.0, 0.0, 1.0); }';

try {
	token = (await post('/api/ui/session', {})).token;
	const shaderTab = page.getByRole('tab').filter({ hasText: /^Shader Editor$/ });
	const panel = page.locator('[data-panel-id="shader-editor"]');
	await shaderTab.waitFor();
	assert.equal(await panel.isVisible(), false, 'Shader Editor opens as an inactive dock tab');
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
	stage = 'verify startup display selection without a physical window';
	const displays = JSON.parse(
		execFileSync(path.resolve('target/debug/native_qualification.exe'), ['--role', 'catalog'], {
			encoding: 'utf8',
			windowsHide: true,
			timeout: 10000
		})
	);
	const external = displays.filter(
		({ internal, bounds }) =>
			internal === false && bounds.extent.width <= 8192 && bounds.extent.height <= 8192
	);
	const selected = external.find(({ id }) => id === screen.definition.target?.display);
	if (external.length > 0) {
		assert(selected, JSON.stringify(screen.definition.target));
		const numbered = external.map(({ index }) => index).filter(Boolean);
		if (numbered.length > 0) assert.equal(selected.index, Math.min(...numbered));
	} else {
		assert.equal(screen.definition.target, null);
	}
	assert(!appLog.includes('[mapgyver-output] window id='));

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

	stage = 'apply code from the docked Shader Editor';
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	const codeEditor = panel.locator('.cm-content');
	await codeEditor.fill(magenta);
	await codeEditor.press('Control+s');
	await waitFor(async () => {
		const parameter = (await graph()).find((node) => node.uuid === code.uuid);
		return parameter?.data.param.value.Str === magenta;
	});
	await delay(500);
	await observer.observePixels('editor-magenta', [255, 0, 255, 255]);

	stage = 'select and automatically reload an external shader';
	const source = panel.getByRole('combobox', { name: 'Source', exact: true });
	const format = panel.getByRole('combobox', { name: 'Format', exact: true });
	const name = 'Live.glsl';
	const filename = path.join(directory, name);
	await writeFile(filename, green);
	await source.selectOption('File');
	const file = panel.getByRole('textbox', { name: 'Shader File', exact: true });
	await file.fill(name);
	await file.press('Enter');
	await delay(750);
	await observer.observePixels('external-green', [0, 255, 0, 255]);

	await writeFile(`${filename}.new`, yellow);
	await rename(`${filename}.new`, filename);
	await delay(750);
	await observer.observePixels('external-atomic-replace-yellow', [255, 255, 0, 255]);

	stage = 'clear a missing external source and recover when it returns';
	await rm(filename);
	await delay(750);
	await observer.observePixels('external-missing', [0, 0, 0, 0]);
	await writeFile(filename, red);
	await delay(750);
	await observer.observePixels('external-recreated-red', [255, 0, 0, 255]);

	stage = 'reload external source settings and preserve dormant embedded code';
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await source.selectOption('Embedded');
	await delay(500);
	await observer.observePixels('retained-embedded-magenta', [255, 0, 255, 255]);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('saved-external-red', [255, 0, 0, 255]);

	stage = 'compile a ShaderToy Image pass and isolate the format from other compiled sources';
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	await source.selectOption('Embedded');
	await waitFor(
		async () =>
			(await graph()).find((node) => node.uuid === code.uuid)?.data.param.value.Str === magenta
	);
	await delay(500);
	await observer.observePixels('reloaded-dormant-embedded-magenta', [255, 0, 255, 255]);
	await format.selectOption('ShaderToy');
	await delay(500);
	await observer.observePixels('format-change-clears-glsl', [0, 0, 0, 0]);
	const shadertoy = [
		'void mainImage(out vec4 color, in vec2 pixel) {',
		'    color = vec4(float(iResolution.x > pixel.x && iFrame >= 0), 1.0, 1.0, 1.0);',
		'}'
	].join('\n');
	await codeEditor.fill(shadertoy);
	await panel.getByRole('button', { name: 'Apply shader', exact: true }).click();
	await waitFor(
		async () =>
			(await graph()).find((node) => node.uuid === code.uuid)?.data.param.value.Str === shadertoy
	);
	await delay(500);
	await observer.observePixels('shadertoy-image', [255, 255, 255, 255]);

	stage = 'load an external ISF generator and round-trip its Format and File parameters';
	const isfName = 'Generator.fs';
	await writeFile(
		path.join(directory, isfName),
		`/*{
		"ISFVSN": "2", "PASSES": [{}],
		"INPUTS": [
			{"NAME":"drawTint","TYPE":"bool","DEFAULT":1},
			{"NAME":"tint","TYPE":"color","DEFAULT":[0,1,1,1]}
		]
	}*/
	void main() {
		gl_FragColor = drawTint && PASSINDEX == 0 ? tint : vec4(1.0, 0.0, 0.0, 1.0);
	}`
	);
	await format.selectOption('ISF');
	await source.selectOption('File');
	await file.fill(isfName);
	await file.press('Enter');
	await delay(750);
	await observer.observePixels('isf-file-generator', [0, 255, 255, 255]);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('reloaded-isf-format-file', [0, 255, 255, 255]);

	stage = 'bind a Media and an exact Channel through the texture pickers';
	const { reference: firstTexture } = await mediaOperation({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Shader Texture Source'
	});
	const { reference: otherTexture } = await mediaOperation({
		kind: 'AddChannel',
		media: firstTexture.media,
		label: 'Shader Texture Green'
	});
	const color = (reference, rgba) =>
		mediaOperation({ kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
	await color(firstTexture, [1, 0, 0, 1]);
	await color(otherTexture, [0, 1, 0, 1]);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	await source.selectOption('Embedded');
	await format.selectOption('ShaderToy');
	const sampler = [
		'void mainImage(out vec4 color, in vec2 p) {',
		'    vec2 uv = p / iResolution.xy;',
		'    vec4 first = texture(iChannel0, uv);',
		'    vec4 second = texture(iChannel3, uv);',
		'    color = vec4(first.rgb + second.rgb, max(first.a, second.a));',
		'}'
	].join('\n');
	await codeEditor.fill(sampler);
	await panel.getByRole('button', { name: 'Apply shader', exact: true }).click();
	await waitFor(
		async () =>
			(await graph()).find((node) => node.uuid === code.uuid)?.data.param.value.Str === sampler
	);
	await delay(500);
	await observer.observePixels('unbound-shader-inputs', [0, 0, 0, 0]);
	await panel.getByText('Texture inputs', { exact: true }).click();
	const pickTexture = async (slot, label, target) => {
		const nodes = await graph();
		const owner = nodes.find((node) => node.uuid === channel.uuid);
		const folder = nodes.find(
			(node) => owner.children.includes(node.node_id) && node.decl_id === 'parameters'
		);
		const parameter = nodes.find(
			(node) =>
				folder.children.includes(node.node_id) &&
				node.decl_id === `parameters/shader_texture_${slot}`
		);
		await panel
			.locator(`.parameter-inspector[data-node-id="${parameter.node_id}"] .picker-trigger`)
			.click();
		const picker = page.getByRole('dialog', { name: 'Select reference target' });
		await picker.getByRole('searchbox').fill(label);
		await picker.getByText(label, { exact: true }).click();
		await picker.getByRole('button', { name: 'Select', exact: true }).click();
		await waitFor(
			async () =>
				(await graph()).find((node) => node.uuid === parameter.uuid)?.data.param.value.Reference
					?.uuid === target
		);
		return parameter;
	};
	const texture0 = await pickTexture(0, 'Shader Texture Source', firstTexture.media);
	await pickTexture(3, 'Shader Texture Green', otherTexture.channel);
	await delay(500);
	await observer.observePixels('media-first-plus-explicit-channel', [255, 255, 0, 255]);

	stage = 'verify live texture updates, removed input and Undo';
	await color(firstTexture, [0, 0, 1, 1]);
	await delay(500);
	await observer.observePixels('live-input-update', [0, 255, 255, 255]);
	await mediaOperation({ kind: 'RemoveChannel', reference: otherTexture });
	await delay(500);
	await observer.observePixels('removed-input-clears-only-that-slot', [0, 0, 255, 255]);
	assert((await post('/api/ui/intent', { kind: 'undo' })).success);
	await delay(500);
	await observer.observePixels('undo-restores-shader-input', [0, 255, 255, 255]);

	stage = 'reject a direct feedback reference and reopen both authored bindings';
	const bad = await post('/api/ui/intent', {
		kind: 'setParam',
		node: texture0.node_id,
		value: { Reference: { uuid: channel.uuid } },
		behaviour: 'Append'
	});
	assert.equal(bad.success, false, JSON.stringify(bad));
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('reloaded-shader-texture-bindings', [0, 255, 255, 255]);

	stage = 'use the first bound texture as a named ISF image input';
	const isfTextureName = 'TextureSource.isf';
	await writeFile(
		path.join(directory, isfTextureName),
		'/*{"INPUTS":[{"NAME":"inputImage","TYPE":"image"}]}*/\nvoid main() { gl_FragColor = IMG_THIS_NORM_PIXEL(inputImage); }'
	);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	await format.selectOption('ISF');
	await source.selectOption('File');
	await file.fill(isfTextureName);
	await file.press('Enter');
	await delay(750);
	await observer.observePixels('isf-named-media-image', [0, 0, 255, 255]);

	stage = 'render a three-pass ISF file with resized temporary targets and one frame counter';
	await mediaOperation({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 64, height: 32 } }
	});
	const multipassName = 'ThreePasses.isf';
	const multipass = `/*{
		"ISFVSN":"2",
		"INPUTS":[{"NAME":"inputImage","TYPE":"image"}],
		"PASSES":[
			{"TARGET":"small","WIDTH":"$WIDTH / 2","HEIGHT":"$HEIGHT / 4","FLOAT":true},
			{"TARGET":"mixed","WIDTH":16,"HEIGHT":4},
			{}
		]
	}*/
	void main() {
		float frameMarker = 2.0 + float(FRAMEINDEX % 3);
		if (PASSINDEX == 0) {
			gl_FragColor = vec4(frameMarker, RENDERSIZE.x == 32.0 && RENDERSIZE.y == 8.0 ? 1.0 : 0.0, 0.0, 1.0);
		} else if (PASSINDEX == 1) {
			vec4 raw = IMG_THIS_NORM_PIXEL(small);
			bool correct = raw.r == frameMarker && raw.g == 1.0 &&
				IMG_SIZE(small) == vec2(32.0, 8.0) && RENDERSIZE == vec2(16.0, 4.0);
			vec4 media = IMG_THIS_NORM_PIXEL(inputImage);
			gl_FragColor = correct ? vec4(media.rgb + vec3(0.0, 1.0, 0.0), 1.0) : vec4(1.0, 0.0, 1.0, 1.0);
		} else if (PASSINDEX == 2 && RENDERSIZE == vec2(64.0, 32.0)) {
			gl_FragColor = IMG_THIS_NORM_PIXEL(mixed);
		}
	}`;
	const multipassPath = path.join(directory, multipassName);
	await writeFile(multipassPath, multipass);
	await file.fill(multipassName);
	await file.press('Enter');
	await delay(750);
	await observer.observePixels('isf-multipass-float-and-shared-frame', [0, 255, 255, 255]);
	await color(firstTexture, [1, 0, 0, 1]);
	await delay(500);
	await observer.observePixels('isf-multipass-live-media', [255, 255, 0, 255]);

	stage = 'reopen the multipass file and replace its pass layout live';
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('isf-multipass-after-reopen', [255, 255, 0, 255]);
	await writeFile(
		`${multipassPath}.new`,
		'/*{}*/\nvoid main() { gl_FragColor = vec4(0.0, 1.0, 0.0, 1.0); }'
	);
	await rename(`${multipassPath}.new`, multipassPath);
	await delay(750);
	await observer.observePixels('isf-multipass-replaced-by-single-pass', [0, 255, 0, 255]);
	await writeFile(`${multipassPath}.new`, multipass);
	await rename(`${multipassPath}.new`, multipassPath);
	await delay(750);
	await observer.observePixels('isf-multipass-layout-restored', [255, 255, 0, 255]);

	stage = 'accumulate ISF history and preserve it across compatible Media edits';
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	const persistentName = 'Persistent.isf';
	const persistentPath = path.join(directory, persistentName);
	const persistent = `/*{
		"INPUTS":[{"NAME":"inputImage","TYPE":"image"}],
		"PASSES":[{"TARGET":"trail","PERSISTENT":true,"WIDTH":"$WIDTH/2","HEIGHT":"$HEIGHT/2"},{}]
	}*/
	void main() {
		vec4 old = IMG_THIS_NORM_PIXEL(trail);
		if (PASSINDEX == 0) {
			if (old.a == 0.0) {
				vec4 initial = IMG_THIS_NORM_PIXEL(inputImage);
				gl_FragColor = vec4(initial.r, 0.0, initial.b, 1.0);
			} else {
				gl_FragColor = vec4(old.r, min(1.0, old.g + 0.25), old.b, 1.0);
			}
		} else {
			gl_FragColor = old;
		}
	}`;
	await writeFile(persistentPath, persistent);
	await file.fill(persistentName);
	await file.press('Enter');
	await delay(750);
	await observer.observePixels('isf-history-accumulates-across-frames', [255, 255, 0, 255]);
	await color(firstTexture, [0, 0, 1, 1]);
	await delay(750);
	await observer.observePixels('isf-history-survives-compatible-replan', [255, 255, 0, 255]);

	stage = 'reset ISF history on resize, changed code and project reopen';
	await mediaOperation({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 80, height: 40 } }
	});
	await delay(750);
	await observer.observePixels('isf-resized-history-seeds-from-blue', [0, 255, 255, 255]);
	await color(firstTexture, [1, 0, 0, 1]);
	await delay(750);
	await observer.observePixels('isf-resized-history-survives-replan', [0, 255, 255, 255]);
	await writeFile(`${persistentPath}.new`, `${persistent}\n// replacement source revision\n`);
	await rename(`${persistentPath}.new`, persistentPath);
	await delay(750);
	await observer.observePixels('isf-history-reseeds-after-code-change', [255, 255, 0, 255]);
	await color(firstTexture, [0, 0, 1, 1]);
	await delay(750);
	await observer.observePixels('isf-history-before-reopen', [255, 255, 0, 255]);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('isf-history-reseeds-after-reopen', [0, 255, 255, 255]);

	stage = 'load ShaderToy JSON with Common, two Buffers, and Image';
	const toy = {
		Shader: {
			renderpass: [
				{
					type: 'image',
					name: 'Image',
					inputs: [{ channel: 1, ctype: 'buffer', id: 258 }],
					code: 'void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel1, uv(p)); }'
				},
				{
					type: 'buffer',
					name: 'Buffer B',
					outputs: [{ channel: 0, id: 258 }],
					inputs: [
						{ channel: 0, ctype: 'texture', id: 1, src: '/media/example.png' },
						{ channel: 2, ctype: 'buffer', id: 257 }
					],
					code: `
					void mainImage(out vec4 c, in vec2 p) {
						vec4 a = texture(iChannel2, uv(p));
						vec4 media = texture(iChannel0, uv(p));
						bool valid = a.r == frameMarker() && a.g == 1.0 && a.b == 1.0 && a.a == 0.0
							&& iChannelResolution[2].xy == iResolution.xy;
						c = valid ? vec4(media.rgb + vec3(0.0, 1.0, 0.0), 2.0) : vec4(1.0, 0.0, 1.0, 2.0);
					}`
				},
				{
					type: 'common',
					name: 'Common',
					code: `
					vec2 uv(vec2 p) { return p / iResolution.xy; }
					float frameMarker() { return 2.0 + float(iFrame % 5); }`
				},
				{
					type: 'buffer',
					name: 'Buffer A',
					outputs: [{ channel: 0, id: 257 }],
					inputs: [
						{ channel: 0, ctype: 'buffer', id: 257 },
						{ channel: 1, ctype: 'buffer', id: 258 }
					],
					code: `
					void mainImage(out vec4 c, in vec2 p) {
						vec4 priorA = texture(iChannel0, uv(p));
						vec4 priorB = texture(iChannel1, uv(p));
						c = vec4(frameMarker(), min(1.0, priorA.g + 0.25), float(priorB.a > 1.0), 0.0);
					}`
				}
			]
		}
	};
	const toyName = 'Buffers.json';
	const toyPath = path.join(directory, toyName);
	await writeFile(toyPath, JSON.stringify(toy));
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	await format.selectOption('ShaderToy');
	await file.fill(toyName);
	await file.press('Enter');
	await delay(900);
	await observer.observePixels('shadertoy-common-and-buffers', [0, 255, 255, 255]);
	await color(firstTexture, [1, 0, 0, 1]);
	await delay(750);
	await observer.observePixels('shadertoy-buffer-live-media', [255, 255, 0, 255]);

	stage = 'reopen ShaderToy JSON and reload a changed Image pass';
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(900);
	await observer.observePixels('shadertoy-buffer-after-reopen', [255, 255, 0, 255]);
	const replacement = structuredClone(toy);
	replacement.Shader.renderpass[0].code =
		'void mainImage(out vec4 c, in vec2 p) { c = vec4(0.0, 1.0, 0.0, 1.0); }';
	await writeFile(`${toyPath}.new`, JSON.stringify(replacement));
	await rename(`${toyPath}.new`, toyPath);
	await delay(900);
	await observer.observePixels('shadertoy-reloaded-image-draw', [0, 255, 0, 255]);
	await writeFile(`${toyPath}.new`, JSON.stringify(toy));
	await rename(`${toyPath}.new`, toyPath);
	await delay(900);
	await observer.observePixels('shadertoy-restored-buffer-layout', [255, 255, 0, 255]);

	stage = 'add typed ISF uniforms through the Shader Editor';
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await manager.getByRole('button', { name: 'Shader', exact: true }).click();
	await shaderTab.click();
	await source.selectOption('Embedded');
	await format.selectOption('ISF');
	const editableIsf = `/*{"INPUTS":[
        {"NAME":"amount","LABEL":"Mix amount","TYPE":"float","DEFAULT":0.25,"MIN":0.0,"MAX":1.0},
        {"NAME":"inputEnabled","TYPE":"bool","DEFAULT":true},
        {"NAME":"mode","TYPE":"long","DEFAULT":-3},
        {"NAME":"pivot","TYPE":"point2D","DEFAULT":[1.0,0.0]},
        {"NAME":"tint","TYPE":"color","DEFAULT":[0.0,0.0,1.0,1.0]}
    ]}*/
    void main() {
        gl_FragColor = vec4(inputEnabled && mode == -3 ? amount : 0.0, pivot.x, tint.b, tint.a);
    }`;
	await codeEditor.fill(editableIsf);
	await panel.getByRole('button', { name: 'Apply shader', exact: true }).click();
	await panel.getByText('Uniforms', { exact: true }).click();
	await delay(500);
	// Spout presents the shader's linear values as sRGB.
	const defaultInputs = [137, 255, 255, 255];
	const editedInputs = [225, 255, 255, 255];
	await observer.observePixels('isf-inputs-use-metadata-defaults', defaultInputs, 1);
	await panel.getByRole('button', { name: 'Add uniform amount', exact: true }).click();
	const inspectUniforms = () => mediaOperation({ kind: 'InspectShaderUniforms', reference });
	await waitFor(async () =>
		(await inspectUniforms()).rows.some(
			({ name, parameter }) => name === 'amount' && parameter !== null
		)
	);
	await panel.getByRole('button', { name: 'Add All', exact: true }).click();
	await waitFor(async () => {
		const { rows } = await inspectUniforms();
		return rows.length === 5 && rows.every(({ parameter }) => parameter !== null);
	});
	const { rows } = await inspectUniforms();
	const fields = Object.fromEntries(rows.map(({ name, parameter }) => [name, parameter]));
	const snapshot = await graph();
	const effective = (name) => snapshot.find((node) => node.node_id === fields[name]).data.param;
	assert.equal(effective('amount').value.Float, 0.25);
	assert.deepEqual(effective('amount').constraints.range, { kind: 'uniform', min: 0, max: 1 });
	assert.equal(effective('inputEnabled').value.Bool, true);
	assert.equal(effective('mode').value.Int, -3);
	assert.deepEqual(effective('pivot').value.Vec2, [1, 0]);
	assert.deepEqual(effective('tint').value.Color, [0, 0, 1, 1]);

	stage = 'edit, undo, and reopen the authored ISF uniform';
	const amount = panel.locator(
		`.parameter-inspector[data-node-id="${fields.amount}"] input.number-field`
	);
	await amount.fill('0.75');
	await amount.press('Enter');
	await waitFor(
		async () =>
			(await graph()).find((node) => node.node_id === fields.amount)?.data.param.value.Float ===
			0.75
	);
	await delay(500);
	await observer.observePixels('isf-uniform-live-edit', editedInputs, 1);
	assert((await post('/api/ui/intent', { kind: 'undo' })).success);
	await delay(500);
	await observer.observePixels('isf-uniform-undo', defaultInputs, 1);
	assert((await post('/api/ui/intent', { kind: 'redo' })).success);
	await delay(500);
	await observer.observePixels('isf-uniform-redo', editedInputs, 1);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('isf-uniform-saved-value', editedInputs, 1);
	const reopened = await graph();
	for (const id of Object.values(fields)) {
		const original = snapshot.find((node) => node.node_id === id);
		assert(
			reopened.some((node) => node.uuid === original.uuid),
			original.decl_id
		);
	}

	async function activateShader() {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await manager.getByRole('button', { name: 'Shader', exact: true }).click();
		await shaderTab.click();
	}

	async function applyShader(dialect, sourceText) {
		await format.selectOption(dialect);
		await codeEditor.fill(sourceText);
		await panel.getByRole('button', { name: 'Apply shader', exact: true }).click();
		const uniforms = panel.locator('details.uniforms');
		await uniforms.waitFor();
		if ((await uniforms.getAttribute('open')) === null) await uniforms.locator('summary').click();
	}

	async function setUniform(name, value) {
		const row = (await inspectUniforms()).rows.find((row) => row.name === name);
		assert(Number.isInteger(row?.parameter), JSON.stringify(row));
		const input = panel.locator(
			`.parameter-inspector[data-node-id="${row.parameter}"] input.number-field`
		);
		await input.fill(String(value));
		await input.press('Enter');
		await waitFor(
			async () =>
				(await graph()).find((node) => node.node_id === row.parameter)?.data.param.value.Float ===
				value
		);
		await delay(500);
	}

	stage = 'expose GLSL uniforms with FragCoord defaults and bounds';
	await activateShader();
	await applyShader(
		'GLSL',
		`uniform float u_value1; //0.5 [0, 1]
        uniform vec3 position; //0.1, 1.0, 0.2, [0, 1]
        uniform bool showBlue = true;
        void main() {
            fragColor = vec4(u_value1, position.y, float(showBlue), 1.0);
        }`
	);
	await delay(500);
	await observer.observePixels('glsl-uniform-defaults', [188, 255, 255, 255], 1);
	await panel.getByRole('button', { name: 'Add All', exact: true }).click();
	await waitFor(async () => {
		const { rows } = await inspectUniforms();
		return rows.length === 3 && rows.every(({ parameter }) => parameter !== null);
	});
	const glslRows = (await inspectUniforms()).rows;
	const glslGraph = await graph();
	const glslParam = (name) =>
		glslGraph.find((node) => node.node_id === glslRows.find((row) => row.name === name).parameter)
			.data.param;
	assert.deepEqual(glslParam('u_value1').constraints.range, { kind: 'uniform', min: 0, max: 1 });
	assert.equal(glslParam('position').value.Vec3[1], 1);
	assert.equal(glslParam('showBlue').value.Bool, true);
	await setUniform('u_value1', 0);
	await observer.observePixels('glsl-uniform-live-edit', [0, 255, 255, 255]);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('glsl-uniform-reopen', [0, 255, 255, 255]);

	stage = 'drive a ShaderToy Buffer and Image with the same Common uniform';
	await activateShader();
	await applyShader(
		'ShaderToy',
		JSON.stringify({
			renderpass: [
				{ type: 'common', code: 'uniform float sharedGain; //0.0, [0, 1]' },
				{
					type: 'buffer',
					name: 'Buffer A',
					outputs: [{ id: 'A' }],
					code: 'void mainImage(out vec4 c, in vec2 p) { c = vec4(sharedGain, 0.0, 0.0, 1.0); }'
				},
				{
					type: 'image',
					inputs: [{ ctype: 'buffer', channel: 0, id: 'A' }],
					code: 'void mainImage(out vec4 c, in vec2 p) { c = vec4(texelFetch(iChannel0, ivec2(0), 0).r, sharedGain, 1.0, 1.0); }'
				}
			]
		})
	);
	await delay(500);
	await observer.observePixels('shadertoy-common-uniform-default', [0, 0, 255, 255]);
	assert.deepEqual(
		(await inspectUniforms()).rows.map(({ name }) => name),
		['sharedGain']
	);
	await panel.getByRole('button', { name: 'Add uniform sharedGain', exact: true }).click();
	await waitFor(async () => Number.isInteger((await inspectUniforms()).rows[0]?.parameter));
	await setUniform('sharedGain', 1);
	await observer.observePixels('shadertoy-common-uniform-live-edit', [255, 255, 255, 255]);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await delay(750);
	await observer.observePixels('shadertoy-common-uniform-reopen', [255, 255, 255, 255]);

	stage = 'discover and edit uniforms from the rendered external file';
	const uniformFile = 'Uniforms.glsl';
	const uniformPath = path.join(directory, uniformFile);
	await writeFile(
		uniformPath,
		`uniform float fileGain; //0 [0, 1]
        uniform float fileGreen; //1 [0, 1]
        void main() { fragColor = vec4(fileGain, fileGreen, 0.0, 1.0); }`
	);
	await activateShader();
	await format.selectOption('GLSL');
	await source.selectOption('File');
	await file.fill(uniformFile);
	await file.press('Enter');
	const controls = panel.locator('details.uniforms');
	await controls.locator('summary').click();
	await controls.getByRole('button', { name: 'Add All', exact: true }).click();
	await waitFor(async () => {
		const { rows, revision } = await inspectUniforms();
		return revision && rows.length === 2 && rows.every(({ parameter }) => parameter !== null);
	});
	const originalFileInputs = await inspectUniforms();
	const previousField = originalFileInputs.rows.find(({ name }) => name === 'fileGain').parameter;
	const removedField = originalFileInputs.rows.find(({ name }) => name === 'fileGreen').parameter;
	const previousUuid = (await graph()).find(({ node_id }) => node_id === previousField).uuid;
	await delay(500);
	await observer.observePixels('file-uniform-defaults', [0, 255, 0, 255]);
	await setUniform('fileGain', 1);
	await observer.observePixels('file-uniform-live-edit', [255, 255, 0, 255]);

	stage = 'replace the file and refresh shader controls without reselection';
	await writeFile(
		`${uniformPath}.new`,
		`uniform float fileGain; //0 [0, 1]
        uniform float fileBlue; //1 [0, 1]
        void main() { fragColor = vec4(fileGain, 0.0, fileBlue, 1.0); }`
	);
	await rename(`${uniformPath}.new`, uniformPath);
	const addBlue = controls.getByRole('button', { name: 'Add uniform fileBlue', exact: true });
	await addBlue.waitFor();
	assert.equal(await controls.locator(`[data-node-id="${removedField}"]`).count(), 0);
	const latestFileInputs = await inspectUniforms();
	assert.notEqual(latestFileInputs.revision, originalFileInputs.revision);
	assert.equal(
		latestFileInputs.rows.find(({ name }) => name === 'fileGain').parameter,
		previousField
	);
	const staleAdd = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload: {
			kind: 'AddShaderUniforms',
			reference,
			name: null,
			revision: originalFileInputs.revision
		}
	});
	assert.equal(staleAdd.success, false);
	assert.match(JSON.stringify(staleAdd), /Shader inputs changed/);
	assert.equal(
		(await inspectUniforms()).rows.find(({ name }) => name === 'fileBlue').parameter,
		null
	);
	await delay(500);
	await observer.observePixels('file-uniform-reload-preserves-edit', [255, 0, 255, 255]);
	await addBlue.click();
	await waitFor(async () =>
		Number.isInteger(
			(await inspectUniforms()).rows.find(({ name }) => name === 'fileBlue')?.parameter
		)
	);
	await setUniform('fileBlue', 0);
	await observer.observePixels('file-uniform-reloaded-input-live-edit', [255, 0, 0, 255]);
	assert((await post('/api/ui/project-save', { path: project })).ok);
	assert.equal(path.resolve((await post('/api/ui/project-load', { path: project })).path), project);
	await waitFor(async () => (await inspectUniforms()).rows.length === 2);
	await delay(750);
	await observer.observePixels('file-uniform-reopen', [255, 0, 0, 255]);
	const persistedGain = (await graph()).find(({ uuid }) => uuid === previousUuid);
	assert.equal(persistedGain?.data.param.value.Float, 1);
	assert.deepEqual(errors, []);
	assert(!appLog.includes('[mapgyver-output] window id='));
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				startupDisplay: selected ? { name: selected.name, index: selected.index } : null,
				externalDisplays: external.map(({ name, index }) => ({ name, index })),
				physicalWindowsOpened: 0,
				adapter: observer.adapter,
				pixels: observer.observations,
				errors
			},
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
