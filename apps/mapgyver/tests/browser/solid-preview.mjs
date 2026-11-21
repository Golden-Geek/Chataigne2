import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

assert(process.env.MAPGYVER_TEST_BINARY, 'Select a compiled MapGyver with preview enabled');
const directory = await mkdtemp(join(tmpdir(), 'mapgyver-solid-preview-'));
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser;
let token;

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
	const label = 'Portable Solid Color';
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
	const setColor = (rgba) =>
		edit({ kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 320, height: 240 } }
	});
	await setColor([1, 0, 0, 1]);
	browser = await chromium.launch({ headless: true, channel: process.env.GC_UI_BROWSER_CHANNEL });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(15000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('response', (response) => {
		if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`);
	});
	await page.goto(host.base);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview(?::|$)/ })
		.click();
	await page.getByRole('combobox', { name: 'Browser Preview Quality' }).selectOption('high');
	const selector = '[data-panel-id="media-preview"] canvas[data-preview-state="available"]';
	const waitForColor = async (rgb) => {
		await page.waitForFunction(
			({ selector, rgb }) => {
				const canvas = document.querySelector(selector);
				if (!canvas || canvas.width !== 480 || canvas.height !== 360) return false;
				const pixel = canvas.getContext('2d').getImageData(240, 180, 1, 1).data;
				return pixel[3] === 255 && rgb.every((value, index) => Math.abs(pixel[index] - value) < 16);
			},
			{ selector, rgb }
		);
	};
	await waitForColor([255, 0, 0]);
	await setColor([0, 0.25, 1, 0.5]);
	await waitForColor([0, 99, 188]);
	assert.deepEqual(errors, []);
	await page.close();
	const deadline = performance.now() + 12000;
	while (!host.log().includes('[mapgyver-browser-preview] renderer retired')) {
		assert(performance.now() < deadline, 'Closing the preview should release its renderer');
		await delay(100);
	}
	console.log(
		'Solid Color preview preserved aspect and alpha, followed a live edit, and retired on close'
	);
} catch (error) {
	console.error(host.log().slice(-8000));
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
