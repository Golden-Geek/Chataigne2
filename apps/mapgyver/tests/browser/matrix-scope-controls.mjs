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
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const label = (name) => grid.getByTitle(`Select ${name}`, { exact: true }).click();
	async function inspectorStop(owner, select, both = false, all = false) {
		await trigger(columns[2], 'play');
		await trigger(cell(0, 0), 'play');
		await trigger(cell(1, 1), 'play');
		const control = all
			? page.locator('[data-matrix-stop-all]')
			: page.locator(`[data-matrix-control="stop"][data-owner-uuid="${owner}"]`);
		const parameter = Number(await control.getAttribute('data-parameter-id'));
		assert(Number.isSafeInteger(parameter) && parameter > 0);
		await page.keyboard.press('Escape');
		await select();
		const inspector = page.locator(`.parameter-inspector[data-node-id="${parameter}"]`);
		const button = inspector.getByRole('button', {
			name: 'Trigger',
			exact: true
		});
		await button.waitFor();
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
		await wait(playback, (value) =>
			all
				? value.desired_column === null && value.layers.length === 0
				: value.desired_column === columns[2] &&
					request(value, 0)?.column === null &&
					request(value, 1)?.column === (both ? null : columns[1])
		);
	}
	const selectCell = async () => {
		const target = grid
			.getByRole('gridcell')
			.filter({ has: page.locator('.source-label') })
			.and(
				page.locator(
					`[title="${initial.items.find((item) => item.id.id === rows[0]).label} / ${initial.columns[0].label}"]`
				)
			);
		const bounds = await target.boundingBox();
		assert(bounds);
		await target.click({ position: { x: 8, y: bounds.height - 8 } });
	};
	await inspectorStop(cell(0, 0), selectCell);
	await inspectorStop(rows[0], () =>
		label(initial.items.find((item) => item.id.id === rows[0]).label)
	);
	await inspectorStop(columns[0], () => label(initial.columns[0].label));
	await inspectorStop(outer.id, () => label('Layer Stop Group'), true);
	await inspectorStop(group, () => label('Column Stop Group'), true);
	await inspectorStop(
		null,
		async () => {
			await page
				.getByRole('tab')
				.filter({ hasText: /^Medias$/ })
				.click();
			await page
				.locator('[data-panel-id="media-manager"]')
				.getByRole('button', { name: 'Typed Matrix controls', exact: true })
				.click();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Matrix(?::|$)/ })
				.click();
		},
		true,
		true
	);
	for (const individual of [false, true]) {
		const owner = individual ? cell(0, 0) : columns[0];
		for (const field of ['play', 'cut']) {
			await matrix({ kind: 'ClearPlayback' });
			const play = page.locator(`[data-matrix-control="play"][data-owner-uuid="${owner}"]`);
			const playParameter = Number(await play.getAttribute('data-parameter-id'));
			await page.keyboard.press('Escape');
			if (individual) await selectCell();
			else await label(initial.columns[0].label);
			await page.locator(`.parameter-inspector[data-node-id="${playParameter}"]`).waitFor();
			const name = 'Trigger';
			const parameterRow =
				field === 'play'
					? page.locator(`.parameter-inspector[data-node-id="${playParameter}"]`)
					: page
							.locator('.parameter-inspector')
							.filter({ has: page.getByText('Cut', { exact: true }) });
			const parameter = Number(await parameterRow.getAttribute('data-node-id'));
			assert(Number.isSafeInteger(parameter) && parameter > 0);
			const before = sent.length;
			await parameterRow.getByRole('button', { name, exact: true }).click();
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
			await wait(playback, (value) =>
				individual
					? value.desired_column === null && request(value, 0)?.column === columns[0]
					: value.desired_column === columns[0] && value.layers.length === 0
			);
		}
	}
	return 'real grid and generic Inspector Stops at all six scopes plus Column/Cell Play/Cut send exact Parameter triggers and preserve the independent/anchor contract';
}
