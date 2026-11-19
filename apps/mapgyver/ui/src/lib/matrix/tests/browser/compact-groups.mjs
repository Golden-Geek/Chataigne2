import assert from 'node:assert/strict';

/** Device scale changes must preserve compact CSS geometry and pointer hit targets.
 * @param {import('playwright-core').Browser} browser
 * @param {string} url
 */
export async function compactGroupScaleChecks(browser, url) {
	const samples = [];
	for (const scale of [1, 1.25, 1.5]) {
		const context = await browser.newContext({
			deviceScaleFactor: scale,
			viewport: { width: 800, height: 700 }
		});
		try {
			const page = await context.newPage();
			await page.goto(url);
			await page.getByRole('button', { name: 'Show Matrix grid', exact: true }).click();
			const grid = page.getByLabel('Grid editor A', { exact: true }).getByRole('grid');
			const group = grid.getByRole('rowheader', { name: 'Layer Group', exact: true });
			const layer = grid.getByRole('rowheader', { name: 'Layer A', exact: true });
			const rem = await page.evaluate(() =>
				parseFloat(getComputedStyle(document.documentElement).fontSize)
			);
			const box = await group.boundingBox(),
				child = await layer.boundingBox();
			assert(box && child);
			assert(Math.abs(box.height / rem - 2.1) < 0.03);
			assert(child.y >= box.y + box.height - 1);
			assert.equal(await group.locator('[data-matrix-row-controls]').count(), 0);
			const slider = layer.getByRole('slider', { name: 'Opacity for Layer A' });
			const opacity = await slider.boundingBox();
			assert(opacity);
			assert(opacity.width <= rem && opacity.height > opacity.width * 2);
			assert(opacity.x + opacity.width <= child.x + child.width);
			assert(child.x + child.width - (opacity.x + opacity.width) < rem);
			assert(opacity.y >= child.y && opacity.y + opacity.height <= child.y + child.height);
			assert.equal(await slider.getAttribute('aria-orientation'), 'vertical');
			await group.getByTitle('Select Layer Group', { exact: true }).click();
			assert.equal(await group.getAttribute('aria-selected'), 'true');
			assert(
				await group.getByRole('button', { name: 'Collapse Layer Group', exact: true }).isVisible()
			);
			samples.push({ scale, group_height_rem: box.height / rem, selection_hit: true });
		} finally {
			await context.close();
		}
	}
	return samples;
}
