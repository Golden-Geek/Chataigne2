// Real Surface Source input and history; backend requests below only observe state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const near = (actual, expected) =>
	Object.entries(expected).every(([key, value]) => Math.abs(actual[key] - value) < 0.005);
const corners = (crop) => [
	{ x: crop.x, y: crop.y },
	{ x: crop.x + crop.width, y: crop.y },
	{ x: crop.x + crop.width, y: crop.y + crop.height },
	{ x: crop.x, y: crop.y + crop.height }
];

export async function verifySurfaceSourceUi({ base, post, media, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	try {
		async function screens() {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.screens',
				operation: 'edit',
				payload: { kind: 'InspectManager' }
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value.snapshot;
		}
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Surface Source$/ });
		await tab.click();
		const panel = page.getByRole('region', { name: 'Surface Source', exact: true });
		await panel.getByRole('button', { name: 'Add screen', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const first = snapshot.screens[0].surfaces[0];
		await panel.getByRole('button', { name: 'Add surface', exact: true }).click();
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		const peer = snapshot.screens[0].surfaces[1];
		const navigation = panel.getByRole('region', { name: 'Ordered surfaces', exact: true });
		await navigation.getByRole('button', { name: first.label, exact: true }).click();
		const sourceMedia = media.media.find((value) => value.media_kind === 'SolidColor');
		assert(sourceMedia);
		const source = sourceMedia.channels[0];
		await panel.getByLabel('Find source', { exact: true }).fill(sourceMedia.label);
		await panel
			.getByLabel('Source channel')
			.selectOption({ label: `${sourceMedia.label} / ${source.label}` });
		await panel.getByLabel('Enabled', { exact: true }).uncheck();
		await panel.getByLabel('Opacity', { exact: true }).fill('0.65');
		await panel.getByLabel('Calibration').selectOption('Grid');
		for (const [label, value] of [
			['Crop X', 0.1],
			['Crop Y', 0.1],
			['Crop width', 0.6],
			['Crop height', 0.5],
			['Top left X', 0.05],
			['Top left Y', 0.04],
			['Top right X', 0.95],
			['Top right Y', 0.02],
			['Bottom right X', 0.9],
			['Bottom right Y', 0.96],
			['Bottom left X', 0.02],
			['Bottom left Y', 0.9]
		]) {
			await panel.getByLabel(label, { exact: true }).fill(String(value));
		}
		assert.deepEqual(await screens(), snapshot, 'all form fields remain a draft before Apply');
		const apply = () =>
			panel.getByRole('button', { name: 'Apply surface settings', exact: true }).click();
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		const definition = async () => (await screens()).screens[0].surfaces[0].definition;
		const crop = async () => (await definition()).crop;
		const ready = () =>
			wait(
				() => panel.getByRole('button', { name: 'Add surface', exact: true }).isEnabled(),
				Boolean
			);
		stage = 'complete form apply and history';
		await apply();
		await wait(definition, (value) => value.calibration === 'Grid');
		const configured = await definition();
		assert.deepEqual(configured.source, source.reference);
		assert.equal(configured.enabled, false);
		assert(Math.abs(configured.opacity - 0.65) < 0.000001);
		assert(near(configured.crop, { x: 0.1, y: 0.1, width: 0.6, height: 0.5 }));
		for (const [index, point] of [
			{ x: 0.05, y: 0.04 },
			{ x: 0.95, y: 0.02 },
			{ x: 0.9, y: 0.96 },
			{ x: 0.02, y: 0.9 }
		].entries())
			assert(near(configured.quad.corners[index], point));
		await undo();
		await wait(definition, (value) => value.source === null);
		assert.deepEqual(await screens(), snapshot, 'one Apply is one whole-definition Undo');
		await redo();
		await wait(definition, (value) => value.calibration === 'Grid');
		assert.deepEqual(await definition(), configured);
		for (const calibration of ['ColorBars', 'Off']) {
			await panel.getByLabel('Calibration').selectOption(calibration);
			await apply();
			await wait(definition, (value) => value.calibration === calibration);
			assert.deepEqual(
				{ ...(await definition()), calibration: 'Grid' },
				configured,
				'calibration cannot replace source/crop/mapping'
			);
		}
		const svg = panel.getByRole('group', { name: 'Source crop controls', exact: true });
		async function begin(index) {
			await ready();
			await svg.scrollIntoViewIfNeeded();
			const box = await svg.boundingBox();
			assert(box && box.width > 20 && box.height > 20);
			const value = await crop();
			const point =
				index === null
					? { x: value.x + value.width / 2, y: value.y + value.height / 2 }
					: corners(value)[index];
			await page.mouse.move(box.x + point.x * box.width, box.y + point.y * box.height);
			await page.mouse.down();
			return await svg.boundingBox();
		}
		async function move(box, point, steps = 1) {
			assert(box);
			await page.mouse.move(box.x + point.x * box.width, box.y + point.y * box.height, { steps });
		}
		async function roundtrip(before, after, expected) {
			assert.deepEqual(after.screens[0].surfaces[1], peer);
			assert.deepEqual(
				{
					...after.screens[0].surfaces[0].definition,
					crop: before.screens[0].surfaces[0].definition.crop
				},
				before.screens[0].surfaces[0].definition
			);
			await undo();
			await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
			assert.deepEqual(await screens(), before);
			await redo();
			await wait(crop, (value) => near(value, expected));
			assert.deepEqual(await screens(), after);
		}
		stage = 'all four crop handles';
		for (const [index, point, expected] of [
			[0, { x: 0.15, y: 0.15 }, { x: 0.15, y: 0.15, width: 0.55, height: 0.45 }],
			[1, { x: 0.65, y: 0.18 }, { x: 0.15, y: 0.18, width: 0.5, height: 0.42 }],
			[2, { x: 0.6, y: 0.55 }, { x: 0.15, y: 0.18, width: 0.45, height: 0.37 }],
			[3, { x: 0.2, y: 0.5 }, { x: 0.2, y: 0.18, width: 0.4, height: 0.32 }]
		]) {
			const before = await screens();
			const box = await begin(index);
			await move(box, point, 12);
			await page.mouse.up();
			await ready();
			await wait(crop, (value) => near(value, expected));
			await roundtrip(before, await screens(), expected);
		}
		stage = 'origin drag';
		let before = await screens();
		let box = await begin(null);
		await move(box, { x: 0.48, y: 0.4 }, 16);
		await page.mouse.up();
		await ready();
		await wait(crop, (value) => near(value, { x: 0.28, y: 0.24 }));
		let after = await screens();
		assert.equal(
			after.screens[0].surfaces[0].definition.crop.width,
			before.screens[0].surfaces[0].definition.crop.width
		);
		assert.equal(
			after.screens[0].surfaces[0].definition.crop.height,
			before.screens[0].surfaces[0].definition.crop.height
		);
		await roundtrip(before, after, after.screens[0].surfaces[0].definition.crop);
		stage = 'invalid pointer sample';
		box = await begin(0);
		await move(box, { x: -0.2, y: -0.2 });
		await page.mouse.up();
		await panel.locator('p.error[role="status"]').waitFor();
		await ready();
		assert.deepEqual(await screens(), after);
		await undo();
		await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
		assert.deepEqual(await screens(), before, 'rejection cannot consume a history step');
		await redo();
		await wait(crop, (value) => near(value, after.screens[0].surfaces[0].definition.crop));
		stage = 'Escape stops accepted origin drag';
		before = await screens();
		box = await begin(null);
		const current = await crop();
		await move(
			box,
			{ x: current.x + current.width / 2 + 0.04, y: current.y + current.height / 2 + 0.02 },
			12
		);
		await wait(crop, (value) => near(value, { x: current.x + 0.04, y: current.y + 0.02 }));
		const accepted = await crop();
		await page.keyboard.press('Escape');
		await move(box, { x: 0.8, y: 0.8 });
		await page.mouse.up();
		await ready();
		assert.deepEqual(await crop(), accepted, 'held-pointer motion after Escape cannot author');
		after = await screens();
		await roundtrip(before, after, accepted);
		stage = 'invalid crop form';
		await panel.getByLabel('Crop X', { exact: true }).fill('0.9');
		await apply();
		await panel.getByRole('alert').waitFor();
		assert.deepEqual(await screens(), after);
		await undo();
		await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
		assert.deepEqual(await screens(), before);
		await redo();
		await wait(crop, (value) => near(value, accepted));
		stage = 'source loss and Undo';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		const manager = page
			.locator('section.manager')
			.filter({ has: page.getByRole('heading', { name: 'Media Manager', exact: true }) });
		const article = manager.locator('article').filter({
			has: page.getByRole('textbox', { name: `Media name: ${sourceMedia.label}`, exact: true })
		});
		await article.getByRole('button', { name: 'Delete media', exact: true }).click();
		await wait(inspectMedia, (value) => !value.media.some((entry) => entry.id === sourceMedia.id));
		await tab.click();
		await wait(
			() =>
				panel
					.getByLabel('Source channel')
					.locator('option')
					.filter({ hasText: 'Missing source' })
					.count(),
			(value) => value === 1
		);
		assert.deepEqual(await screens(), after);
		await undo();
		await wait(inspectMedia, (value) => value.media.some((entry) => entry.id === sourceMedia.id));
		assert.deepEqual(await inspectMedia(), media);
		assert.deepEqual(await screens(), after);
		stage = 'shared selection and panel lifetime';
		await navigation.getByRole('button', { name: peer.label, exact: true }).click();
		await navigation.getByRole('button', { name: first.label, exact: true }).click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		await tab.click();
		await ready();
		assert.deepEqual(await screens(), after);
		assert.deepEqual(errors, []);
		await svg.scrollIntoViewIfNeeded();
		await page.screenshot({ path: 'target/m05-surface-source-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual Surface Source form/calibration/crop/origin/rejection/Escape/source-loss/history input; native preview remains unverified',
				browser: browser.version(),
				surface: first.reference,
				source: source.reference,
				final_definition: await definition(),
				page_errors: errors.length,
				screenshot: 'target/m05-surface-source-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-3500)
			})
		);
		await page.screenshot({ path: 'target/m05-surface-source-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
