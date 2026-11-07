import assert from 'node:assert/strict';

// Build the sparse authored scene before opening the browser. Input checks below use real gestures.
export async function prepareCellScrollTransfers(operation) {
	const media = (
		await operation({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Sparse Cell Transfers' })
	).value.reference.media;
	const edit = (edit) => operation({ kind: 'Matrix', media, edit });
	await edit({ kind: 'AddLayers', parent: null, count: 997 });
	await edit({ kind: 'AddColumns', parent: null, count: 997 });
	const initial = (await edit({ kind: 'Inspect' })).value.snapshot;
	assert.equal(initial.items.length, 1000);
	assert.equal(initial.columns.length, 1000);
	const outer = (await edit({ kind: 'AddGroup', parent: null, label: 'Outer transfer group' }))
		.value.items[0].id;
	const inner = (await edit({ kind: 'AddGroup', parent: outer, label: 'Nested transfer group' }))
		.value.items[0].id;
	const nestedRow = initial.items[0].id.id;
	await edit({ kind: 'MoveItem', item: initial.items[0].id, parent: inner, after: null });
	const axisOuter = (
		await edit({ kind: 'AddColumnGroup', parent: null, label: 'Outer transfer columns' })
	).value.group;
	const axisInner = (
		await edit({ kind: 'AddColumnGroup', parent: axisOuter, label: 'Nested transfer columns' })
	).value.group;
	const firstColumn = initial.columns[0].id,
		lastColumn = initial.columns.at(-1).id;
	await edit({
		kind: 'MoveColumnItem',
		item: { kind: 'Column', id: lastColumn },
		parent: axisInner,
		after: null
	});
	const source = (
		await edit({
			kind: 'CreateCellMedia',
			layer: initial.items[1].id.id,
			column: firstColumn,
			media_kind: 'SolidColor'
		})
	).value.cell;
	return {
		media,
		source,
		nestedRow,
		lastColumn,
		peerColumn: initial.columns.at(-2).id,
		rootRow: initial.items.at(-1).id.id
	};
}

export async function verifyCellScrollTransfers({
	scene,
	page,
	read,
	wait,
	show,
	history,
	checks,
	setStage
}) {
	await show('Sparse Cell Transfers', scene.media);
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid' });
	const cell = (row, column) =>
		grid.locator(`[data-cell-layer="${row}"][data-cell-column="${column}"] button.cell`);
	const source = (id) => grid.locator(`button.cell[data-cell-id="${id}"]`);
	const inspect = () => read(scene.media);
	async function clickBody(locator, options = {}) {
		const box = await locator.boundingBox();
		assert(box);
		await locator.click({ ...options, position: { x: box.width / 4, y: box.height * 0.65 } });
	}
	async function settled(id, row, column) {
		await wait(
			() => cell(row, column).getAttribute('data-cell-id'),
			(value) => value === id
		);
		await wait(() => cell(row, column).isEnabled(), Boolean);
	}
	setStage('sparse offscreen clipboard Reference');
	await source(scene.source).waitFor();
	await clickBody(source(scene.source));
	await page.keyboard.press('Control+c');
	const beforeScroll = await inspect();
	const box = await grid.boundingBox();
	assert(box);
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.wheel(1000000, 1000000);
	await cell(scene.nestedRow, scene.lastColumn).waitFor({ state: 'visible' });
	assert.equal(await source(scene.source).count(), 0, 'source is unmounted after scrolling');
	assert.deepEqual((await inspect()).value, beforeScroll.value, 'scrolling does not author cells');
	const mounted = await grid.getByRole('gridcell').count();
	assert(mounted > 0 && mounted <= 64 * 32, `mounted Cells ${mounted}`);
	assert.equal(beforeScroll.value.items.filter((item) => item.id.kind === 'Layer').length, 1000);
	await clickBody(cell(scene.nestedRow, scene.lastColumn));
	await wait(() => cell(scene.nestedRow, scene.lastColumn).isEnabled(), Boolean);
	await page.keyboard.press('Control+Alt+v');
	let after = await wait(inspect, (value) => value.value.cells.length === 2);
	const reference = after.value.cells.find((item) => item.id !== scene.source);
	assert(reference);
	assert.equal(
		reference.definition.source.channel,
		beforeScroll.value.cells[0].definition.source.channel
	);
	await history(beforeScroll, after, scene.media);
	await settled(reference.id, scene.nestedRow, scene.lastColumn);
	checks.push({
		stage: 'sparse offscreen clipboard Reference',
		dimensions: [1000, 1000],
		nested_groups: true,
		mounted,
		exact_history: true
	});

	setStage('sparse Copy clipboard remains after Reference');
	let before = await inspect();
	await clickBody(cell(scene.nestedRow, scene.peerColumn));
	await wait(() => cell(scene.nestedRow, scene.peerColumn).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	after = await wait(inspect, (value) => value.value.cells.length === 3);
	const copy = after.value.cells.find(
		(item) => !before.value.cells.some((old) => old.id === item.id)
	);
	assert(copy);
	assert.notEqual(
		copy.definition.source.channel,
		beforeScroll.value.cells[0].definition.source.channel
	);
	await history(before, after, scene.media);
	await settled(copy.id, scene.nestedRow, scene.peerColumn);
	checks.push({ stage: 'sparse Copy clipboard remains after Reference', exact_history: true });

	async function drag(id, row, column, alt) {
		const from = await source(id).boundingBox(),
			to = await cell(row, column).boundingBox();
		assert(from && to);
		await page.mouse.move(from.x + from.width / 4, from.y + from.height * 0.65);
		await page.mouse.down();
		await page.mouse.move(from.x + from.width / 4 + 12, from.y + from.height * 0.65 + 2, {
			steps: 3
		});
		if (alt) await page.keyboard.down('Alt');
		await page.mouse.move(to.x + to.width / 4 + 2, to.y + to.height * 0.65, { steps: 8 });
		await page.mouse.move(to.x + to.width / 4, to.y + to.height * 0.65);
		await page.mouse.up();
		if (alt) await page.keyboard.up('Alt');
	}
	setStage('sparse grouped Alt Move to root Layer');
	before = await inspect();
	await drag(reference.id, scene.rootRow, scene.peerColumn, true);
	await page.getByRole('menuitem', { name: /^Move\b/ }).click();
	after = await wait(
		inspect,
		(value) => value.value.cells.find((item) => item.id === reference.id)?.layer === scene.rootRow
	);
	await history(before, after, scene.media);
	await settled(reference.id, scene.rootRow, scene.peerColumn);
	checks.push({ stage: 'sparse grouped Alt Move to root Layer', exact_history: true });

	setStage('sparse grouped occupied swap');
	before = await inspect();
	await drag(copy.id, scene.rootRow, scene.peerColumn, false);
	after = await wait(
		inspect,
		(value) => value.value.cells.find((item) => item.id === copy.id)?.layer === scene.rootRow
	);
	assert.equal(after.value.cells.find((item) => item.id === reference.id).layer, scene.nestedRow);
	assert.equal(after.value.cells.length, 3);
	await history(before, after, scene.media);
	checks.push({ stage: 'sparse grouped occupied swap', exact_history: true, sparse_cells: 3 });
}
