import assert from 'node:assert/strict';
import path from 'node:path';

/** Actual app metadata, hierarchy edits and input over a clipped decorative overlay. */
export async function verifyGroupRails({ page, grid, matrix, context, wait, compact, directory }) {
	assert(compact, '--group-rails requires --compact-groups');
	const { group: outer, nested, child } = compact;
	const row = (id) => grid.locator(`[role="rowheader"][data-row-id="${id}"]`);
	const rail = (id) => grid.locator('xpath=..').locator(`path[data-group-rail="${id}"]`);
	const ready = () =>
		wait(
			() => grid.locator('xpath=ancestor::section[1]').getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	const stroke = (id) => rail(id).evaluate((node) => getComputedStyle(node).stroke);
	await rail(nested).waitFor();
	const fallback = await stroke(nested);
	const sourceColor = await stroke(outer);
	await context(row(nested));
	await page.getByRole('menuitem', { name: 'Set Color', exact: true }).click();
	const input = page.locator('.gc-node-context-color-submenu .cp-input-hex');
	await input.fill('38B8B0');
	await input.press('Tab');
	await page.keyboard.press('Escape');
	await wait(
		() => stroke(nested),
		(value) => value === 'rgb(56, 184, 176)'
	);
	assert.equal(await stroke(outer), sourceColor, 'Nested colors belong to their own Group');
	const controls = await row(child).locator('[data-matrix-row-controls]').boundingBox();
	const bracket = await rail(nested).boundingBox();
	assert(
		controls && bracket && controls.x > bracket.x + 8,
		'Membership rails must stay left of Layer controls'
	);
	const shape = await rail(nested).getAttribute('d');
	await ready();
	await row(nested).getByTitle('Select Compact nested', { exact: true }).click();
	assert.equal(await row(nested).getAttribute('aria-selected'), 'true');
	await row(nested).hover();
	assert.equal(await stroke(nested), 'rgb(56, 184, 176)');
	assert.equal(
		await rail(nested).getAttribute('d'),
		shape,
		'Hover/selection do not move membership rails'
	);
	assert.equal(await rail(nested).evaluate((node) => getComputedStyle(node).pointerEvents), 'none');
	await ready();
	await row(child).getByTitle('Select Layer 1', { exact: true }).click();
	assert.equal(await row(child).getAttribute('aria-selected'), 'true');
	const selectedBox = await row(child).boundingBox();
	for (const ancestor of [outer, nested]) {
		const wrap = await rail(ancestor).boundingBox();
		assert(
			wrap && selectedBox && wrap.x + 1 < selectedBox.x,
			'The selected child contour stays inside every ancestor wrap'
		);
	}
	assert.equal(
		await rail(nested)
			.locator('xpath=..')
			.evaluate((node) => getComputedStyle(node).zIndex),
		'1'
	);
	assert.equal(
		await row(child).evaluate((node) => getComputedStyle(node).zIndex),
		'2',
		'Wrap decoration must paint below selected row cards'
	);
	const settings = (await matrix({ kind: 'Inspect' })).snapshot.items.find(
		(item) => item.id.id === nested
	).settings;
	await matrix({
		kind: 'ConfigureItem',
		item: { kind: 'Group', id: nested },
		settings: { ...settings, enabled: false }
	});
	await wait(
		() => rail(nested).getAttribute('data-enabled'),
		(value) => value === 'false'
	);
	assert.equal(await rail(nested).evaluate((node) => getComputedStyle(node).opacity), '0.45');
	assert.equal(await rail(outer).getAttribute('data-enabled'), 'true');
	await matrix({ kind: 'ConfigureItem', item: { kind: 'Group', id: nested }, settings });
	await wait(
		() => rail(nested).getAttribute('data-enabled'),
		(value) => value === 'true'
	);
	const css = await page.evaluate(() => document.documentElement.style.cssText);
	for (const theme of ['dark', 'light']) {
		if (theme === 'light')
			await page.evaluate(() => {
				const root = document.documentElement;
				root.style.setProperty('--gc-color-header', '#f0f0f0');
				root.style.setProperty('--gc-color-panel', '#ffffff');
				root.style.setProperty('--gc-color-text', '#202020');
				root.style.setProperty('--gc-color-accent', '#6550a0');
			});
		assert.equal(await stroke(nested), 'rgb(56, 184, 176)');
		assert.equal(await stroke(outer), sourceColor);
		await page.screenshot({ path: path.join(directory, `group-rails-${theme}.png`) });
	}
	await page.evaluate((css) => {
		document.documentElement.style.cssText = css;
	}, css);
	const { items: longItems } = await matrix({
		kind: 'AddGroup',
		parent: nested,
		label: 'Rail scrolling'
	});
	const long = longItems[0].id;
	await matrix({ kind: 'AddLayers', parent: long, count: 80 });
	await wait(
		() => grid.getAttribute('aria-rowcount'),
		(value) => Number(value) >= 90
	);
	await page.setViewportSize({ width: 1000, height: 1000 });
	await wait(
		() => grid.evaluate((node) => node.scrollHeight - node.clientHeight),
		(value) => value > 3000
	);
	await ready();
	const firstX = (await rail(outer).boundingBox()).x;
	await grid.evaluate((node) => {
		node.scrollTop = 3000;
		node.scrollLeft = node.scrollWidth;
	});
	assert(await grid.evaluate((node) => node.scrollLeft > 0), 'Exercise a real horizontal scroll');
	await wait(
		() => row(outer).count(),
		(value) => value === 0
	);
	await wait(
		() => rail(outer).getAttribute('data-start-clipped'),
		(value) => value === 'true'
	);
	assert.equal(await row(nested).count(), 0);
	assert.equal(await rail(nested).getAttribute('data-start-clipped'), 'true');
	assert.equal(await rail(long).getAttribute('data-start-clipped'), 'true');
	assert.equal(await rail(outer).getAttribute('data-end-clipped'), 'true');
	assert.equal(await stroke(outer), sourceColor);
	assert(
		Math.abs((await rail(outer).boundingBox()).x - firstX) < 1,
		'Rails stay beside horizontally sticky labels'
	);
	assert((await grid.locator('.canvas > .row').count()) <= 64);
	await page.screenshot({ path: path.join(directory, 'group-rails-clipped.png') });
	await grid.evaluate((node) => {
		node.scrollTop = 0;
		node.scrollLeft = 0;
	});
	await wait(
		() => row(nested).count(),
		(value) => value === 1
	);
	await ready();
	await row(nested).getByRole('button', { name: 'Collapse Compact nested', exact: true }).click();
	await wait(
		() => row(child).count(),
		(value) => value === 0
	);
	const rem = await page.evaluate(() =>
		parseFloat(getComputedStyle(document.documentElement).fontSize)
	);
	assert((await rail(nested).boundingBox()).height < 2.1 * rem);
	await ready();
	await row(nested).getByRole('button', { name: 'Expand Compact nested', exact: true }).click();
	await wait(
		() => row(child).count(),
		(value) => value === 1
	);
	const { items: destinations } = await matrix({
		kind: 'AddGroup',
		parent: null,
		label: 'Rail destination'
	});
	const destination = destinations[0].id;
	await matrix({
		kind: 'MoveItem',
		item: { kind: 'Group', id: destination },
		parent: null,
		after: null
	});
	await matrix({
		kind: 'MoveItem',
		item: { kind: 'Group', id: nested },
		parent: destination,
		after: null
	});
	await grid.evaluate((node) => {
		node.scrollTop = 3000;
	});
	await wait(
		() => rail(destination).count(),
		(value) => value === 1
	);
	await wait(
		() => rail(outer).count(),
		(value) => value === 0
	);
	assert.equal(await stroke(nested), 'rgb(56, 184, 176)');
	await matrix({
		kind: 'MoveItem',
		item: { kind: 'Group', id: nested },
		parent: outer,
		after: null
	});
	await matrix({ kind: 'RemoveItem', item: { kind: 'Group', id: long } });
	await matrix({ kind: 'RemoveItem', item: { kind: 'Group', id: destination } });
	await wait(
		() => grid.evaluate((node) => node.scrollTop),
		(value) => value <= 1
	);
	await grid.evaluate((node) => {
		node.scrollLeft = 0;
	});
	await wait(
		() => row(nested).count(),
		(value) => value === 1
	);
	await page.setViewportSize({ width: 1500, height: 1000 });
	await context(row(nested));
	await page.getByRole('menuitem', { name: 'Reset Color', exact: true }).click();
	await wait(
		() => stroke(nested),
		(value) => value === fallback
	);
	assert.equal(await stroke(outer), sourceColor);
	return {
		custom_reset: true,
		nested_owner_colors: true,
		selected_hover: true,
		selected_child_separation: true,
		disabled: true,
		pointer_transparent: true,
		themes: ['dark', 'light'],
		clipped_ancestors: true,
		bounded_window: true,
		shrinking_scroll_extent: true,
		collapse: true,
		reparent_reorder: true
	};
}
