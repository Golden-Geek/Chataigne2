// An isolated Windows app copy with every NDI runtime candidate missing or invalid.
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

assert(process.env.MAPGYVER_TEST_BINARY, 'Select the isolated app executable');
const root = path.resolve(process.argv[2]);
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const native = await openNativeEditor({
	environment: { NDI_RUNTIME_DIR_V6: directory, GOLDEN_NDI_AUTOMATION_SKIP_SYSTEM_RUNTIME: '1' }
});
const { page } = native;
const checks = [];
const pageErrors = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
let token, stage, observer, failure;

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
	const data = await response.json();
	assert(response.ok, JSON.stringify(data));
	return data;
}

async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}

async function select(manager, label) {
	await page
		.getByRole('tab')
		.filter({ hasText: new RegExp(`^${manager}$`) })
		.click();
	await page
		.locator(`section[aria-label="${manager}"]`)
		.getByRole('button', { name: label, exact: true })
		.click();
	await page.locator('.inspector-header .title-text').filter({ hasText: label }).first().waitFor();
}

const guide = page
	.locator('.inspector')
	.getByText(/Install or update the NDI 6 Runtime for Windows x64/)
	.filter({ visible: true })
	.first();
const color = (reference, rgba) =>
	edit('media', { kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });

try {
	token = (await post('/api/ui/session', {})).token;
	observer = await createPreviewOutputObserver(post, path.join(directory, 'spout'), page);
	stage = 'establish ordinary output';
	const { reference: solid } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Ordinary source'
	});
	await color(solid, [1, 0, 0, 1]);
	await observer.demand(solid);
	await observer.observePixels('before', [255, 0, 0, 255]);

	stage = 'NDI Media shows actionable guidance';
	const { reference } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'Ndi',
		label: 'NDI input'
	});
	await select('Medias', 'NDI input');
	await guide.waitFor({ timeout: 8500 });
	checks.push({ stage, text: await guide.textContent(), channel: reference.channel });

	stage = 'ordinary output keeps updating during NDI failure';
	const { screen } = await edit('screens', { kind: 'CreateScreen', label: 'NDI output' });
	const extent = { width: 96, height: 64 };
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: { enabled: true, target: { kind: 'Ndi', sender: null, extent } }
	});
	await color(solid, [0, 1, 0, 1]);
	await observer.observePixels('while-ndi-unavailable', [0, 255, 0, 255]);

	stage = 'NDI Screen shows actionable guidance';
	await select('Screens', 'NDI output');
	await guide.waitFor({ timeout: 8500 });
	checks.push({ stage, text: await guide.textContent(), screen });

	stage = 'switch the affected Screen to Shared Texture';
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: { enabled: true, target: { kind: 'AutomaticSpout', extent } }
	});
	await guide.waitFor({ state: 'hidden', timeout: 8500 });
	await observer.observePixels('changed-target', [0, 255, 0, 255], 0, 'MapGyver - NDI output');
	checks.push({ stage, guidanceCleared: true });
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	const log = native.log();
	await native.close();
	const resultPath = path.join(directory, 'result.json');
	await writeFile(
		resultPath,
		JSON.stringify(
			{
				result: failure ? 'fail' : 'pass',
				stage,
				failure,
				checks,
				pixels: observer?.observations,
				pageErrors,
				log: failure ? log : undefined
			},
			null,
			2
		)
	);
	assert(!failure, `${resultPath}\n${failure}`);
	console.log(resultPath);
}
