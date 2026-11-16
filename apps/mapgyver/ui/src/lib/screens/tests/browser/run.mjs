import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { chromium } from 'playwright-core';
import { gridSelectionChecks } from '../../../matrix/tests/browser/selection.mjs';

const configFile = fileURLToPath(new URL('./vite.config.ts', import.meta.url));
await build({ configFile, logLevel: 'warn' });
const server = await preview({ configFile, logLevel: 'warn' });
let browser;
try {
	const address = server.httpServer.address();
	assert(address && typeof address === 'object');
	const executablePath = process.env.GOLDEN_PREVIEW_BROWSER_PATH;
	browser = await chromium.launch({
		executablePath,
		channel: executablePath ? undefined : 'msedge',
		headless: true
	});
	const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
	/** @type {string[]} */
	const errors = [];
	page.on('pageerror', (error) => {
		errors.push(error.message);
		console.error('Fixture page error:', error.message);
	});
	await page.goto(`http://127.0.0.1:${address.port}`);
	const view = page.getByLabel('Warp fixture', { exact: true });
	const owner = page.getByLabel('Inspected owner');
	/** @param {string} label */
	const polygon = (label) => page.getByRole('button', { name: label, exact: true });
	const first = polygon('Select A');
	const second = polygon('Select B (invalid warp shape)');
	/** @param {import('playwright-core').Locator} element */
	const point = async (element) => {
		const box = await element.boundingBox();
		assert(box);
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const a = await point(first),
		b = await point(second);
	const idleStroke = await first.evaluate((element) => getComputedStyle(element).stroke);
	await page.mouse.move(a.x, a.y);
	assert.equal(await first.getAttribute('data-viewport-hovered'), 'true');
	assert.notEqual(await first.evaluate((element) => getComputedStyle(element).stroke), idleStroke);
	assert.equal(await owner.textContent(), 'screen', 'hover cannot select');
	assert.equal(await page.getByRole('group', { name: /Warp handles for/ }).count(), 0);
	/** @type {import('golden_video_ui').OverlayPrimitive[]} */
	const overlayValues = JSON.parse(
		(await page.getByLabel('Native overlays').textContent()) ?? '[]'
	);
	const hoverOverlay = overlayValues.find((entry) => entry.hit === 'A');
	assert(hoverOverlay);
	assert.equal(hoverOverlay.fill[3], 0, 'native hover cannot fill');
	assert.deepEqual(hoverOverlay.stroke, [192 / 255, 216 / 255, 1, 1]);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'A', 'pointer down selects before release');
	assert.equal(
		await page.getByLabel('Geometry starts').textContent(),
		'0',
		'selection creates no edit'
	);
	assert.equal(await page.getByRole('group', { name: 'Warp handles for A' }).count(), 1);
	assert.equal(await page.getByRole('group', { name: 'Warp handles for B' }).count(), 0);
	await page.mouse.up();
	await page.mouse.move(b.x, b.y);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'B',
		'unavailable source/invalid geometry remains selectable'
	);
	await page.mouse.up();
	const zone = await view.locator('.active-zone').boundingBox();
	assert(zone);
	await page.mouse.move(zone.x + zone.width / 2, zone.y + zone.height / 2);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'screen', 'background inspects owner before release');
	assert.equal(await page.getByRole('group', { name: /Warp handles for/ }).count(), 0);
	await page.mouse.up();
	await first.focus();
	await page.keyboard.press('Enter');
	assert.equal(await owner.textContent(), 'A', 'keyboard selection preserved');
	await page.keyboard.press('Escape');
	assert.equal(await owner.textContent(), 'screen', 'idle Escape clears child selection');
	await first.focus();
	await page.keyboard.press('Enter');
	await view.focus();
	await page.keyboard.down('Space');
	await page.mouse.move(b.x, b.y);
	await page.mouse.down();
	await page.mouse.move(b.x + 12, b.y + 7);
	await page.mouse.up();
	await page.keyboard.up('Space');
	assert.equal(await owner.textContent(), 'A', 'pan does not select or clear');
	assert.equal(await page.getByLabel('Geometry starts').textContent(), '0');
	await page.getByRole('button', { name: 'Toggle warp metadata busy', exact: true }).click();
	const busySecond = await point(second);
	await page.mouse.move(busySecond.x, busySecond.y);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'B',
		'metadata reads cannot block primary-down selection'
	);
	await page.mouse.up();
	const busyCorner = page.getByRole('button', { name: 'Warp corner 1', exact: true });
	await busyCorner.focus();
	await page.keyboard.press('Enter');
	assert.equal(await owner.textContent(), 'B', 'busy corner remains keyboard selectable');
	assert.equal(await busyCorner.getAttribute('aria-pressed'), 'true');
	const busyAt = await point(busyCorner);
	await page.mouse.move(busyAt.x, busyAt.y);
	await page.mouse.down();
	await page.mouse.move(busyAt.x + 24, busyAt.y + 12);
	await page.mouse.up();
	assert.equal(
		await page.getByLabel('Geometry starts').textContent(),
		'0',
		'busy geometry is refused'
	);
	await page.getByRole('button', { name: 'Toggle warp input availability', exact: true }).click();
	await first.focus();
	await page.keyboard.press('Enter');
	const unavailableFirst = await point(first);
	await page.mouse.move(unavailableFirst.x, unavailableFirst.y);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'B', 'unavailable input cannot select');
	await page.mouse.up();
	await page.getByRole('button', { name: 'Toggle warp input availability', exact: true }).click();
	await page.getByRole('button', { name: 'Toggle warp metadata busy', exact: true }).click();
	await page.getByRole('button', { name: 'Show Composition', exact: true }).click();
	const composition = page.getByLabel('Composition view', { exact: true });
	assert.equal(await composition.getByRole('button', { name: /^Move / }).count(), 0);
	const move = page.getByRole('button', { name: 'Select Item A', exact: true });
	const item = await point(move);
	await page.mouse.move(item.x, item.y);
	assert.equal(await move.getAttribute('data-viewport-hovered'), 'true');
	assert.equal(await owner.textContent(), 'composition');
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'Item A',
		'unavailable-source hit selects even when edit admission is denied'
	);
	assert.equal(await composition.locator('[data-instance-handle="Translate"]').count(), 1);
	await page.mouse.up();
	const compositionZone = await composition.locator('.active-zone').boundingBox();
	assert(compositionZone);
	await page.mouse.move(
		compositionZone.x + compositionZone.width / 2,
		compositionZone.y + compositionZone.height * 0.8
	);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'composition',
		'Composition background resolves its owner before release'
	);
	assert.equal(await composition.locator('[data-instance-handle="Translate"]').count(), 0);
	await page.mouse.up();
	await move.click();
	assert.equal(await owner.textContent(), 'Item A');
	await page.keyboard.press('Escape');
	assert.equal(await owner.textContent(), 'composition');
	assert.equal(await composition.locator('[data-instance-handle="Translate"]').count(), 0);
	await move.focus();
	await page.keyboard.press('Enter');
	assert.equal(await owner.textContent(), 'Item A');
	await page.keyboard.press('Escape');
	assert.equal(
		await owner.textContent(),
		'composition',
		'keyboard target replacement retains viewport focus'
	);
	await page.getByRole('button', { name: 'Toggle source bounds', exact: true }).click();
	assert.equal(
		await composition.locator('.viewport-target').count(),
		0,
		'known bounds need no pivot selection marker'
	);
	assert.equal(await composition.getByRole('button', { name: /^Move / }).count(), 0);
	const known = composition.getByRole('button', { name: 'Select Item B', exact: true });
	await known.hover();
	assert.equal(await known.getAttribute('data-viewport-hovered'), 'true');
	assert.equal(await owner.textContent(), 'composition');
	assert.equal(await composition.locator('[data-instance-handle]').count(), 0);
	await known.click();
	assert.equal(await composition.locator('[data-instance-handle="Translate"]').count(), 1);
	assert.equal(await composition.locator('[data-instance-handle="Resize"]').count(), 8);
	assert.equal(await composition.getByRole('button', { name: /^Move / }).count(), 1);
	await page.keyboard.press('Escape');
	assert.equal(await composition.locator('[data-instance-handle]').count(), 0);
	for (const kind of ['Layer', 'Cell']) {
		await page.getByRole('button', { name: `Show Matrix ${kind}`, exact: true }).click();
		const matrix = page.getByLabel('Matrix fixture', { exact: true });
		const handles = matrix.locator(
			kind === 'Layer' ? '[data-matrix-layer-resize]' : '[data-matrix-cell-resize]'
		);
		assert.equal(await handles.count(), 0, 'retained geometry cannot invent a selection');
		const rectangle = matrix.getByRole('button', { name: 'Select Rect B', exact: true });
		const at = await point(rectangle);
		await page.mouse.move(at.x, at.y);
		assert.equal(await rectangle.getAttribute('data-viewport-hovered'), 'true');
		assert.equal(await handles.count(), 0, 'hover cannot introduce resize handles');
		assert.equal(await owner.textContent(), kind === 'Layer' ? 'matrix' : 'layer-owner');
		await page.mouse.down();
		assert.equal(
			await owner.textContent(),
			'Rect B',
			'Matrix selection precedes release and refused edit'
		);
		assert.equal(await handles.count(), 8);
		await page.mouse.up();
		const area = await matrix.locator('.active-zone').boundingBox();
		assert(area);
		await page.mouse.move(area.x + area.width / 2, area.y + area.height * 0.8);
		await page.mouse.down();
		assert.equal(await owner.textContent(), kind === 'Layer' ? 'matrix' : 'layer-owner');
		assert.equal(
			await handles.count(),
			0,
			'cached selected definition cannot restore cleared handles'
		);
		await page.mouse.up();
		await rectangle.click();
		await page.keyboard.press('Escape');
		assert.equal(await owner.textContent(), kind === 'Layer' ? 'matrix' : 'layer-owner');
		assert.equal(await handles.count(), 0);
		const unavailable = matrix.getByRole('button', { name: 'Select Missing bounds', exact: true });
		assert.equal(await matrix.getByRole('button', { name: /^Move / }).count(), 0);
		await unavailable.hover();
		assert.equal(await unavailable.getAttribute('data-viewport-hovered'), 'true');
		assert.equal(await handles.count(), 0);
		await unavailable.focus();
		await page.keyboard.press('Enter');
		assert.equal(
			await owner.textContent(),
			'Missing bounds',
			'unavailable geometry remains keyboard selectable'
		);
		assert.equal(
			await matrix.getByRole('button', { name: 'Move Missing bounds', exact: true }).count(),
			1
		);
		assert.equal(await handles.count(), 0, 'unknown size cannot invent resize controls');
		await page.keyboard.press('Escape');
		await unavailable.hover();
		await page.mouse.down();
		assert.equal(
			await owner.textContent(),
			'Missing bounds',
			'missing bounds select before release'
		);
		await page.mouse.up();
		await page.keyboard.press('Escape');
		assert.equal(await matrix.getByRole('button', { name: /^Move / }).count(), 0);
	}
	await page.getByRole('button', { name: 'Show Surface Source', exact: true }).click();
	const source = page.getByLabel('Surface source view', { exact: true });
	const crop = source.getByRole('button', { name: 'Select source crop', exact: true });
	const corners = source.getByRole('button', { name: /^Crop corner/ });
	assert.equal(await corners.count(), 0, 'Surface owner has no selected crop controls');
	const at = await point(crop);
	await page.mouse.move(at.x, at.y);
	assert.equal(await crop.getAttribute('data-viewport-hovered'), 'true');
	assert.equal(await corners.count(), 0, 'crop hover cannot introduce corners');
	assert.equal(await owner.textContent(), 'A');
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'crop-origin',
		'crop selects its typed Parameter before release/refused edit'
	);
	assert.equal(await corners.count(), 4);
	await page.mouse.move(at.x + 8, at.y + 3);
	assert.equal(await owner.textContent(), 'crop-origin', 'refused drag preserves selection');
	await page.mouse.up();
	assert.equal(await page.getByLabel('Geometry starts').textContent(), '0');
	const area = await source.locator('.active-zone').boundingBox();
	assert(area);
	await page.mouse.move(area.x + area.width * 0.1, area.y + area.height * 0.85);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'A',
		'crop background inspects Surface owner before release'
	);
	assert.equal(await corners.count(), 0);
	await page.mouse.up();
	await crop.click();
	await page.keyboard.press('Escape');
	assert.equal(await owner.textContent(), 'A');
	assert.equal(await corners.count(), 0);
	await crop.focus();
	await page.keyboard.press('Enter');
	assert.equal(await owner.textContent(), 'crop-origin');
	assert.equal(await corners.count(), 4);
	const corner = await point(corners.first());
	await page.mouse.move(corner.x, corner.y);
	await page.mouse.down();
	assert.equal(await owner.textContent(), 'crop-size');
	await page.mouse.up();
	await crop.focus();
	await page.keyboard.press('Enter');
	await page.getByRole('button', { name: 'Toggle crop metadata busy', exact: true }).click();
	await corners.first().focus();
	await page.keyboard.press('Enter');
	assert.equal(
		await owner.textContent(),
		'crop-size',
		'metadata refresh cannot block keyboard selection'
	);
	await crop.focus();
	await page.keyboard.press('Enter');
	await page.mouse.move(corner.x, corner.y);
	await page.mouse.down();
	assert.equal(
		await owner.textContent(),
		'crop-size',
		'metadata refresh cannot block primary-down selection'
	);
	await page.mouse.up();
	assert.equal(await page.getByLabel('Geometry starts').textContent(), '0');
	await page.getByRole('button', { name: 'Toggle crop metadata busy', exact: true }).click();
	await page.getByRole('button', { name: 'Toggle crop editing', exact: true }).click();
	await crop.focus();
	await page.keyboard.press('Enter');
	await corners.first().focus();
	await page.keyboard.press('Enter');
	assert.equal(
		await owner.textContent(),
		'crop-size',
		'keyboard activation selects the size Parameter without an edit'
	);
	await page.mouse.move(at.x, at.y);
	await page.mouse.down();
	await page.mouse.move(at.x + 12, at.y + 6);
	await page.keyboard.press('Escape');
	assert.equal(
		await page.getByLabel('Crop stops').textContent(),
		'1',
		'first Escape cancels the captured edit'
	);
	assert.equal(await owner.textContent(), 'crop-origin', 'cancel preserves tool selection');
	assert.equal(await corners.count(), 4);
	await page.mouse.up();
	assert.equal(
		await page.getByLabel('Crop finishes').textContent(),
		'0',
		'cancel cannot commit on release'
	);
	await page.keyboard.press('Escape');
	assert.equal(await owner.textContent(), 'A', 'idle Escape then deselects the crop');
	assert.equal(await corners.count(), 0);
	await page.mouse.move(at.x, at.y);
	await page.mouse.down();
	await page.mouse.move(at.x + 18, at.y + 9);
	await page.mouse.up();
	assert.equal(await page.getByLabel('Crop finishes').textContent(), '1');
	assert.equal(await page.getByLabel('Crop stops').textContent(), '1');
	assert(Number(await page.getByLabel('Crop moves').textContent()) >= 2);
	await gridSelectionChecks(page);
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'DOM Screen/Source/Composition/Matrix spatial hover/selected controls and Matrix grid primary-down/owner clearing, rename/launch/control/drag isolation, independent grids and action disposal; native overlay declarations; no native pixel or full M14 acceptance'
		})
	);
} finally {
	await browser?.close();
	await new Promise((resolve) => server.httpServer.close(resolve));
}
