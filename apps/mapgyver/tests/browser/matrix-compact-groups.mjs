import assert from 'node:assert/strict';

/** Functional Group presentation over the real bundled grid and Golden Inspector. */
export async function verifyCompactGroups({ page, grid, matrix, group, wait }) {
	const row = (id) => grid.locator(`[role="rowheader"][data-row-id="${id}"]`);
	const ready = () =>
		wait(
			() => grid.locator('xpath=ancestor::section[1]').getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const settings = async () =>
		(await inspect()).items.find((item) => item.id.id === group).settings;
	const parameter = (label) =>
		page
			.locator('.inspector .parameter-inspector')
			.filter({ has: page.getByText(label, { exact: true }) });
	const select = async (id) => {
		await ready();
		await row(id)
			.getByTitle(/^Select /)
			.click();
		await wait(
			() => page.locator('.inspector').getAttribute('data-inspected-uuid'),
			(value) => value === id
		);
	};
	const geometry = async () =>
		grid.locator('.canvas > .row').evaluateAll((nodes) => {
			const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
			return nodes.map((node) => {
				const bounds = node.getBoundingClientRect();
				return {
					id: node.getAttribute('data-row-id'),
					group: node.classList.contains('group'),
					top: bounds.top,
					bottom: bounds.bottom,
					height: bounds.height / rem
				};
			});
		});
	const before = await geometry();
	assert(
		before.some((entry) => entry.id === group && Math.abs(entry.height - 2.1) < 0.03),
		'The tested binary must contain the compact Group contract'
	);
	const { items: nestedItems } = await matrix({
		kind: 'AddGroup',
		parent: group,
		label: 'Compact nested'
	});
	const nested = nestedItems[0].id;
	const { items: layers } = await matrix({ kind: 'AddLayers', parent: nested, count: 1 });
	const child = layers[0].id;
	const { items: emptyItems } = await matrix({
		kind: 'AddGroup',
		parent: nested,
		label: 'Compact empty'
	});
	const empty = emptyItems[0].id;
	await wait(
		() => row(empty).count(),
		(value) => value === 1
	);
	for (const id of [group, nested, empty]) {
		assert.equal(await row(id).locator('[data-matrix-row-controls]').count(), 0);
		await select(id);
		await parameter('Opacity').locator('input.number-field').waitFor();
		await parameter('Blend Mode').locator('select').waitFor();
	}
	assert.equal(
		await row(child).locator('[data-matrix-row-controls]').count(),
		1,
		'Layers retain their existing inline controls'
	);
	await select(group);
	await parameter('Opacity').locator('input.number-field').fill('0.625');
	await parameter('Opacity').locator('input.number-field').press('Enter');
	await wait(settings, (value) => value.opacity === 0.625);
	await parameter('Blend Mode').locator('select').selectOption('Add');
	await wait(settings, (value) => value.blend === 'Add');
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(settings, (value) => value.blend === 'Normal' && value.opacity === 0.625);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(settings, (value) => value.opacity === 1);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(settings, (value) => value.opacity === 0.625);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(settings, (value) => value.blend === 'Add');
	await select(child);
	await ready();
	await row(group)
		.getByRole('button', { name: /^Collapse / })
		.click();
	await wait(
		() => row(child).count(),
		(value) => value === 0
	);
	assert.equal(await row(nested).count(), 0);
	assert.equal(await page.locator('.inspector').getAttribute('data-inspected-uuid'), child);
	await parameter('Opacity').locator('input.number-field').waitFor();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => row(child).count(),
		(value) => value === 1
	);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(
		() => row(child).count(),
		(value) => value === 0
	);
	await ready();
	await row(group)
		.getByRole('button', { name: /^Expand / })
		.click();
	await wait(
		() => row(child).count(),
		(value) => value === 1
	);
	await ready();
	await row(empty)
		.getByRole('button', { name: /^Collapse / })
		.click();
	await wait(
		() => row(empty).getAttribute('aria-expanded'),
		(value) => value === 'false'
	);
	await ready();
	await row(empty)
		.getByRole('button', { name: /^Expand / })
		.click();
	await wait(
		() => row(empty).getAttribute('aria-expanded'),
		(value) => value === 'true'
	);
	const samples = [];
	for (const font of [12, 16, 24]) {
		await page.evaluate((font) => {
			document.documentElement.style.fontSize = `${font}px`;
		}, font);
		await page.setViewportSize({ width: font === 24 ? 1100 : 1500, height: 1000 + font });
		await ready();
		const rows = await geometry();
		for (const entry of rows) {
			if (entry.group) assert(Math.abs(entry.height - 2.1) < 0.04, JSON.stringify(entry));
			else assert(entry.height >= 7.49, JSON.stringify(entry));
		}
		for (let i = 1; i < rows.length; i++)
			assert(
				rows[i].top >= rows[i - 1].bottom - 1,
				'Mixed-height row hit regions must not overlap'
			);
		samples.push({
			font,
			rows: rows.length,
			group_height: rows.find((entry) => entry.group).height
		});
	}
	await page.evaluate(() => {
		document.documentElement.style.fontSize = '';
	});
	await page.setViewportSize({ width: 1500, height: 1000 });
	await ready();
	return {
		group,
		nested,
		child,
		empty,
		inspector: true,
		history: true,
		hidden_selection: true,
		mixed_geometry: samples,
		expected: { opacity: 0.625, blend: 'Add' }
	};
}

export async function verifyExistingCompactGroup({ page, post, wait, project }) {
	await post('/api/ui/project-load', { path: project });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Selection colors', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const group = grid.getByRole('rowheader', { name: 'Color Group', exact: true });
	await group.waitFor();
	assert.equal(await group.locator('[data-matrix-row-controls]').count(), 0);
	const rem = await page.evaluate(() =>
		parseFloat(getComputedStyle(document.documentElement).fontSize)
	);
	const box = await group.boundingBox();
	assert(box && Math.abs(box.height / rem - 2.1) < 0.03);
	assert((await group.getAttribute('class')).includes('custom-color'));
	await group.getByTitle('Select Color Group', { exact: true }).click();
	const opacity = page
		.locator('.inspector .parameter-inspector')
		.filter({ has: page.getByText('Opacity', { exact: true }) })
		.locator('input.number-field');
	await wait(
		() => opacity.inputValue(),
		(value) => Number(value) === 1
	);
	const blend = page
		.locator('.inspector .parameter-inspector')
		.filter({ has: page.getByText('Blend Mode', { exact: true }) })
		.locator('select');
	assert.equal(await blend.inputValue(), 'Normal');
	return { project, compact: true, opacity: 1, blend: 'Normal', custom_color: true };
}
