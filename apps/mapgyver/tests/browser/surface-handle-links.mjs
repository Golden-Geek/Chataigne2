// Real editor input, retained pointer and independent SDK output pixels from one owned scene.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const host = await openNativeEditor();
const { page } = host;
let token,
	stage = 'prepare';
const checks = [],
	errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
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
async function intent(body) {
	const ack = await post('/api/ui/intent', body);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}
const edit = (namespace, payload) =>
	intent({
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
const screens = (payload) => edit('screens', payload);
const surface = async (reference) =>
	(await screens({ kind: 'InspectSurface', reference })).snapshot;
const panel = page.locator('section.workbench-editor[data-panel-id="screen-warp"]');
async function settled() {
	await page.waitForFunction(
		() =>
			document
				.querySelector('section.workbench-editor[data-panel-id="screen-warp"]')
				?.getAttribute('aria-busy') === 'false'
	);
}
async function select(label) {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: label, exact: true })
		.click();
	await page.locator('.gc-dv-tab[data-content-panel-id="screen-warp"]').click();
	await settled();
}
async function move(corner, point, release = true, offset = 0) {
	const handle = panel.getByRole('button', { name: `Warp corner ${corner + 1}`, exact: true });
	const box = await handle.boundingBox(),
		rect = await panel.locator('svg[aria-label="Surface mapping controls"]').boundingBox();
	assert(box && rect);
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(rect.x + point.x * rect.width + offset, rect.y + point.y * rect.height, {
		steps: 8
	});
	if (release) {
		await page.mouse.up();
		await settled();
	}
}
async function pointEqual(reference, corner, expected) {
	const deadline = Date.now() + 5000;
	for (;;) {
		const point = (await surface(reference)).definition.quad.corners[corner];
		if (Math.abs(point.x - expected.x) < 1e-6 && Math.abs(point.y - expected.y) < 1e-6) return;
		assert(Date.now() < deadline, `corner did not update while held: ${JSON.stringify(point)}`);
		await delay(30);
	}
}
async function review(name) {
	if (!process.argv.includes('--review') || name === 'initial') return;
	const hwnd = ownedWindowHandle(host.app.pid);
	await writeFile(
		path.join(directory, `review-${name}.json`),
		JSON.stringify({ pid: host.app.pid, hwnd, stage })
	);
	console.log(JSON.stringify({ readyForReview: name, pid: host.app.pid }));
	const deadline = Date.now() + 8 * 60 * 1000;
	for (;;) {
		try {
			await access(path.join(directory, `review-${name}.done`));
			break;
		} catch {}
		assert.equal(host.app.exitCode, null);
		assert(Date.now() < deadline, `${name} review expired`);
		await delay(100);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const initial = (await screens({ kind: 'InspectManager' })).snapshot.screens[0];
	const a = initial.surfaces[0].reference;
	const b = (await screens({ kind: 'AddSurface', screen: initial.id, label: 'Linked Blue' }))
		.reference;
	const c = (await screens({ kind: 'AddSurface', screen: initial.id, label: 'Third peer' }))
		.reference;
	await screens({ kind: 'RenameSurface', reference: a, label: 'Linked Red' });
	for (const [reference, label, rgba, corners] of [
		[
			a,
			'Red source',
			[1, 0, 0, 1],
			[
				[0.1, 0.2],
				[0.45, 0.2],
				[0.45, 0.8],
				[0.1, 0.8]
			]
		],
		[
			b,
			'Blue source',
			[0, 0, 1, 1],
			[
				[0.55, 0.2],
				[0.9, 0.2],
				[0.9, 0.8],
				[0.55, 0.8]
			]
		]
	]) {
		const { reference: source } = await edit('media', {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label
		});
		await edit('media', {
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba }
		});
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: {
				...(await surface(reference)).definition,
				source,
				quad: { corners: corners.map(([x, y]) => ({ x, y })) }
			}
		});
	}
	await screens({
		kind: 'ConfigureSurface',
		reference: c,
		definition: {
			...(await surface(c)).definition,
			quad: {
				corners: [
					[0.5, 0.4],
					[0.65, 0.4],
					[0.65, 0.55],
					[0.5, 0.55]
				].map(([x, y]) => ({ x, y }))
			}
		}
	});
	const sender = `MapGyver-Link-${randomUUID()}`;
	await screens({
		kind: 'ConfigureScreen',
		screen: initial.id,
		definition: {
			...initial.definition,
			enabled: true,
			target: { kind: 'Spout', sender, extent: { width: 256, height: 160 } },
			anti_aliasing: 'Raw'
		}
	});
	const probe = path.resolve('target/debug/native_qualification.exe');
	const hardware = JSON.parse(
		execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
	);
	// The owned WebView can still be completing its initial host navigation after CDP attaches.
	// Selecting the real workbench first waits for the connected application, not its bootstrap page.
	await select('Linked Red');
	const adapter = await observeNativeAdapter(page, hardware);
	async function pixels(name) {
		await delay(300);
		const target = path.join(directory, name);
		const received = JSON.parse(
			execFileSync(
				probe,
				[
					'--role',
					'pixels',
					'--adapter',
					String(adapter.index),
					'--name',
					sender,
					'--directory',
					target
				],
				{ encoding: 'utf8', windowsHide: true, timeout: 15000 }
			)
		);
		assert.deepEqual(received, { width: 256, height: 160, retired: true });
		const layers = [
			{ corners: (await surface(a)).definition.quad.corners, rgb: [255, 0, 0], label: 'red' },
			{ corners: (await surface(b)).definition.quad.corners, rgb: [0, 0, 255], label: 'blue' }
		];
		const oracle = JSON.parse(
			execFileSync(
				'python',
				[
					'apps/mapgyver/tests/native/linked_surface_pixels.py',
					path.join(target, 'pixels.png'),
					JSON.stringify(layers)
				],
				{ encoding: 'utf8', windowsHide: true }
			)
		);
		checks.push({ stage: name, received, oracle });
	}
	const magnet = panel.getByRole('button', { name: 'Snap Surface handles', exact: true });
	assert.equal(await magnet.getAttribute('aria-pressed'), 'false');
	assert.equal(
		await magnet.locator('img').evaluate((node) => getComputedStyle(node).filter),
		'grayscale(1)'
	);
	await magnet.click();
	await settled();
	assert.equal(
		await magnet.locator('img').evaluate((node) => getComputedStyle(node).filter),
		'none'
	);
	assert.equal(
		await panel.getByRole('button', { name: /(?:Previous|Next) Screen handles/ }).count(),
		0
	);
	stage = 'ready with colored magnet and two Surface outputs';
	await review('initial');
	const originalA = await surface(a);
	await move(1, { x: 0.55, y: 0.2 }, true, 3);
	await panel.getByRole('button', { name: 'Unlink corner group 2', exact: true }).waitFor();
	await pointEqual(a, 1, { x: 0.55, y: 0.2 });
	await intent({ kind: 'undo' });
	await settled();
	assert.deepEqual((await surface(a)).definition.quad, originalA.definition.quad);
	await panel
		.getByRole('button', { name: 'Unlink corner group 2', exact: true })
		.waitFor({ state: 'detached' });
	assert.equal(
		await panel.getByRole('button', { name: 'Unlink corner group 2', exact: true }).count(),
		0
	);
	await intent({ kind: 'redo' });
	await settled();
	await panel.getByRole('button', { name: 'Unlink corner group 2', exact: true }).waitFor();
	checks.push('drag and link share one Undo/Redo step');
	await magnet.click();
	await settled();
	const held = { x: 0.5, y: 0.35 };
	await move(1, held, false);
	await pointEqual(a, 1, held);
	await pointEqual(b, 0, held);
	await page.waitForFunction(
		({ id, expected }) => {
			const polygon = document.querySelector(
				`section[data-panel-id="screen-warp"] polygon[data-native-hit-id="${id}"]`
			);
			const point = polygon?.getAttribute('points')?.split(' ')[0].split(',').map(Number);
			return (
				point && Math.abs(point[0] - expected.x) < 1e-6 && Math.abs(point[1] - expected.y) < 1e-6
			);
		},
		{ id: b.surface, expected: held }
	);
	await pixels('two-surfaces-while-held');
	stage = 'both linked Surface contours and red/blue output updated before release';
	await page.mouse.up();
	await settled();
	await review('held');
	await magnet.click();
	await settled();
	await select('Linked Blue');
	await move(0, { x: 0.5, y: 0.4 }, true, 2);
	await pointEqual(a, 1, { x: 0.5, y: 0.4 });
	await pointEqual(b, 0, { x: 0.5, y: 0.4 });
	const document = (
		await screens({
			kind: 'InspectEditor',
			query: {
				editor: 'Screen',
				selected: b.surface,
				document: null,
				locked: null,
				retained: null,
				surface: b.surface
			}
		})
	).page.document;
	const groups = (
		await screens({
			kind: 'InspectScreenHandles',
			query: { document, reference: b, cursor: null, include_candidates: false }
		})
	).page.groups;
	assert.equal(groups.find((group) => group.corner === 0).members.length, 3);
	await pixels('three-member-linked');
	await panel.getByRole('button', { name: 'Unlink corner group 1', exact: true }).click();
	await settled();
	await panel
		.getByRole('button', { name: 'Unlink corner group 1', exact: true })
		.waitFor({ state: 'detached' });
	assert.equal(
		await panel.getByRole('button', { name: 'Unlink corner group 1', exact: true }).count(),
		0
	);
	checks.push('three-member merge and icon unlink');
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ status: 'pass', pid: host.app.pid, adapter, checks }, null, 2)
	);
	console.log(JSON.stringify({ status: 'pass', pid: host.app.pid, checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, log: host.log() }, null, 2)
	);
	throw error;
} finally {
	await host.close();
}
