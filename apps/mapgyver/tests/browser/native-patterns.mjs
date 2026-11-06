// Finite native color/alpha/reference acceptance through public app authoring.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { ownedWindowGeometry, ownedWindowHandle } from './capture-alignment.mjs';

const directory = path.resolve(process.argv[2]);
const gpu = process.argv[3];
assert(/^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu));
const windowState = JSON.parse(await readFile(path.resolve(process.argv[4]), 'utf8'));
await mkdir(directory, { recursive: true });
assert(
	!(await access(path.join(directory, 'ready.json')).then(
		() => true,
		() => false
	))
);
const python = 'apps/mapgyver/tests/native/pattern_pixels.py';
const pattern = JSON.parse(
	execFileSync('python', [python, directory, '--create'], { encoding: 'utf8' })
);
const appDataDirectory = path.join(directory, 'app-data');
const preferences = path.join(appDataDirectory, 'MapGyver');
await mkdir(preferences, { recursive: true });
await writeFile(path.join(preferences, 'window-state.json'), JSON.stringify(windowState));
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = JSON.parse(
	execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
execFileSync(
	observer,
	[
		'--role',
		'gpu-preferences',
		'--file',
		path.join(preferences, 'preferences.json'),
		'--selected',
		gpu
	],
	{ windowsHide: true }
);
const native = await openNativeEditor({ appDataDirectory });
const page = native.page;
const errors = [],
	stages = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token;
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
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.' + namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const tab = (expression) => page.getByRole('tab').filter({ hasText: expression });
try {
	await page.getByRole('tab').first().waitFor();
	token = (await post('/api/ui/session', {})).token;
	const { reference: picture } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'Picture',
		label: 'RGBA reference pattern'
	});
	await edit('media', {
		kind: 'ImportPicture',
		target: picture,
		locator: { kind: 'absolute', path: path.join(directory, 'rgba-pattern.png') },
		label: null
	});
	const references = [];
	let target = picture;
	for (let index = 0; index < 2; index++) {
		const { media } = await edit('media', {
			kind: 'CreateReference',
			target: target.media,
			label: `Pattern Reference ${index + 1}`
		});
		const { page: channels } = await edit('media', {
			kind: 'InspectChannels',
			query: { media, start: 0, count: 2 }
		});
		assert.equal(channels.channels.length, 1);
		assert.deepEqual(channels.channels[0].definition.source, { kind: 'Reference', target });
		target = channels.channels[0].reference;
		references.push(target);
	}
	const { screen, surface } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Pattern screen'
	});
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Pattern-${screen}`, extent: { width: 800, height: 600 } }
		}
	});
	const base = (await edit('screens', { kind: 'InspectSurface', reference: surface })).snapshot;
	const rectangle = [
		{ x: 0.2, y: 0.2 },
		{ x: 0.8, y: 0.2 },
		{ x: 0.8, y: 0.8 },
		{ x: 0.2, y: 0.8 }
	];
	const perspective = [
		{ x: 0.12, y: 0.22 },
		{ x: 0.82, y: 0.12 },
		{ x: 0.9, y: 0.82 },
		{ x: 0.18, y: 0.9 }
	];
	const cases = [
		{ name: 'direct', source: picture, crop: { x: 0, y: 0, width: 1, height: 1 }, quad: rectangle },
		{
			name: 'reference',
			source: target,
			crop: { x: 0, y: 0, width: 1, height: 1 },
			quad: rectangle
		},
		{
			name: 'crop-perspective',
			source: target,
			crop: { x: 0.1, y: 0.15, width: 0.7, height: 0.65 },
			quad: perspective
		}
	];
	async function configure(value) {
		await edit('screens', {
			kind: 'ConfigureSurface',
			reference: surface,
			definition: {
				...base.definition,
				source: value.source,
				crop: value.crop,
				quad: { corners: value.quad }
			}
		});
		const actual = (await edit('screens', { kind: 'InspectSurface', reference: surface })).snapshot;
		assert.deepEqual(actual.definition.source, value.source);
		assert.equal(actual.mapping_valid, true);
	}
	async function selectSurface() {
		await tab(/^Screens$/).click();
		await page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: base.label, exact: true })
			.click();
	}
	await configure(cases[0]);
	await selectSurface();
	await tab(/^Screen Warping(?::|$)/).click();
	await page
		.getByRole('application', { name: 'Screen view', exact: true })
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	const adapter = await observeNativeAdapter(page, hardware);
	assert.equal(adapter.key, gpu);
	await writeFile(
		path.join(directory, 'ready.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				gpu,
				directory,
				picture,
				references,
				surface,
				pattern,
				base: native.base
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			activate_owned_window: native.app.pid,
			gpu,
			resume: path.join(directory, 'capture')
		})
	);
	const deadline = Date.now() + 480000;
	while (
		!(await access(path.join(directory, 'capture')).then(
			() => true,
			() => false
		))
	) {
		assert(Date.now() < deadline, 'Owned pattern fixture foreground wait expired');
		await delay(50);
	}
	const hwnd = ownedWindowHandle(native.app.pid);
	const original = ownedWindowGeometry(native.app.pid, hwnd);
	async function viewFor(editor) {
		await tab(editor === 'screen' ? /^Screen Warping(?::|$)/ : /^Surface Source(?::|$)/).click();
		const view = page.getByRole('application', {
			name: editor === 'screen' ? 'Screen view' : 'Surface source view',
			exact: true
		});
		await view
			.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
			.waitFor();
		await view.getByRole('button', { name: 'Home view', exact: true }).click();
		await delay(250);
		return view;
	}
	async function capture(value, editor, selected, view) {
		const stage = `${value.name}-${editor}-${selected ? 'selected' : 'plain'}`;
		const output = path.join(directory, stage);
		await mkdir(output);
		const projection = () =>
			view.evaluate((element) => {
				const rect = element.querySelector('.active-zone').getBoundingClientRect();
				return {
					viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
					active: { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
				};
			});
		const before = await projection();
		await page.mouse.move(10, 15);
		await delay(150);
		await writeFile(
			path.join(output, 'projection.json'),
			JSON.stringify(
				{
					...before,
					editor,
					selected,
					crop: value.crop,
					quad: value.quad.map((p) => [p.x, p.y]),
					pattern
				},
				null,
				2
			)
		);
		const pixels = JSON.parse(
			execFileSync('python', [python, output, '--hwnd', hwnd, '--pid', String(native.app.pid)], {
				encoding: 'utf8',
				windowsHide: true
			})
		);
		assert.deepEqual(await projection(), before);
		assert.deepEqual(ownedWindowGeometry(native.app.pid, hwnd), original);
		assert.equal((await observeNativeAdapter(page, hardware)).key, gpu);
		stages.push({ stage, pixels });
		console.log(JSON.stringify({ stage, ...pixels }));
	}
	for (const value of cases) {
		await configure(value);
		await selectSurface();
		const screenView = await viewFor('screen');
		await screenView.locator('polygon.selected').waitFor();
		await capture(value, 'screen', true, screenView);
		const active = await screenView.locator('.active-zone').boundingBox();
		await page.mouse.click(active.x + active.width * 0.02, active.y + active.height * 0.02);
		await page.waitForFunction(
			() => !document.querySelector('[aria-label="Screen view"] polygon.selected')
		);
		await delay(150);
		await capture(value, 'screen', false, screenView);
		await selectSurface();
		const sourceView = await viewFor('source');
		await capture(value, 'source', true, sourceView);
	}
	assert.deepEqual(errors, []);
	assert(
		stages.every(({ pixels }) => pixels.passed),
		'Original RGB <=2 color/alpha limits failed; all observed stages retained'
	);
	await writeFile(
		path.join(directory, 'pattern-result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: native.app.pid,
				hwnd: Number(hwnd),
				adapter,
				geometry: original,
				stages,
				page_errors: errors
			},
			null,
			2
		)
	);
} catch (error) {
	await writeFile(
		path.join(directory, 'pattern-failure.json'),
		JSON.stringify(
			{ error: String(error), stages, page_errors: errors, log: native.log() },
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'lifecycle.json'),
		JSON.stringify({ normal_exit: native.app.exitCode }, null, 2)
	);
}
