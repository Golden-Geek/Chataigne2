import assert from 'node:assert/strict';

// Real bundled buttons and their exact Parameter identities; no domain mutation through DOM.
export async function verifyMatrixScopeControls({ page, matrix, wait, sent }) {
	const initial = (await matrix({ kind: 'Inspect' })).snapshot;
	const rows = initial.items.filter((item) => item.id.kind === 'Layer').map((item) => item.id.id);
	const columns = initial.columns.map((column) => column.id);
	for (const layer of rows)
		for (const column of columns)
			await matrix({
				kind: 'ConfigureCell',
				layer,
				column,
				settings: {
					position: [0, 0],
					size: null,
					fit_mode: 'Fit',
					opacity: 1,
					enabled: true
				}
			});
	const authored = (await matrix({ kind: 'Inspect' })).snapshot;
	const cells = new Map(
		authored.cells.map((cell) => [`${cell.layer}:${cell.definition.column}`, cell.id])
	);
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	async function trigger(owner, field) {
		const button = page.locator(`[data-matrix-control="${field}"][data-owner-uuid="${owner}"]`);
		await button.waitFor();
		await wait(
			() => button.isEnabled(),
			(enabled) => enabled
		);
		await button.scrollIntoViewIfNeeded();
		const zone = page.locator('.source-drop-zone').filter({ has: button });
		if (await zone.count()) await zone.hover({ position: { x: 16, y: 16 } });
		const parameter = Number(
			await wait(
				() => button.getAttribute('data-parameter-id'),
				(value) => Number.isSafeInteger(Number(value)) && Number(value) > 0
			)
		);
		assert(
			Number.isSafeInteger(parameter) && parameter > 0,
			`${owner}/${field} has a real Parameter`
		);
		const before = sent.length;
		await button.click();
		await wait(
			() => sent.slice(before),
			(intents) =>
				intents.some(
					(intent) =>
						intent.kind === 'setParam' &&
						intent.node === parameter &&
						Array.isArray(intent.value?.Trigger)
				)
		);
		return parameter;
	}
	const cell = (row, column) => cells.get(`${rows[row]}:${columns[column]}`);
	const request = (value, row) => value.layers.find((entry) => entry.layer === rows[row]);
	await trigger(columns[0], 'play');
	await wait(playback, (value) => value.desired_column === columns[0]);
	await trigger(cell(1, 2), 'play');
	await wait(playback, (value) => request(value, 1)?.column === columns[2]);
	await trigger(cell(1, 2), 'stop');
	await wait(
		playback,
		(value) => request(value, 1)?.column === null && value.desired_column === columns[0]
	);
	await trigger(cell(0, 1), 'play');
	await wait(playback, (value) => request(value, 0)?.column === columns[1]);
	await trigger(rows[0], 'stop');
	await wait(
		playback,
		(value) => request(value, 0)?.column === null && value.desired_column === columns[0]
	);
	await trigger(cell(1, 2), 'play');
	await trigger(columns[0], 'stop');
	await wait(
		playback,
		(value) => value.desired_column === null && request(value, 1)?.column === columns[2]
	);
	const { items: outerItems } = await matrix({
		kind: 'AddGroup',
		parent: null,
		label: 'Layer Stop Group'
	});
	const outer = outerItems[0];
	const { items: innerItems } = await matrix({
		kind: 'AddGroup',
		parent: outer.id,
		label: 'Nested Layer Stop Group'
	});
	for (const [row, parent] of [
		[rows[0], outer.id],
		[rows[1], innerItems[0].id]
	])
		await matrix({ kind: 'MoveItem', item: { kind: 'Layer', id: row }, parent, after: null });
	for (let row = 0; row < 3; row++) await trigger(cell(row, 2), 'play');
	await trigger(outer.id, 'stop');
	await wait(
		playback,
		(value) =>
			request(value, 0)?.column === null &&
			request(value, 1)?.column === null &&
			request(value, 2)?.column === columns[2]
	);
	const { group } = await matrix({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Column Stop Group'
	});
	const { group: nested } = await matrix({
		kind: 'AddColumnGroup',
		parent: group,
		label: 'Nested Column Stop Group'
	});
	for (const [column, parent] of [
		[columns[0], group],
		[columns[1], nested]
	])
		await matrix({
			kind: 'MoveColumnItem',
			item: { kind: 'Column', id: column },
			parent,
			after: null
		});
	await trigger(columns[2], 'play');
	await trigger(cell(0, 0), 'play');
	await trigger(cell(1, 1), 'play');
	await trigger(group, 'stop');
	await wait(
		playback,
		(value) =>
			request(value, 0)?.column === null &&
			request(value, 1)?.column === null &&
			value.desired_column === columns[2]
	);
	return 'real Column/Cell Play and Cell/Layer/Column/nested Layer Group/nested Column Group Stop buttons send exact Parameter triggers and preserve unrelated scope/anchor';
}
