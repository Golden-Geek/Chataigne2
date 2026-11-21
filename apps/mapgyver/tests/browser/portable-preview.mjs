import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

assert(process.env.MAPGYVER_TEST_BINARY, 'Select a compiled MapGyver with preview enabled');
const directory = await mkdtemp(join(tmpdir(), 'mapgyver-portable-preview-'));
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

async function pictureFile(page, name, colors) {
	const data = await page.evaluate((colors) => {
		const canvas = document.createElement('canvas');
		canvas.width = 16;
		canvas.height = 32;
		const context = canvas.getContext('2d');
		const image = context.createImageData(canvas.width, canvas.height);
		for (let y = 0; y < canvas.height; y++) {
			const color = colors[y < 8 ? 0 : y >= 24 ? 2 : 1];
			for (let x = 0; x < canvas.width; x++) {
				image.data.set(color, 4 * (y * canvas.width + x));
			}
		}
		context.putImageData(image, 0, 0);
		return canvas.toDataURL('image/png').split(',')[1];
	}, colors);
	const file = join(directory, name);
	await writeFile(file, Buffer.from(data, 'base64'));
	return file;
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
	const preview = async (label) => {
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
	};
	await preview(label);
	await page.getByRole('combobox', { name: 'Browser Preview Quality' }).selectOption('high');
	const selector = '[data-panel-id="media-preview"] canvas[data-preview-state="available"]';
	const waitForPixels = async (...samples) => {
		await page.waitForFunction(
			({ selector, samples }) => {
				const canvas = document.querySelector(selector);
				if (!canvas || canvas.width !== 480 || canvas.height !== 360) return false;
				const context = canvas.getContext('2d');
				return samples.every(([x, y, rgb]) => {
					const pixel = context.getImageData(x, y, 1, 1).data;
					return (
						pixel[3] === 255 && rgb.every((value, index) => Math.abs(pixel[index] - value) < 16)
					);
				});
			},
			{ selector, samples }
		);
	};
	const red = [255, 0, 0];
	const green = [0, 255, 0];
	const black = [0, 0, 0];
	await waitForPixels([240, 180, red]);
	await setColor([0, 0.25, 1, 0.5]);
	await waitForPixels([240, 180, [0, 99, 188]]);
	console.log('Solid Color preview preserved aspect and alpha and followed a live edit');

	const pictureLabel = 'Portable Picture';
	const { reference: picture } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Picture',
		label: pictureLabel
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: picture.media,
		size: { kind: 'Fixed', extent: { width: 320, height: 240 } }
	});
	const setFit = (mode) => edit({ kind: 'ConfigureMediaFitMode', media: picture.media, mode });
	const setPicture = (file) => edit({ kind: 'ConfigurePictureFile', reference: picture, file });
	const shaded = [128, 0, 255, 128];
	await setPicture(
		await pictureFile(page, 'portrait.png', [[...red, 255], shaded, [...green, 255]])
	);
	await setFit('Fit');
	await preview(pictureLabel);
	const shadedOnBlack = [93, 0, 188];
	await waitForPixels([70, 180, black], [240, 30, red], [240, 180, shadedOnBlack]);
	await setFit('Fill');
	await waitForPixels([70, 30, shadedOnBlack], [240, 180, shadedOnBlack]);
	await setFit('Stretch');
	await waitForPixels([70, 30, red], [70, 180, shadedOnBlack]);
	await setPicture(await pictureFile(page, 'relinked.png', Array(3).fill([...green, 255])));
	await waitForPixels([70, 30, green], [240, 180, green]);
	console.log('Picture preview rendered Fit, Fill, Stretch, alpha and a live file relink');
	assert.deepEqual(errors, []);
	const retirements = () => host.log().split('[mapgyver-browser-preview] renderer retired').length;
	const beforeClose = retirements();
	await page.close();
	const deadline = performance.now() + 12000;
	while (retirements() === beforeClose) {
		assert(performance.now() < deadline, 'Closing the preview should release its renderer');
		await delay(100);
	}
	console.log('Portable preview renderer retired on close');
} catch (error) {
	console.error(host.log().slice(-8000));
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
