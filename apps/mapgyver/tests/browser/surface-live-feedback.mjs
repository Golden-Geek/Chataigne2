// Real owned WebView input. Measures submitted requests and DOM geometry, not physical scanout.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
const { page } = native;
const client = await page.context().newCDPSession(page);
const requests = [],
	receipts = [],
	phases = [],
	errors = [];
let phase = 'setup',
	token,
	failure;
page.on('pageerror', (error) => errors.push(String(error)));
client.on('Network.webSocketFrameSent', ({ response }) => {
	try {
		const frame = JSON.parse(response.payloadData);
		if (frame.kind === 'intent')
			requests.push({ phase, at: performance.now(), id: frame.request_id, intent: frame.intent });
		if (frame.kind === 'intentBatch')
			requests.push(
				...frame.intents.map((intent) => ({
					phase,
					at: performance.now(),
					id: frame.request_id,
					intent
				}))
			);
	} catch {}
});
client.on('Network.webSocketFrameReceived', ({ response }) => {
	try {
		const frame = JSON.parse(response.payloadData);
		if (frame.kind === 'control' && ['applied', 'rejected'].includes(frame.update.phase))
			receipts.push({ phase, at: performance.now(), ...frame.update });
	} catch {}
});
await client.send('Network.enable');
async function post(route, body) {
	const response = await fetch(native.base + route, {
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
async function op(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function wait(read, ready) {
	const until = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < until, `${phase}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}
const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
const shapeValue = (shape) =>
	shape.evaluate((element) =>
		element.tagName.toLowerCase() === 'rect'
			? ['x', 'y', 'width', 'height'].map((key) => element.getAttribute(key)).join(',')
			: element.getAttribute('points')
	);
async function drag(name, control, shape, dx, dy, alt = false) {
	phase = name;
	await control.scrollIntoViewIfNeeded();
	const box = await control.boundingBox();
	assert(box);
	const point = { x: box.x + box.width * 0.5, y: box.y + box.height * 0.5 };
	const before = await shapeValue(shape),
		samples = [];
	await page.mouse.move(point.x, point.y);
	if (alt) await page.keyboard.down('Alt');
	const start = performance.now();
	await page.mouse.down();
	try {
		for (let index = 1; index <= 48; index++) {
			await page.mouse.move(point.x + (dx * index) / 48, point.y + (dy * index) / 48);
			samples.push({ ms: performance.now() - start, shape: await shapeValue(shape) });
			await delay(16);
		}
	} finally {
		await page.mouse.up();
		if (alt) await page.keyboard.up('Alt');
	}
	const held_ms = samples.at(-1).ms;
	await delay(250);
	const end = performance.now(),
		after = await shapeValue(shape);
	let previous = before,
		changes = 0;
	for (const sample of samples) {
		if (sample.shape !== previous) changes++;
		previous = sample.shape;
	}
	const sent = requests.filter(
		(entry) => entry.phase === name && entry.at >= start && entry.at <= end
	);
	const counts = {};
	for (const entry of sent) {
		const key =
			entry.intent.kind === 'invokeAppOperation' ? entry.intent.payload?.kind : entry.intent.kind;
		counts[key] = (counts[key] ?? 0) + 1;
	}
	phases.push({ name, held_ms, in_progress_dom_changes: changes, before, after, counts, samples });
}
try {
	token = (await post('/api/ui/session', {})).token;
	const source = (
		await op('mapgyver.media', {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Live geometry source'
		})
	).reference;
	await op('mapgyver.media', {
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 640, height: 360 } }
	});
	const { screen, surface } = await op('mapgyver.screens', {
		kind: 'CreateScreen',
		label: 'Live geometry Screen'
	});
	await op('mapgyver.screens', {
		kind: 'RenameSurface',
		reference: surface,
		label: 'Live geometry Surface'
	});
	await op('mapgyver.screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: {
				kind: 'Spout',
				sender: `LiveGeometry-${screen}`,
				extent: { width: 640, height: 360 }
			}
		}
	});
	const inspect = () => op('mapgyver.screens', { kind: 'InspectSurface', reference: surface });
	const original = (await inspect()).snapshot;
	await op('mapgyver.screens', {
		kind: 'ConfigureSurface',
		reference: surface,
		definition: {
			...original.definition,
			source,
			crop: { x: 0.1, y: 0.1, width: 0.6, height: 0.6 },
			quad: {
				corners: [
					{ x: 0.2, y: 0.2 },
					{ x: 0.8, y: 0.2 },
					{ x: 0.8, y: 0.8 },
					{ x: 0.2, y: 0.8 }
				]
			}
		}
	});
	await tab('Screens').click();
	await page
		.getByRole('region', { name: 'Screens', exact: true })
		.getByRole('button', { name: 'Live geometry Surface', exact: true })
		.click();
	await tab('Screen Warping').click();
	const warp = page.locator('svg[aria-label="Surface mapping controls"]');
	const polygon = warp.locator(`polygon[data-surface-drop-target="${surface.surface}"]`);
	await polygon.waitFor();
	await drag(
		'canvas-corner',
		page.getByRole('button', { name: 'Warp corner 1', exact: true }),
		polygon,
		35,
		12
	);
	const row = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	await drag(
		'inspector-corner',
		row('Top left').locator('.infinite-slider').first(),
		polygon,
		14,
		0,
		true
	);
	await tab('Surface Source').click();
	const crop = page.locator(
		'svg[aria-label="Source crop controls"] rect[data-native-hit-id="crop"]'
	);
	await crop.waitFor();
	await drag('inspector-crop-origin', row('Crop origin').locator('.slider').first(), crop, 12, 0);
	await drag('inspector-crop-size', row('Crop size').locator('.slider').first(), crop, -12, 0);
	await drag('canvas-crop', crop, crop, 20, 8);
	if (process.argv.includes('--expect-live')) {
		for (const result of phases)
			assert(
				result.in_progress_dom_changes > 2,
				`${result.name}: geometry must update while held, not only on release`
			);
	}
	assert.deepEqual(errors, []);
	await page.screenshot({ path: path.join(directory, 'surface.png') });
} catch (error) {
	failure = error;
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: failure ? 'failed' : 'pass',
				pid: native.app.pid,
				phases,
				requests,
				receipts,
				errors,
				failure: failure ? String(failure.stack ?? failure) : null,
				limit:
					'Trusted input, DOM geometry and ordinary wire receipts only. Neither presented native pixels nor physical latency/pacing qualification.'
			},
			null,
			2
		)
	);
}
console.log(JSON.stringify({ phases: phases.map(({ samples, ...rest }) => rest) }));
if (failure) throw failure;
