import assert from 'node:assert/strict';

/** Presentation only: production grid, ordinary selection and renderer-state declarations. */
/** @param {import('playwright-core').Page} page */
export async function gridAppearanceChecks(page) {
	await page.getByRole('button', { name: 'Show Matrix grid', exact: true }).click();
	const editor = page.getByLabel('Grid editor A', { exact: true });
	const grid = editor.getByRole('grid');
	const cell = grid.locator('[role="gridcell"][title="Layer A / Column A"]');
	const other = grid.locator('[role="gridcell"][title="Layer B / Column A"]');
	const toggle = page.getByRole('button', { name: 'Toggle submitted Cell', exact: true });
	await page.getByRole('button', { name: 'Set grid custom colors', exact: true }).click();
	await page.mouse.move(0, 0);
	await page.waitForFunction(
		(node) => node?.classList.contains('custom-color'),
		await cell.elementHandle()
	);
	const before = await cell.boundingBox();
	assert(before);
	const styles = () =>
		cell.evaluate((node) => {
			const style = getComputedStyle(node);
			return {
				radius: parseFloat(style.borderRadius),
				border: style.borderColor,
				shadow: style.boxShadow,
				transition: style.transitionDuration,
				fill: style.backgroundColor
			};
		});
	const idle = await styles();
	assert(idle.radius > 0 && idle.radius <= 8, 'Small relative-unit corners');
	assert.equal(idle.shadow, 'none');
	await toggle.click();
	await page.waitForFunction(
		(node) =>
			node?.getAttribute('data-submitted') === 'true' &&
			getComputedStyle(node).boxShadow !== 'none',
		await cell.elementHandle()
	);
	assert.equal(
		await other.getAttribute('data-submitted'),
		'false',
		'A global Column mix cannot activate an unrelated Layer'
	);
	assert.deepEqual(
		await cell.boundingBox(),
		before,
		'Elevation cannot change hit bounds or preview placement'
	);
	assert.equal((await styles()).fill, idle.fill, 'Active feedback preserves authored color');
	await cell.click({ position: { x: before.width * 0.8, y: before.height * 0.75 } });
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	assert.notEqual((await styles()).shadow, 'none', 'Selection and active feedback coexist');
	await toggle.click();
	await toggle.click();
	await toggle.click();
	await page.waitForFunction(
		(node) =>
			node?.getAttribute('data-submitted') === 'false' &&
			getComputedStyle(node).boxShadow === 'none',
		await cell.elementHandle()
	);
	assert.equal(
		await cell.getAttribute('aria-selected'),
		'true',
		'Interrupted playback paint does not change selection'
	);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	assert.equal((await styles()).transition, '0s');
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	const previous = await page.evaluate(() => document.documentElement.style.cssText);
	for (const { width, font, header, panel, text } of [
		{ width: 1000, font: 16, header: '#181818', panel: '#1f2021', text: '#bababa' },
		{ width: 600, font: 20, header: '#f0f0f0', panel: '#ffffff', text: '#202020' }
	]) {
		await page.setViewportSize({ width, height: 800 });
		await page.evaluate(
			({ font, header, panel, text }) => {
				const root = document.documentElement;
				root.style.fontSize = `${font}px`;
				root.style.setProperty('--gc-color-header', header);
				root.style.setProperty('--gc-color-panel', panel);
				root.style.setProperty('--gc-color-text', text);
			},
			{ font, header, panel, text }
		);
		await editor.evaluate((node) => {
			node.style.maxInlineSize = '28rem';
		});
		await cell.scrollIntoViewIfNeeded();
		const box = await cell.boundingBox();
		assert(box && box.width > 0 && box.height > 0);
		assert.equal(await cell.getAttribute('aria-selected'), 'true');
		assert.equal(Math.round((await styles()).radius * 10), Math.round(font * 0.3 * 10));
	}
	await page.evaluate((style) => {
		document.documentElement.style.cssText = style;
	}, previous);
	await page.setViewportSize({ width: 1000, height: 800 });
}
