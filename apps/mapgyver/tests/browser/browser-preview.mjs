import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
assert(process.env.MAPGYVER_TEST_BINARY, 'Use a compiled MapGyver binary');
await mkdir(directory);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser;
let page;
let token;
const errors = [];
const checks = [];
const responses = new Map();

async function post(route, payload) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		headers: {
			'content-type': 'application/json',
			...(token ? { authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(payload),
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}

async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Shader',
		label: 'Moving shader'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference,
		source: {
			kind: 'Shader',
			format: 'GLSL',
			input: {
				kind: 'Embedded',
				code: `void main() {
					bool red = fract(iTime * 2.0) < 0.5;
					fragColor = red ? vec4(1.0, 0.0, 0.0, 1.0) : vec4(0.0, 0.0, 1.0, 1.0);
				}`
			},
			textures: [null, null, null, null]
		}
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 320, height: 240 } }
	});
	const response = await fetch(`${host.base}/api/ui/browser-preview`, {
		method: 'POST',
		body: JSON.stringify({ kind: 'Channel', reference })
	});
	assert.equal(response.status, 401);
	checks.push('unauthenticated request refused');

	browser = await chromium.launch({ headless: true });
	page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('response', (response) => {
		if (!response.url().includes('/api/ui/browser-preview')) return;
		const status = response.status();
		responses.set(status, (responses.get(status) ?? 0) + 1);
	});
	await page.goto(host.base);
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
	await tab('Medias').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Moving shader', exact: true })
		.click();
	await tab('Media Preview').click();
	const preview = page.locator(
		'[data-panel-id="media-preview"] canvas[data-preview-state="available"]'
	);
	await preview.waitFor({ timeout: 15000 });
	const seen = new Set();
	const deadline = performance.now() + 6000;
	while (seen.size < 2 && performance.now() < deadline) {
		const frame = await preview.evaluate((canvas) => {
			const context = canvas.getContext('2d');
			return {
				width: canvas.width,
				height: canvas.height,
				pixel: Array.from(context.getImageData(canvas.width / 2, canvas.height / 2, 1, 1).data)
			};
		});
		assert.equal(frame.width * 3, frame.height * 4, 'preserve the source aspect');
		const [red, green, blue, alpha] = frame.pixel;
		assert(green < 10 && alpha === 255);
		if (red > 240 && blue < 10) seen.add('red');
		if (blue > 240 && red < 10) seen.add('blue');
		await delay(35);
	}
	assert.deepEqual([...seen].sort(), ['blue', 'red']);
	checks.push('standalone browser displayed moving 4:3 GPU images');

	await tab('Screen Warping').click();
	const retireDeadline = performance.now() + 6000;
	while (!host.log().includes('[mapgyver-browser-preview] renderer retired')) {
		assert(
			performance.now() < retireDeadline,
			'preview renderer should retire after the panel is hidden'
		);
		await delay(50);
	}
	checks.push('hidden preview stopped demand and the native renderer retired');
	assert.deepEqual(errors, []);
	assert(!host.log().includes('[mapgyver-output] window id='));
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', checks, errors }, null, 2)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	const preview = await page?.locator('[data-panel-id="media-preview"]').evaluateAll((panels) =>
		panels.map((panel) => {
			const image = panel.querySelector('canvas');
			return {
				content: panel.textContent,
				image: image?.outerHTML,
				bounds: image?.getBoundingClientRect().toJSON()
			};
		})
	);
	const diagnostics = { errors, responses: Object.fromEntries(responses), preview };
	await writeFile(
		path.join(directory, 'failure.log'),
		`${error.stack}\n${JSON.stringify(diagnostics)}\n${host.log()}`
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
