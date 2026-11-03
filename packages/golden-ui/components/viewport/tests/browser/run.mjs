import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { chromium } from 'playwright-core';

const configFile = fileURLToPath(new URL('./vite.config.ts', import.meta.url));
await build({ configFile, logLevel: 'warn' });
const server = await preview({ configFile, logLevel: 'warn' });
let browser;
const near = (actual, expected, message) =>
	assert(Math.abs(actual - expected) < 1.1, `${message}: ${actual} versus ${expected}`);

try {
	const address = server.httpServer.address();
	assert(address && typeof address === 'object');
	const executablePath = process.env.GOLDEN_PREVIEW_BROWSER_PATH;
	browser = await chromium.launch({
		executablePath,
		channel: executablePath ? undefined : process.platform === 'win32' ? 'msedge' : undefined,
		headless: true
	});
	const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
	page.setDefaultTimeout(3000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`http://127.0.0.1:${address.port}`);
	const view = page.getByLabel('Test viewport', { exact: true });
	const active = view.locator('.active-zone');
	const stage = view.locator('.stage');
	const handle = page.getByRole('button', { name: 'Test handle' });
	const selection = page.getByLabel('Test selection');
	const zoom = view.getByLabel('View zoom');
	const atZoom = (value) =>
		page.waitForFunction(
			(value) =>
				Math.abs(
					Number(document.querySelector('output[aria-label="View zoom"]').dataset.zoom) - value
				) < 1e-9,
			value
		);
	const box = async (locator) => {
		const value = await locator.boundingBox();
		assert(value);
		return value;
	};
	const roundHandle = async (original) => {
		const bounds = await box(handle);
		near(bounds.width, bounds.height, 'round handle dimensions');
		if (original) near(bounds.width, original.width, 'constant screen-space handle size');
		assert.equal(await handle.evaluate((element) => getComputedStyle(element).borderRadius), '50%');
		return bounds;
	};
	await page.waitForFunction(
		() => document.querySelector('.active-zone').getBoundingClientRect().width > 100
	);
	const initial = await box(active);
	near(initial.width / initial.height, 2, 'non-square active aspect');
	assert.equal(
		await active.evaluate((element) => getComputedStyle(element).backgroundColor),
		'rgb(0, 0, 0)'
	);
	const originalHandle = await roundHandle();
	const anchor = { x: initial.x + initial.width * 0.4, y: initial.y + initial.height * 0.45 };
	await page.evaluate(() => {
		window.__zoomSamples = [];
		const until = performance.now() + 500;
		function sample() {
			const rect = document.querySelector('.active-zone').getBoundingClientRect();
			window.__zoomSamples.push({
				zoom: Number(document.querySelector('output[aria-label="View zoom"]').dataset.zoom),
				x: rect.x + rect.width * 0.4,
				y: rect.y + rect.height * 0.45
			});
			if (performance.now() < until) requestAnimationFrame(sample);
		}
		requestAnimationFrame(sample);
	});
	await page.mouse.move(anchor.x, anchor.y);
	await page.mouse.wheel(0, -100);
	await zoom.getByText('115%').waitFor();
	await atZoom(Math.exp(0.14));
	const samples = await page.evaluate(() => window.__zoomSamples);
	assert(
		new Set(
			samples
				.filter((sample) => sample.zoom > 1 && sample.zoom < Math.exp(0.14))
				.map((sample) => sample.zoom)
		).size >= 2,
		'wheel produces intermediate animation frames'
	);
	for (const sample of samples) {
		near(sample.x, anchor.x, 'animated anchor X');
		near(sample.y, anchor.y, 'animated anchor Y');
	}
	const zoomed = await box(active);
	near(zoomed.x + zoomed.width * 0.4, anchor.x, 'wheel anchor X');
	near(zoomed.y + zoomed.height * 0.45, anchor.y, 'wheel anchor Y');
	await roundHandle(originalHandle);
	await page.getByRole('button', { name: 'Update geometry snapshot', exact: true }).click();
	assert.equal(
		await zoom.textContent(),
		'115%',
		'A same-document geometry snapshot preserves zoom'
	);
	near((await box(active)).x, zoomed.x, 'A same-document geometry snapshot preserves pan');
	await view.focus();
	await page.keyboard.press('Home');
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	near((await box(active)).x, initial.x, 'Home resets pan');
	const handlePosition = await box(handle);
	await page.keyboard.down('Space');
	await page.mouse.move(
		handlePosition.x + handlePosition.width / 2,
		handlePosition.y + handlePosition.height / 2
	);
	await page.mouse.down();
	await page.mouse.move(
		handlePosition.x + handlePosition.width / 2 + 70,
		handlePosition.y + handlePosition.height / 2 + 35
	);
	await page.mouse.up();
	await page.keyboard.up('Space');
	const panned = await box(active);
	near(panned.x, initial.x + 70, 'Space pan X');
	near(panned.y, initial.y + 35, 'Space pan Y');
	assert.equal(await page.getByLabel('Domain edit starts').textContent(), '0');
	await page.mouse.move(panned.x + panned.width / 2, panned.y + panned.height / 2);
	await page.mouse.down({ button: 'middle' });
	await page.mouse.move(panned.x + panned.width / 2 - 45, panned.y + panned.height / 2 + 20);
	await page.mouse.up({ button: 'middle' });
	near((await box(active)).x, panned.x - 45, 'middle pan X');
	near((await box(active)).y, panned.y + 20, 'middle pan Y');
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await zoom.getByText('120%').waitFor();
	await atZoom(1.2);
	const beforeEdit = await box(active);
	await handle.click();
	assert.equal(await page.getByLabel('Domain edit starts').textContent(), '1');
	near((await box(active)).x, beforeEdit.x, 'Handle edit receipts preserve pan X');
	near((await box(active)).y, beforeEdit.y, 'Handle edit receipts preserve pan Y');
	near((await box(active)).width, beforeEdit.width, 'Handle edit receipts preserve zoom');
	await page.getByRole('button', { name: 'Change selection', exact: true }).click();
	near((await box(active)).x, beforeEdit.x, 'Changing selection preserves pan');
	near((await box(active)).width, beforeEdit.width, 'Changing selection preserves zoom');
	await view.focus();
	await page.keyboard.press('f');
	await zoom.getByText('400%').waitFor();
	await atZoom(4);
	const framed = await box(selection);
	const stageBounds = await box(stage);
	near(framed.x + framed.width / 2, stageBounds.x + stageBounds.width / 2, 'F centers selection X');
	near(
		framed.y + framed.height / 2,
		stageBounds.y + stageBounds.height / 2,
		'F centers selection Y'
	);
	await roundHandle(originalHandle);
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await zoom.getByText('120%').waitFor();
	await atZoom(1.2);
	await view.getByRole('button', { name: 'Zoom out', exact: true }).click();
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	await page.keyboard.press('f');
	await zoom.getByText('400%').waitFor();
	await atZoom(4);
	await view.getByRole('button', { name: 'Frame selection', exact: true }).click();
	await zoom.getByText('400%').waitFor();
	await atZoom(4);
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	assert.equal(await view.getByRole('button', { name: 'Pan view', exact: true }).count(), 0);
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await zoom.getByText('120%').waitFor();
	await atZoom(1.2);
	await page.keyboard.press('+');
	await zoom.getByText('144%').waitFor();
	await atZoom(1.44);
	await page.keyboard.press('-');
	await zoom.getByText('120%').waitFor();
	await atZoom(1.2);
	await page.keyboard.press('h');
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await page.keyboard.press('Home');
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	const zoomIn = view.getByRole('button', { name: 'Zoom in', exact: true });
	assert.equal(await zoomIn.getAttribute('title'), 'Zoom in (+)');
	await zoomIn.focus();
	await page.keyboard.press('Space');
	await zoom.getByText('120%').waitFor();
	await atZoom(1.2);
	assert.equal(await view.evaluate((element) => element.classList.contains('panning')), false);
	await page.keyboard.press('h');
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await page.getByRole('button', { name: 'Change document', exact: true }).click();
	await zoom.getByText('100%').waitFor();
	await atZoom(1);
	await page.waitForTimeout(200);
	assert.equal(
		await zoom.getAttribute('data-zoom'),
		'1',
		'document replacement cancels pending animation'
	);
	await page.getByRole('button', { name: 'Resize view', exact: true }).click();
	await page.waitForFunction(
		() => document.querySelector('main').getBoundingClientRect().width === 512
	);
	await roundHandle(originalHandle);
	const resized = await box(active);
	near(resized.width / resized.height, 2, 'resized active aspect');
	await page.getByRole('button', { name: 'Toggle native', exact: true }).click();
	assert.equal(
		await active.evaluate((element) => getComputedStyle(element).backgroundColor),
		'rgba(0, 0, 0, 0)'
	);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
	assert.equal(await zoom.getAttribute('data-zoom'), '1.2');
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	const released = await box(active);
	await page.mouse.move(released.x + released.width / 2, released.y + released.height / 2);
	await page.mouse.down();
	await page.mouse.move(5, 5);
	await page.mouse.up();
	await page.mouse.move(released.x + released.width / 2, released.y + released.height / 2);
	await page.mouse.wheel(0, -100);
	await page.getByRole('button', { name: 'Update geometry snapshot', exact: true }).click();
	await atZoom(Math.exp(0.14));
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			blackActiveArea: true,
			roundConstantHandles: true,
			anchoredWheelZoom: true,
			smoothIntermediateFrames: true,
			animationCancelledOnDocumentReplacement: true,
			reducedMotion: true,
			outsidePointerReleaseAndLiveSnapshot: true,
			spacePan: true,
			middlePan: true,
			home: true,
			frameSelection: true,
			toolbar: true,
			panSuppressesDomainEdits: true,
			nativeTransparency: true,
			cameraPreservedAcrossSnapshotsAndHandleEdits: true,
			cameraPreservedAcrossSelection: true,
			hShortcut: true,
			noStickyPanTool: true,
			shortcutsAfterToolbarClick: true,
			nativeToolbarSpaceActivation: true,
			pageErrors: errors
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
