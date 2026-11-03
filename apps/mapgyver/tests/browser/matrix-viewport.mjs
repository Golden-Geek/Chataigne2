import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixViewportUi({ base, edit }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Viewport Matrix'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	await matrix({ kind: 'AddLayers', parent: null, count: 61 });
	await matrix({ kind: 'AddColumns', count: 61 });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Matrix viewport lifetime');
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('region', { name: 'Media Manager', exact: true })
			.getByRole('button', { name: 'Viewport Matrix', exact: true })
			.click();
		const tab = page.getByRole('tab').filter({ hasText: /^Matrix$/ });
		await tab.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await grid.getByTitle('Select Layer 1', { exact: true }).click();
		await grid.getByTitle('Select Column 1', { exact: true }).click();
		await parameter('Override transition').locator('input[type=checkbox]').check();
		const duration = parameter('Transition duration (ms)').locator('input.number-field');
		await duration.fill('1234');
		await duration.press('Enter');
		const beforeScroll = await wait(
			inspect,
			(state) => state.columns[0].definition.transition?.duration_ms === 1234
		);
		await grid.focus();
		for (let index = 0; index < 4; index++) await page.keyboard.press('PageDown');
		for (let index = 0; index < 10; index++) await page.keyboard.press('ArrowRight');
		await wait(
			() => grid.evaluate((element) => ({ top: element.scrollTop, left: element.scrollLeft })),
			(scroll) => scroll.top > 0 && scroll.left > 0
		);
		await wait(
			() => grid.getByTitle('Select Column 1', { exact: true }).count(),
			(count) => count === 0
		);
		assert.equal(await grid.getByTitle('Select Layer 1', { exact: true }).count(), 0);
		assert((await grid.getByRole('gridcell').count()) < 2048);
		assert.deepEqual(await inspect(), beforeScroll);
		await duration.fill('4321');
		await duration.press('Enter');
		const edited = await wait(
			inspect,
			(state) => state.columns[0].definition.transition?.duration_ms === 4321
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeScroll));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, edited));
		assert.equal(
			await grid.getByTitle('Select Column 1', { exact: true }).count(),
			0,
			'Inspector edit does not scroll selection into view'
		);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		await tab.click();
		await duration.waitFor();
		assert.equal(await duration.inputValue(), '4321.000');
		for (const viewport of [
			{ width: 2560, height: 1200 },
			{ width: 1440, height: 1000 },
			{ width: 1300, height: 900 }
		]) {
			await page.setViewportSize(viewport);
			await grid.waitFor();
			assert((await grid.getByRole('gridcell').count()) <= 2048);
			assert.deepEqual(await inspect(), edited);
		}
		await page.evaluate(() => (document.documentElement.style.fontSize = '125%'));
		await duration.scrollIntoViewIfNeeded();
		const label = await parameter('Transition duration (ms)')
			.locator('.parameter-label')
			.boundingBox();
		const input = await duration.boundingBox();
		const outer = await page.locator('.inspector').boundingBox();
		assert(
			label.y + label.height <= input.y || label.x + label.width <= input.x,
			'narrow Inspector label never overlaps its input'
		);
		assert(
			input.x >= outer.x && input.x + input.width <= outer.x + outer.width,
			'narrow Inspector input stays inside its panel'
		);
		await page.screenshot({ path: 'target/m07-viewport-inspector.png' });
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'64x64 bounded keyboard scroll, offscreen Column Inspector/history without scrolling selection, tab lifetime, resize and narrow 125% Inspector geometry; no launch',
				initialRows: initial.items.length,
				initialColumns: initial.columns.length,
				browser: browser.version()
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-viewport-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-4000)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
