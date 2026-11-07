import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixViewportUi({ base, edit, page: nativePage = null }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Viewport Matrix'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	let initial;
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
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
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Viewport Matrix', exact: true })
			.click();
		const tab = page.getByRole('tab').filter({ hasText: /^Matrix(?::|$)/ });
		await tab.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		// Fractional dock bounds used to let the measured sticky overlay change its own
		// scrollbar extent, repeatedly invalidating every native view in the session.
		for (const width of [701.4, 746.2, 446.2]) {
			const sizes = await grid.evaluate(async (element, width) => {
				const original = element.style.cssText;
				element.style.width = `${width}px`;
				element.style.height = '635.2px';
				element.style.flex = 'none';
				const sizes = [];
				try {
					for (let frame = 0; frame < 90; frame++) {
						await new Promise(requestAnimationFrame);
						if (frame >= 30) sizes.push([element.clientWidth, element.clientHeight]);
					}
				} finally {
					element.style.cssText = original;
				}
				return sizes;
			}, width);
			assert.equal(new Set(sizes.map(JSON.stringify)).size, 1, 'idle Matrix scroll bounds settle');
		}
		await matrix({ kind: 'AddLayers', parent: null, count: 61 });
		await matrix({ kind: 'AddColumns', count: 61 });
		initial = await inspect();
		await grid
			.locator(
				`[role="rowheader"][data-row-id="${initial.items.find((item) => item.label === 'Layer 1').id.id}"]`
			)
			.click();
		await grid.locator(`[role="columnheader"][data-column-id="${initial.columns[0].id}"]`).click();
		await parameter('Transition Time')
			.getByRole('switch', { name: 'Toggle node enabled', exact: true })
			.click();
		const duration = parameter('Transition Time').locator('input.number-field');
		await duration.fill('00:00:01.234');
		const draftInput = await duration.elementHandle();
		await matrix({
			kind: 'RenameColumn',
			column: initial.columns[1].id,
			label: 'Background renamed'
		});
		await grid
			.locator(`[role="columnheader"][data-column-id="${initial.columns[1].id}"]`)
			.filter({ hasText: 'Background renamed' })
			.waitFor();
		assert.equal(
			await duration.inputValue(),
			'00:00:01.234',
			'unrelated backend edit preserves the draft'
		);
		assert(
			await draftInput.evaluate(
				(element) => element.isConnected && document.activeElement === element
			),
			'background refresh preserves the focused field identity'
		);
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
			() =>
				grid.locator(`[role="columnheader"][data-column-id="${initial.columns[0].id}"]`).count(),
			(count) => count === 0
		);
		assert.equal(
			await grid
				.locator(
					`[role="rowheader"][data-row-id="${initial.items.find((item) => item.label === 'Layer 1').id.id}"]`
				)
				.count(),
			0
		);
		assert((await grid.getByRole('gridcell').count()) < 2048);
		assert.deepEqual(await inspect(), beforeScroll);
		const retainedScroll = await grid.evaluate((element) => ({
			top: element.scrollTop,
			left: element.scrollLeft
		}));
		await duration.fill('00:00:04.321');
		await duration.press('Enter');
		const edited = await wait(
			inspect,
			(state) => state.columns[0].definition.transition?.duration_ms === 4321
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeScroll));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, edited));
		assert.deepEqual(
			await grid.evaluate((element) => ({ top: element.scrollTop, left: element.scrollLeft })),
			retainedScroll,
			'Inspector/history refresh preserves the exact Matrix scroll position'
		);
		assert.equal(
			await grid
				.locator(`[role="columnheader"][data-column-id="${initial.columns[0].id}"]`)
				.count(),
			0,
			'Inspector edit does not scroll selection into view'
		);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.click();
		await tab.click();
		await duration.waitFor();
		assert.equal(await duration.inputValue(), '00:00:04.321');
		for (const viewport of nativePage
			? []
			: [
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
		const label = await parameter('Transition Time').locator('.parameter-label').boundingBox();
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
		const toolbar = page.locator('[data-panel-id="matrix"] .canvas-toolbar');
		assert.equal(await toolbar.count(), 0, 'Matrix is a scrolling table without canvas navigation');
		await grid.evaluate((element) => {
			element.scrollTo({ top: 0, left: 0 });
			element.dispatchEvent(new Event('scroll'));
		});
		await grid
			.locator(`[role="columnheader"][data-column-id="${initial.columns[0].id}"]`)
			.waitFor();
		await grid
			.locator(
				`[role="rowheader"][data-row-id="${initial.items.find((item) => item.label === 'Layer 1').id.id}"]`
			)
			.waitFor();
		assert.deepEqual(await inspect(), edited, 'table scrolling does not author domain state');
		assert.equal(await page.locator('[data-panel-id="matrix"] > .editor-toolbar').count(), 0);
		assert((await grid.getByRole('gridcell').count()) <= 2048);
		await page.evaluate(() => (document.documentElement.style.fontSize = ''));
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
				backgroundDraftPreserved: true,
				exactScrollPreserved: true,
				native: !!nativePage,
				browser: (browser ?? page.context().browser()).version()
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
		if (nativePage)
			await page.evaluate(() => (document.documentElement.style.fontSize = '')).catch(() => {});
		await browser?.close();
	}
}
