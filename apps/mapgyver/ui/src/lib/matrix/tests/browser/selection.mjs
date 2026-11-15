import assert from 'node:assert/strict';

/** Production grid components with a connected public Golden fixture session; no native pixels. */
/** @param {import('playwright-core').Page} page */
export async function gridSelectionChecks(page) {
	page.setDefaultTimeout(5000);
	await page.getByRole('button', { name: 'Show Matrix grid', exact: true }).click();
	const editor = page.getByLabel('Grid editor A', { exact: true });
	const grid = editor.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const selected = page.getByLabel('Grid selected node');
	const count = page.getByLabel('Grid selections');
	const mutations = page.getByLabel('Grid mutations');
	const cell = grid.locator('[role="gridcell"][title="Layer A / Column A"]');
	const cellPoint = async () => {
		const box = await cell.boundingBox();
		assert(box);
		return { x: box.width * 0.8, y: box.height * 0.8 };
	};
	await page
		.waitForFunction(
			() => {
				const button = document.querySelector('button[aria-label="Stop Layer A"]');
				return button && !button.hasAttribute('disabled');
			},
			null,
			{ timeout: 5000 }
		)
		.catch(async (error) => {
			throw new Error(
				`${error}; connection=${await page.getByLabel('Grid connection').textContent()}; stop=${await grid.getByRole('button', { name: 'Stop Layer A', exact: true }).getAttribute('disabled')}`
			);
		});
	/** @param {import('playwright-core').Locator} target @param {string} expected */
	const press = async (target, expected) => {
		const before = Number(await count.textContent());
		await target.hover(target === cell ? { position: await cellPoint() } : {});
		await page.mouse.down();
		assert.equal(await selected.textContent(), expected, 'selection happens before release');
		assert.equal(Number(await count.textContent()), before + 1, 'nested label cannot select twice');
		await page.mouse.up();
		assert.equal(Number(await count.textContent()), before + 1, 'physical release cannot reselect');
	};
	const fill = await cell.evaluate((element) => getComputedStyle(element).backgroundColor);
	await cell.hover({ position: await cellPoint() });
	assert.equal(await cell.getAttribute('data-viewport-hovered'), 'true');
	assert.equal(await selected.textContent(), 'none', 'hover does not select');
	assert.equal(await cell.evaluate((element) => getComputedStyle(element).backgroundColor), fill);
	await press(cell, 'cell-layer-0-column-0');
	await press(grid.getByRole('button', { name: 'Layer A', exact: true }), 'layer-0');
	await press(grid.getByRole('button', { name: 'Column A', exact: true }), 'column-0');
	await press(grid.getByRole('button', { name: 'Group A', exact: true }), 'group');
	for (const [selector, expected] of [
		['[role="rowheader"][aria-label="Layer B"]', 'layer-1'],
		['[role="columnheader"][data-column-id]', 'column-0'],
		['[data-column-group-id]', 'group']
	]) {
		const target = grid.locator(selector).first();
		await target.dispatchEvent('pointerdown', { button: 0 });
		assert.equal(await selected.textContent(), expected, 'header body selects on down');
		await target.focus();
		await page.keyboard.press('Enter');
		assert.equal(await selected.textContent(), expected);
	}
	assert.equal(await mutations.textContent(), '0', 'selection sends no authored edits');
	assert.equal(await page.getByLabel('Grid launches').textContent(), '0');
	const keyboardBefore = Number(await count.textContent());
	await cell.focus();
	await page.keyboard.press('Enter');
	assert.equal(await selected.textContent(), 'cell-layer-0-column-0');
	assert.equal(Number(await count.textContent()), keyboardBefore + 1);
	await page.keyboard.press('Space');
	assert.equal(
		Number(await count.textContent()),
		keyboardBefore + 2,
		'Space activation selects once without a later native click'
	);
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'matrix', 'idle Escape selects edited owner');
	assert.equal(await grid.locator('[aria-selected="true"]').count(), 0);
	await cell.hover({ position: await cellPoint() });
	await page.mouse.down();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'matrix');
	await page.mouse.up();
	assert.equal(
		await selected.textContent(),
		'matrix',
		'late release after clear cannot restore child'
	);
	const area = await grid.boundingBox();
	assert(area);
	await press(cell, 'cell-layer-0-column-0');
	await page.mouse.move(area.x + area.width - 8, area.y + area.height - 8);
	await page.mouse.down();
	assert.equal(await selected.textContent(), 'matrix', 'background selects owner before release');
	await page.mouse.up();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'matrix', 'background retains keyboard focus');

	await press(cell, 'cell-layer-0-column-0');
	const beforeControl = Number(await count.textContent());
	await grid.getByRole('button', { name: 'Stop Layer A', exact: true }).click();
	assert.equal(await selected.textContent(), 'cell-layer-0-column-0');
	assert.equal(
		Number(await count.textContent()),
		beforeControl,
		'nested trigger cannot select header/owner'
	);
	assert.equal(await mutations.textContent(), '1');
	await grid.getByRole('button', { name: 'Layer A', exact: true }).dblclick();
	const rename = grid.getByRole('textbox', { name: 'Rename Layer A', exact: true });
	await rename.fill('Temporary');
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'layer-0', 'rename Escape belongs to text tool');
	assert.equal(await rename.count(), 0);
	assert.equal(await mutations.textContent(), '1', 'rename cancellation cannot mutate');
	await grid.getByRole('button', { name: 'Layer B', exact: true }).dblclick();
	await grid.getByRole('textbox', { name: 'Rename Layer B', exact: true }).fill('Other');
	await page.keyboard.press('Escape');
	assert.equal(
		await selected.textContent(),
		'layer-1',
		'unselected label still admits native double-click rename'
	);
	assert.equal(await mutations.textContent(), '1');
	await cell.dblclick({ position: await cellPoint() });
	assert.equal(
		await page.getByLabel('Grid launches').textContent(),
		'1',
		'double click still launches once'
	);

	await page.getByRole('button', { name: 'Toggle grid disabled', exact: true }).click();
	const beforeDisabled = await selected.textContent();
	await grid
		.locator('[data-column-id][role="columnheader"]')
		.last()
		.dispatchEvent('pointerdown', { button: 0 });
	await grid.focus();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), beforeDisabled, 'disabled grid refuses input');
	await page.getByRole('button', { name: 'Toggle grid disabled', exact: true }).click();

	const header = grid.locator('[role="rowheader"][aria-label="Layer A"]');
	await header.dispatchEvent('dragstart', {
		dataTransfer: await page.evaluateHandle(() => new DataTransfer())
	});
	await grid.focus();
	await page.getByRole('button', { name: 'Toggle independent grid', exact: true }).click();
	const other = page.getByLabel('Grid editor B', { exact: true }).getByRole('grid');
	await other.focus();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'matrix');
	await grid.focus();
	const clearsBefore = await page.getByLabel('Grid clears').textContent();
	await page.keyboard.press('Escape');
	assert.equal(
		await page.getByLabel('Grid clears').textContent(),
		clearsBefore,
		'this editor still owns its drag'
	);
	await page.keyboard.press('Escape');
	assert.equal(
		Number(await page.getByLabel('Grid clears').textContent()),
		Number(clearsBefore) + 1
	);
	await page.getByRole('button', { name: 'Toggle independent grid', exact: true }).click();

	const detached = await cell.elementHandle();
	assert(detached);
	await page.getByRole('button', { name: 'Show Surface Source', exact: true }).click();
	assert.equal(await page.getByLabel('Inspected owner').textContent(), 'A');
	await detached.evaluate((element) =>
		element.dispatchEvent(new PointerEvent('pointerdown', { button: 0 }))
	);
	assert.equal(
		await page.getByLabel('Inspected owner').textContent(),
		'A',
		'destroyed target removes its selection listeners'
	);
}
