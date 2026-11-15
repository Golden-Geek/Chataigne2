import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { chromium } from 'playwright-core';
const configFile = fileURLToPath(new URL('./vite.config.ts', import.meta.url));
await build({ configFile, logLevel: 'warn' });
const server = await preview({ configFile, logLevel: 'warn' });
let browser;
try {
	browser = await chromium.launch({ channel: 'msedge', headless: true });
	const page = await browser.newPage({ viewport: { width: 1280, height: 960 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => {
		errors.push(error.message);
		console.error(error.message);
	});
	await page.goto(`http://127.0.0.1:${server.httpServer.address().port}`);
	const primary = page.getByLabel('Primary curve Inspector', { exact: true });
	const canvas = primary.locator('canvas.curve-canvas');
	await canvas.waitFor();
	await page.evaluate(() => document.fonts.ready);
	const original = await canvas.elementHandle();
	assert(original);
	const selected = page.getByLabel('Selected curve nodes');
	await page.getByRole('button', { name: 'Run curve guard checks', exact: true }).click();
	assert.equal(await page.getByLabel('Curve guard checks').textContent(), 'pass');
	await page.getByRole('button', { name: 'Select Curve A key 0', exact: true }).click();
	assert.equal(await selected.textContent(), 'Curve A key 0');
	assert(
		await canvas.evaluate((current, old) => current === old, original),
		'key selection keeps the editor mounted'
	);
	assert.equal(await primary.locator('.selected-key-node-inspector').count(), 1);
	const beforeField = await canvas.boundingBox();
	await page.getByRole('button', { name: 'Select key value', exact: true }).click();
	assert.equal(
		await selected.textContent(),
		'Value',
		'field selection stays on the real Parameter'
	);
	assert(await canvas.evaluate((current, old) => current === old, original));
	assert.equal(await primary.locator('.watcher-wrapper').count(), 1);
	assert.deepEqual(await canvas.boundingBox(), beforeField, 'field Watcher cannot move the canvas');
	await page.getByRole('button', { name: 'Select both keys', exact: true }).click();
	assert.equal(await primary.locator('.watcher-wrapper').count(), 0);
	assert.equal(await selected.textContent(), 'Curve A key 0|Curve A key 1');
	assert(await canvas.evaluate((current, old) => current === old, original));
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	await page.waitForFunction(() => {
		const element = document.querySelector('canvas'),
			context = element?.getContext('2d');
		if (!element || !context || element.width < 100) return false;
		const pixels = context.getImageData(0, 0, element.width, element.height).data;
		let left = Infinity,
			right = -Infinity;
		for (let i = 0; i < pixels.length; i += 4)
			if (pixels[i] === 177 && pixels[i + 1] === 186 && pixels[i + 2] === 201) {
				const x = (i / 4) % element.width;
				left = Math.min(left, x);
				right = Math.max(right, x);
			}
		return right - left > element.width * 0.25;
	});
	const points = await canvas.evaluate((element) => {
		const pixels = element.getContext('2d').getImageData(0, 0, element.width, element.height).data;
		const dots = [];
		for (let i = 0; i < pixels.length; i += 4)
			if (pixels[i] === 177 && pixels[i + 1] === 186 && pixels[i + 2] === 201)
				dots.push({ x: (i / 4) % element.width, y: Math.floor(i / 4 / element.width) });
		const left = Math.min(...dots.map((point) => point.x)),
			right = Math.max(...dots.map((point) => point.x));
		const box = element.getBoundingClientRect();
		return [left, right].map((x) => {
			const group = dots.filter((point) => Math.abs(point.x - x) < 6);
			return {
				x:
					box.x +
					((group.reduce((sum, point) => sum + point.x, 0) / group.length) * box.width) /
						element.width,
				y:
					box.y +
					((group.reduce((sum, point) => sum + point.y, 0) / group.length) * box.height) /
						element.height
			};
		});
	});
	await canvas.evaluate((element) =>
		element.addEventListener(
			'pointerdown',
			(event) => {
				element.dataset.testPointerId = String(event.pointerId);
			},
			{ once: true, capture: true }
		)
	);
	await page.mouse.move(points[0].x, points[0].y);
	await page.mouse.down();
	assert.equal(
		await selected.textContent(),
		'Curve A key 0',
		'canvas pointer-down updates workbench before release'
	);
	assert(await canvas.evaluate((current, old) => current === old, original));
	assert(
		await canvas.evaluate((element) =>
			element.hasPointerCapture(Number(element.dataset.testPointerId))
		),
		'selection keeps owned capture'
	);
	await page.mouse.up();
	await page.keyboard.down('Shift');
	await page.mouse.move(points[1].x, points[1].y);
	await page.mouse.down();
	assert.equal(await selected.textContent(), 'Curve A key 1|Curve A key 0');
	await page.mouse.up();
	await page.keyboard.up('Shift');
	assert.equal(
		await page.getByLabel('Curve writes').textContent(),
		'0',
		'selection does not mutate Parameters'
	);
	await page.getByRole('button', { name: 'Open locked editors', exact: true }).click();
	await page.getByRole('button', { name: 'Select Curve B key 0', exact: true }).click();
	assert.equal(await primary.locator('canvas.curve-canvas').count(), 1);
	assert.equal(
		await page.getByLabel('Other curve Inspector').locator('canvas.curve-canvas').count(),
		1
	);
	assert(await canvas.evaluate((current, old) => current === old, original));
	assert.equal(
		await selected.textContent(),
		'Curve B key 0',
		'another editor cannot steal external selection'
	);
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'curve Inspector owner anchoring, real key pointer-down/capture, multi/field/external selection and locked editors; input cancellation and native qualification remain open'
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
