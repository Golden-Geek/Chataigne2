import assert from 'node:assert/strict';

/** @param {import('playwright-core').Page} page */
export async function gridColumnEnabledChecks(page) {
	const grid = page.getByLabel('Grid editor A', { exact: true }).getByRole('grid');
	const header = grid.locator('[role="columnheader"][data-column-id]').first();
	const stop = header.getByRole('button', { name: 'Stop Column A', exact: true });
	/** @param {string} name */
	const toggle = (name) => page.getByRole('button', { name, exact: true }).click();
	await toggle('Toggle column-0 enabled');
	await page.waitForFunction(
		(node) => node?.classList.contains('disabled-item'),
		await header.elementHandle()
	);
	assert.equal(await stop.isDisabled(), true);
	await header.getByRole('button', { name: 'Column A', exact: true }).click();
	assert.equal(
		await header.getAttribute('aria-selected'),
		'true',
		'Disabled Columns stay inspectable'
	);
	await toggle('Toggle group enabled');
	await toggle('Toggle group enabled');
	assert.equal(await stop.isDisabled(), true, 'Parent toggles preserve the child enabled metadata');
	await toggle('Toggle column-0 enabled');
	await page.waitForFunction(
		(node) => !node?.classList.contains('disabled-item'),
		await header.elementHandle()
	);
	await page.waitForFunction(
		(node) => node instanceof HTMLButtonElement && !node.disabled,
		await stop.elementHandle()
	);
	await toggle('Toggle group enabled');
	await page.waitForFunction(
		(node) => node?.classList.contains('disabled-item'),
		await header.elementHandle()
	);
	assert.equal(await stop.isDisabled(), true, 'Column pulses honor inherited disablement');
	await toggle('Toggle group enabled');
	await page.waitForFunction(
		(node) => !node?.classList.contains('disabled-item'),
		await header.elementHandle()
	);
	await page.waitForFunction(
		(node) => node instanceof HTMLButtonElement && !node.disabled,
		await stop.elementHandle()
	);
}
