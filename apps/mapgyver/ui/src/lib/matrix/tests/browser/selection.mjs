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
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		1,
		'Cell selection cannot cross-select its headers'
	);
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	const empty = grid.locator('[role="gridcell"][title="Layer A / Column B"]');
	assert.equal(await empty.getAttribute('data-cell-id'), null);
	const emptyBounds = await empty.boundingBox();
	assert(emptyBounds);
	const beforeEmpty = await count.textContent();
	await empty.click({ position: { x: emptyBounds.width * 0.2, y: emptyBounds.height * 0.75 } });
	assert.equal(await selected.textContent(), 'cell-layer-0-column-0');
	assert.equal(await count.textContent(), beforeEmpty, 'empty Cell body does not select the Layer');
	assert.equal(await cell.getAttribute('aria-selected'), 'true');
	await grid.getByRole('button', { name: 'Add Media to Layer A / Column B', exact: true }).click();
	assert.equal(await selected.textContent(), 'layer-0');
	await press(grid.getByRole('button', { name: 'Layer A', exact: true }), 'layer-0');
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		1,
		'Layer selection cannot retain a Cell or Column outline'
	);
	await press(grid.getByRole('button', { name: 'Column A', exact: true }), 'column-0');
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		1,
		'Column selection cannot retain a row or Cell outline'
	);
	assert.equal(
		await grid.locator('button[aria-pressed="true"]').count(),
		0,
		'Header labels have no second selection outline'
	);
	await press(grid.getByRole('button', { name: 'Group A', exact: true }), 'group');
	assert.equal(await grid.locator('[aria-selected="true"]').count(), 1);
	await press(grid.getByRole('button', { name: 'Layer Group', exact: true }), 'layer-group');
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		1,
		'A nested row group selects only its own header'
	);
	await page.keyboard.down('Control');
	await cell.click({ position: await cellPoint() });
	await page.keyboard.up('Control');
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		2,
		'Control adds an explicit second selection'
	);
	await page.keyboard.down('Control');
	await cell.click({ position: await cellPoint() });
	await page.keyboard.up('Control');
	assert.equal(
		await grid.locator('[aria-selected="true"]').count(),
		1,
		'Control toggles only the requested Cell'
	);
	const colored = [
		cell,
		grid.locator('[role="rowheader"][aria-label="Layer A"]'),
		grid.locator('[data-column-id][role="columnheader"]').first(),
		grid.locator('[data-column-group-id]').first(),
		grid.locator('[role="rowheader"][aria-label="Layer Group"]')
	];
	const beforeColors = await Promise.all(
		colored.map((node) => node.evaluate((element) => getComputedStyle(element).backgroundColor))
	);
	await page.getByRole('button', { name: 'Set grid custom colors', exact: true }).click();
	for (const [index, node] of colored.entries()) {
		await page
			.waitForFunction(
				(element) => element?.classList.contains('custom-color'),
				await node.elementHandle()
			)
			.catch(async (error) => {
				throw new Error(
					`Custom color target ${index}: ${await node.getAttribute('class')}; ${error}`
				);
			});
		assert.notEqual(
			await node.evaluate((element) => getComputedStyle(element).backgroundColor),
			beforeColors[index],
			'Golden metadata change updates grid paint'
		);
	}
	await page.getByRole('button', { name: 'Select Layer B externally', exact: true }).click();
	for (const theme of [
		{ header: '#181818', panel: '#1f2021', text: '#bababa' },
		{ header: '#f0f0f0', panel: '#ffffff', text: '#202020' }
	]) {
		const ratios = await page.evaluate(
			({ theme }) => {
				const root = document.documentElement,
					previous = root.style.cssText;
				root.style.setProperty('--gc-color-header', theme.header);
				root.style.setProperty('--gc-color-panel', theme.panel);
				root.style.setProperty('--gc-color-text', theme.text);
				const canvas = document.createElement('canvas');
				canvas.width = canvas.height = 1;
				const context = /** @type {CanvasRenderingContext2D} */ (
					canvas.getContext('2d', { willReadFrequently: true })
				);
				if (!context) throw new Error('2D context unavailable');
				/** @param {string} color */
				function luminance(color) {
					context.fillStyle = color;
					context.fillRect(0, 0, 1, 1);
					const values = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map((part) => {
						const value = part / 255;
						return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
					});
					return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
				}
				const results = [...document.querySelectorAll('.custom-color')].map((element) => {
					const style = getComputedStyle(element),
						a = luminance(style.color),
						b = luminance(style.backgroundColor);
					return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
				});
				root.style.cssText = previous;
				return results;
			},
			{ theme }
		);
		assert(ratios.length >= 5);
		assert(
			ratios.every((ratio) => ratio >= 4.5),
			`Custom-color text contrast: ${JSON.stringify({ theme, ratios })}`
		);
	}
	assert.equal(await grid.locator('[aria-selected="true"]').count(), 1);
	assert.equal(
		await grid.locator('[role="rowheader"][aria-label="Layer B"]').getAttribute('aria-selected'),
		'true',
		'Shared selection updates even a disabled Layer independently of retained grid coordinates'
	);
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
	await other.getByRole('button', { name: 'Layer B', exact: true }).click();
	assert.equal(
		await grid.locator('[role="rowheader"][aria-label="Layer B"]').getAttribute('aria-selected'),
		'true',
		'Independent panels paint the same explicit document selection'
	);
	assert.equal(await other.locator('[aria-selected="true"]').count(), 1);
	assert.equal(
		await other
			.locator('[role="rowheader"][aria-label="Layer A"]')
			.getAttribute('class')
			.then((value) => value?.includes('custom-color')),
		true,
		'Independent panels read the same Golden colors'
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
