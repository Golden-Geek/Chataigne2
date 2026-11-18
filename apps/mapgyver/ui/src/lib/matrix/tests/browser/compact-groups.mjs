import assert from 'node:assert/strict';

/** Device scale changes must preserve compact CSS geometry and pointer hit targets. */
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
