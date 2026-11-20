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
let requests = 0;

async function waitFor(check, label, timeout = 10000) {
	const deadline = performance.now() + timeout;
	while (!(await check())) {
		assert(performance.now() < deadline, label);
		await delay(50);
	}
}

const retirements = () =>
	host.log().split('[mapgyver-browser-preview] renderer retired').length - 1;

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

async function edit(payload, domain = 'media') {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${domain}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}

const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
const canvasIn = (view) => view.locator('canvas[data-preview-state="available"]');
const pixel = (canvas, x, y) =>
	canvas.evaluate(
		(image, [u, v]) => {
			const context = image.getContext('2d');
			return Array.from(
				context.getImageData(Math.floor(image.width * u), Math.floor(image.height * v), 1, 1).data
			).slice(0, 3);
		},
		[x, y]
	);
const rgbNear = (pixel, expected) =>
	pixel.every((value, index) => Math.abs(value - expected[index]) < 18);
const color = (pixel) => (pixel[0] > 240 ? 'red' : pixel[2] > 240 ? 'blue' : null);

async function verifyScreenEditors(reference) {
	const updateScreen = (payload) => edit(payload, 'screens');
	const {
		snapshot: {
			screens: [screen]
		}
	} = await updateScreen({ kind: 'InspectManager' });
	const [surface] = screen.surfaces;
	const quad = [
		{ x: 0.2, y: 0.2 },
		{ x: 0.8, y: 0.2 },
		{ x: 0.8, y: 0.8 },
		{ x: 0.2, y: 0.8 }
	];
	const crop = { x: 0.25, y: 0.2, width: 0.5, height: 0.6 };
	await updateScreen({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			enabled: true,
			target: {
				kind: 'Spout',
				sender: 'Browser Fixture Screen',
				extent: { width: 800, height: 500 }
			}
		}
	});
	const configure = (calibration) =>
		updateScreen({
			kind: 'ConfigureSurface',
			reference: surface.reference,
			definition: {
				...surface.definition,
				source: reference,
				crop,
				quad: { corners: quad },
				calibration
			}
		});
	await configure('Off');
	await tab('Screens').click();
	await page
		.getByRole('region', { name: 'Screens', exact: true })
		.getByRole('button', { name: surface.label, exact: true })
		.click();
	await tab('Screen Warping').click();
	const warp = page.getByRole('application', { name: 'Screen view', exact: true });
	const output = canvasIn(warp);
	const mappings = warp.getByRole('group', { name: 'Surface mapping controls', exact: true });
	await output.waitFor();
	const seen = new Set();
	await waitFor(async () => {
		const sample = color(await pixel(output, 0.5, 0.5));
		if (sample) seen.add(sample);
		return seen.size === 2;
	}, 'Screen Warping did not display both moving colors');
	await waitFor(
		async () => color(await pixel(output, 0.27, 0.28)) !== null,
		'warp witness must start inside the image'
	);
	assert(rgbNear(await pixel(output, 0.06, 0.06), [0, 0, 0]));
	const handle = warp.getByRole('button', { name: 'Warp corner 1', exact: true });
	const hit = await handle.boundingBox();
	const controls = await mappings.boundingBox();
	const image = await output.boundingBox();
	assert(hit && controls && image);
	assert(Math.abs(image.width / image.height - 8 / 5) < 0.01);
	for (const dimension of ['x', 'y', 'width', 'height'])
		assert(Math.abs(controls[dimension] - image[dimension]) < 1);
	await page.mouse.move(hit.x + hit.width / 2, hit.y + hit.height / 2);
	await page.mouse.down();
	try {
		await page.mouse.move(controls.x + controls.width * 0.45, controls.y + controls.height * 0.47, {
			steps: 8
		});
		await waitFor(
			async () => rgbNear(await pixel(output, 0.27, 0.28), [0, 0, 0]),
			'held warp drag did not update the rendered image'
		);
		assert(color(await pixel(output, 0.6, 0.62)));
	} finally {
		await page.mouse.up();
	}
	checks.push('Screen Warping showed moving content and updated while a DOM corner was held');

	await tab('Surface Source').click();
	const sourceView = page.getByRole('application', { name: 'Surface source view', exact: true });
	const source = canvasIn(sourceView);
	const cropControls = sourceView.getByRole('group', { name: 'Source crop controls', exact: true });
	await source.waitFor();
	await waitFor(
		async () => color(await pixel(source, 0.06, 0.06)) !== null,
		'Surface Source must show the full source outside the crop'
	);
	assert.equal(await cropControls.locator('rect').getAttribute('x'), '0.25');
	const sourceRect = await source.boundingBox();
	const cropRect = await cropControls.boundingBox();
	assert(sourceRect && cropRect);
	assert(Math.abs(sourceRect.width / sourceRect.height - 4 / 3) < 0.01);
	for (const dimension of ['x', 'y', 'width', 'height'])
		assert(Math.abs(cropRect[dimension] - sourceRect[dimension]) < 1);
	checks.push('Surface Source showed the full 4:3 source with aligned crop controls');

	await configure('ColorBars');
	await waitFor(
		async () =>
			(await source.isVisible()) && rgbNear(await pixel(source, 0.06, 0.5), [255, 255, 255]),
		'Surface Source did not render the full calibration pattern'
	);
	await tab('Screen Warping').click();
	await waitFor(
		async () =>
			(await output.isVisible()) && rgbNear(await pixel(output, 0.24, 0.5), [0, 255, 255]),
		'Screen Warping did not render the cropped calibration pattern'
	);
	checks.push(
		'Surface Source showed full calibration bars and Screen Warping showed the authored crop'
	);
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
		body: JSON.stringify({
			source: JSON.stringify({ kind: 'Channel', reference }),
			quality: 'high',
			after: null
		})
	});
	assert.equal(response.status, 401);
	checks.push('unauthenticated request refused');

	browser = await chromium.launch({ headless: true });
	page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('request', (request) => {
		if (request.url().endsWith('/api/ui/browser-preview')) requests++;
	});
	page.on('response', (response) => {
		if (!response.url().includes('/api/ui/browser-preview')) return;
		const status = response.status();
		responses.set(status, (responses.get(status) ?? 0) + 1);
	});
	await page.goto(host.base);
	const mode = page.getByRole('combobox', { name: 'Browser Preview Quality' });
	assert.equal(await mode.inputValue(), 'auto');
	assert.deepEqual(await mode.getByRole('option').allTextContents(), [
		'Disabled',
		'Auto',
		'Forced Low Quality',
		'Forced High Quality'
	]);
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
	const waitForQuality = (expected, width) =>
		waitFor(async () => {
			return (
				(await mode.getAttribute('data-effective-quality')) === expected &&
				(await preview.count()) === 1 &&
				(await preview.getAttribute('width')) === String(width)
			);
		}, `expected ${expected} preview at width ${width}`);
	await waitForQuality('high', 480);
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

	await mode.selectOption('low');
	await waitForQuality('low', 240);
	await mode.selectOption('high');
	await waitForQuality('high', 480);
	checks.push('forced low and high delivered distinct frame resolutions');

	await mode.selectOption('auto');
	await waitForQuality('high', 480);
	const cdp = await page.context().newCDPSession(page);
	const { ruleIds } = await cdp.send('Network.emulateNetworkConditionsByRule', {
		matchedNetworkConditions: [
			{ urlPattern: '', latency: 0, downloadThroughput: 8192, uploadThroughput: 8192 }
		]
	});
	assert.equal(ruleIds.length, 1);
	await waitForQuality('low', 240);
	assert.equal(await mode.inputValue(), 'auto');
	await cdp.send('Network.emulateNetworkConditionsByRule', { matchedNetworkConditions: [] });
	await waitForQuality('high', 480);
	await cdp.detach();
	checks.push('Auto switched to low at 8 KiB/s and recovered high after bandwidth returned');

	await mode.selectOption('disabled');
	const disabled = page.locator(
		'[data-panel-id="media-preview"] canvas[data-preview-state="disabled"]'
	);
	await disabled.waitFor();
	const disabledRequests = requests;
	await waitFor(() => retirements() === 1, 'preview renderer should retire on Disabled');
	assert.equal(requests, disabledRequests, 'Disabled should stop frame pulls');
	checks.push('Disabled aborted frame pulls and retired the renderer');

	await mode.selectOption('auto');
	await waitForQuality('high', 480);
	await verifyScreenEditors(reference);
	const beforeHide = retirements();
	await tab('Composition').click();
	await waitFor(
		() => retirements() === beforeHide + 1,
		'preview renderer should retire after the panel is hidden'
	);
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
	const quality = await page
		?.getByRole('combobox', { name: 'Browser Preview Quality' })
		.getAttribute('data-effective-quality', { timeout: 1000 })
		.catch(() => null);
	const diagnostics = {
		errors,
		requests,
		responses: Object.fromEntries(responses),
		quality,
		checks,
		preview
	};
	await writeFile(
		path.join(directory, 'failure.log'),
		`${error.stack}\n${JSON.stringify(diagnostics)}\n${host.log()}`
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
