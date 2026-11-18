import assert from 'node:assert/strict';

/** @param {import('playwright-core').Page} page */
export async function handleLinkChecks(page) {
	await page.getByRole('button', { name: 'Show Screen linking', exact: true }).click();
	const view = page.getByLabel('Warp fixture', { exact: true });
	const handle = view.getByRole('button', { name: 'Warp corner 1', exact: true });
	/** @param {number} offset */
	async function drag(offset) {
		const bounds = await view.locator('svg[aria-label="Surface mapping controls"]').boundingBox();
		const box = await handle.boundingBox();
		assert(bounds && box);
		const rem = await page.evaluate(() =>
			Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
		);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(
			bounds.x + bounds.width * 0.55 + offset * rem,
			bounds.y + bounds.height * 0.2,
			{ steps: 5 }
		);
		await page.mouse.up();
		return JSON.parse(
			(await page.getByLabel('Warp link', { exact: true }).textContent()) ?? 'null'
		);
	}
	for (const font of [16, 20]) {
		await page.evaluate((size) => {
			document.documentElement.style.fontSize = `${size}px`;
		}, font);
		const link = await drag(0.25);
		assert.equal(link.source.reference.surface, 'A');
		assert.equal(link.target.reference.surface, 'B');
		const point = JSON.parse(
			(await page.getByLabel('Warp sample', { exact: true }).textContent()) ?? 'null'
		);
		assert(Math.abs(point.x - 0.55) < 1e-10 && Math.abs(point.y - 0.2) < 1e-10);
		await view.getByRole('button', { name: 'Unlink corner group 1', exact: true }).click();
		assert.equal(
			await view.getByRole('button', { name: 'Unlink corner group 1', exact: true }).count(),
			0
		);
		assert.equal(await drag(1.5), null, 'outside the screen-sized threshold must remain unlinked');
		await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
		assert.equal(
			(await drag(0.25)).target.reference.surface,
			'B',
			'zoom keeps a screen-sized snap threshold'
		);
		await view.getByRole('button', { name: 'Unlink corner group 1', exact: true }).click();
		assert.equal(await drag(1.5), null);
		await view.getByRole('button', { name: 'Reset zoom to 100%', exact: true }).click();
	}
	await page.getByRole('button', { name: 'Toggle Screen snap', exact: true }).click();
	assert.equal(await drag(0), null, 'snap off must not create links');
	await page.evaluate(() => {
		document.documentElement.style.fontSize = '';
	});
}
