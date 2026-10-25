import assert from 'node:assert/strict';

export async function gestureChecks(page, record) {
	for (const mode of ['move', 'resize']) {
		let scene, origin;
		const command = () =>
			mode === 'move'
				? { kind: 'move', origin }
				: { kind: 'resize', origin, edges: { left: false, top: false, right: true, bottom: true } };
		const update = (dx = 24) => ({ kind: 'update', point: { x: origin.x + dx, y: origin.y + 16 } });
		const setup = async (options = {}) => {
			scene = await page.evaluate(() =>
				window.dashboardFixture.setup('free', true, 600, 'dense', true)
			);
			await page.evaluate(
				({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[0]),
				scene
			);
			await page.evaluate((options) => window.dashboardFixture.configureEdits(options), options);
			const box = await page
				.locator(`.dashboard-widget-shell[data-node-id="${scene.ids[0]}"]`)
				.boundingBox();
			assert(box);
			origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		};
		const edit = (command) =>
			page.evaluate((command) => window.dashboardFixture.layoutEdit(0, command), command);
		const state = () => page.evaluate(() => window.dashboardFixture.editState());
		const closed = async (expectedEnds = 1) => {
			try {
				await page.waitForFunction(
					(ends) => {
						const s = window.dashboardFixture.editState();
						return (
							!s.active &&
							s.pending === 0 &&
							s.records.filter((r) => r.kind === 'endEdit').length === ends
						);
					},
					expectedEnds,
					{ timeout: 5000 }
				);
			} catch (error) {
				throw new Error(`${error}: ${JSON.stringify(await state())}`);
			}
			const s = await state();
			assert.equal(s.records.filter((r) => r.kind === 'beginEdit').length, 1);
			assert.equal(s.records.filter((r) => r.kind === 'endEdit').length, expectedEnds);
			return s;
		};
		const writes = (s) => s.records.filter((r) => r.kind === 'setParam').length;

		await setup();
		const before = await page
			.locator(`.dashboard-widget-shell[data-node-id="${scene.ids[0]}"]`)
			.boundingBox();
		assert(await edit(command()));
		assert(await edit(update()));
		assert(await edit({ kind: 'finish' }));
		const after = await page
			.locator(`.dashboard-widget-shell[data-node-id="${scene.ids[0]}"]`)
			.boundingBox();
		assert(before && after);
		assert(
			mode === 'move' ? after.x - before.x > 10 : after.width - before.width > 10,
			JSON.stringify({ mode, before, after })
		);
		assert(writes(await closed()) > 0);
		record(`free-${mode}-commits-visible-geometry`);

		await setup({ holdBegin: true });
		assert(await edit(command()));
		assert(await edit(update()));
		assert.equal(writes(await state()), 0);
		await page.evaluate(() => window.dashboardFixture.editing(false));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert.equal(writes(await closed()), 0);
		assert.equal(await edit(update(48)), false);
		record(`free-${mode}-late-begin-closes-on-mode-exit`);

		for (const retirement of ['mode', 'layout', 'removed', 'reparented', 'replaced']) {
			await setup({ holdWrites: true });
			assert(await edit(command()));
			assert(await edit(update()));
			const admitted = writes(await state());
			assert(admitted > 0);
			assert(await edit(update(48)));
			if (retirement === 'mode') await page.evaluate(() => window.dashboardFixture.editing(false));
			if (retirement === 'layout')
				await page.evaluate(() =>
					window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'grid' })
				);
			if (retirement === 'removed') await page.evaluate(() => window.dashboardFixture.remove(0));
			if (retirement === 'reparented')
				await page.evaluate(() => window.dashboardFixture.reparent(0));
			if (retirement === 'replaced')
				await page.evaluate(() => window.dashboardFixture.refreshIdentities());
			await page.evaluate(() => window.dashboardFixture.releaseEdits());
			assert.equal(
				writes(await closed(retirement === 'replaced' ? 0 : 1)),
				admitted,
				`${mode}: retired tail must not reach transport`
			);
			assert.equal(await page.locator('.free-layout-active').count(), 0);
			assert.equal(await edit(update(72)), false);
			record(`free-${mode}-retires-${retirement}`, { admitted });
		}

		await setup();
		assert.deepEqual(
			await page.evaluate(
				(commands) => window.dashboardFixture.layoutEdits(0, commands),
				[command(), update(), { kind: 'cancel' }]
			),
			[true, true, true]
		);
		assert.equal(writes(await closed()), 0);
		record(`free-${mode}-cancels-unsent-sample`);

		await setup();
		const expectedWrites = mode === 'move' ? 1 : 2;
		await page.evaluate(
			(commands) => window.dashboardFixture.layoutEdits(0, commands),
			[command(), update(), update(48), update(72), { kind: 'finish' }]
		);
		assert.equal(writes(await closed()), expectedWrites);
		record(`free-${mode}-coalesces-before-final-flush`);

		await setup();
		await page.evaluate(() => window.dashboardFixture.selectMany([0, 1]));
		const selected = await page.evaluate(() => window.dashboardFixture.selected());
		assert(await edit(command()));
		assert(await edit(update()));
		assert(await edit({ kind: 'finish' }));
		assert.deepEqual(await page.evaluate(() => window.dashboardFixture.selected()), selected);
		assert.equal(writes(await closed()), expectedWrites * 2);
		record(`free-${mode}-retains-group-selection`);
	}
}
