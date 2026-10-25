import assert from 'node:assert/strict';

export async function orderedMoveChecks(page, record) {
	for (const layout of ['horizontal', 'vertical', 'grid', 'tabs', 'accordion']) {
		let scene;
		const selector = (index = 0) => `.dashboard-widget-shell[data-node-id="${scene.ids[index]}"]`;
		const state = () => page.evaluate(() => window.dashboardFixture.editState());
		const moves = (s) => s.records.filter((r) => r.kind === 'moveDashboardWidget');
		const edit = (command, index = 0) =>
			page.evaluate(({ command, index }) => window.dashboardFixture.layoutEdit(index, command), {
				command,
				index
			});
		const setup = async (options = {}) => {
			scene = await page.evaluate(() =>
				window.dashboardFixture.setup('free', true, 600, 'dense', true)
			);
			await page.evaluate(
				(layout) =>
					window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout }),
				layout
			);
			await page.evaluate(
				({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[0]),
				scene
			);
			await page.evaluate((options) => window.dashboardFixture.configureEdits(options), options);
		};
		const start = async (index = 0) => {
			const box = await page.locator(selector(index)).boundingBox();
			assert(box);
			assert(
				await edit(
					{ kind: 'move', origin: { x: box.x + box.width / 2, y: box.y + box.height / 2 } },
					index
				)
			);
		};
		const update = async (index = 0) => {
			const destination = await page.locator('.dashboard-page-frame').evaluate((frame) => {
				const box = frame.getBoundingClientRect(),
					point = { x: box.right - 24, y: box.bottom - 24 };
				const target = document
					.elementFromPoint(point.x, point.y)
					?.closest('[data-dashboard-surface-node-id]');
				return { point, parent: Number(target?.getAttribute('data-dashboard-surface-node-id')) };
			});
			assert(
				destination.parent > 0 && destination.parent !== scene.containerId,
				JSON.stringify(destination)
			);
			assert(await edit({ kind: 'update', point: destination.point }, index));
			assert(
				(await page.locator('.dashboard-drop-preview').innerText()).includes(`Widget ${index}`)
			);
			return destination;
		};
		await setup();
		await start();
		const destination = await update();
		assert(await edit({ kind: 'finish' }));
		let settled = await state();
		assert.equal(moves(settled).length, 1);
		assert.equal(moves(settled)[0].parent, destination.parent);
		assert.equal(settled.operations, 0);
		assert.equal(settled.active, false);
		assert.equal(await page.locator(selector()).count(), 1);
		record(`ordered-${layout}-move-commits-to-free-surface`, { destination });
		await setup();
		await start();
		await update();
		assert(await edit({ kind: 'cancel' }));
		assert.equal(moves(await state()).length, 0);
		assert.equal((await state()).operations, 0);
		assert.equal(await page.locator('.dashboard-drop-preview').count(), 0);
		record(`ordered-${layout}-cancel-never-commits`);
		await setup();
		await page.evaluate(() => window.dashboardFixture.editing(false));
		assert.equal(await edit({ kind: 'move', origin: { x: 100, y: 100 } }), false);
		assert.equal(moves(await state()).length, 0);
		record(`ordered-${layout}-readonly-refuses-move`);
		for (const retirement of [
			'mode',
			'layout',
			'removed',
			'reparented',
			'replaced',
			'destination-layout',
			'destination-order'
		]) {
			await setup();
			await start();
			await update();
			if (retirement === 'mode') await page.evaluate(() => window.dashboardFixture.editing(false));
			if (retirement === 'layout')
				await page.evaluate(
					(layout) =>
						window.dashboardFixture.update(-2, 'layout_kind', {
							kind: 'enum',
							value: layout === 'grid' ? 'horizontal' : 'grid'
						}),
					layout
				);
			if (retirement === 'removed') await page.evaluate(() => window.dashboardFixture.remove(0));
			if (retirement === 'reparented')
				await page.evaluate(() => window.dashboardFixture.reparent(0));
			if (retirement === 'replaced')
				await page.evaluate(() => window.dashboardFixture.refreshIdentities());
			if (retirement === 'destination-layout')
				await page.evaluate(() =>
					window.dashboardFixture.update(-1, 'layout_kind', { kind: 'enum', value: 'grid' })
				);
			if (retirement === 'destination-order')
				await page.evaluate(() => window.dashboardFixture.reorder());
			assert.equal(await edit({ kind: 'finish' }), false);
			assert.equal(moves(await state()).length, 0);
			assert.equal((await state()).operations, 0);
			assert.equal(await page.locator('.dashboard-drop-preview').count(), 0);
			record(`ordered-${layout}-move-retires-${retirement}`);
		}
		await setup({ holdWrites: true });
		await start();
		await update();
		const pending = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		assert.equal(moves(await state()).length, 1);
		await page.evaluate(() => window.dashboardFixture.editing(false));
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
		);
		assert.equal(await page.locator(selector()).count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), pending));
		assert.equal(moves(await state()).length, 1);
		assert.equal((await state()).operations, 0);
		record(`ordered-${layout}-confirmed-move-survives-mode-and-disposal`);
		await setup({ rejectWrites: true });
		await start();
		await update();
		assert(await edit({ kind: 'finish' }));
		assert.equal(moves(await state()).length, 1);
		assert.equal((await state()).operations, 0);
		assert.equal(await page.locator('.dashboard-drop-preview').count(), 0);
		assert.equal(await page.locator(selector()).count(), 1);
		record(`ordered-${layout}-move-rejection-clears-owned-preview`);

		await setup({ holdWrites: true, rejectWrites: true });
		await start();
		await update();
		const older = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		await page.evaluate(
			({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[1]),
			scene
		);
		await start(1);
		await update(1);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), older));
		assert((await page.locator('.dashboard-drop-preview').innerText()).includes('Widget 1'));
		assert(await edit({ kind: 'cancel' }, 1));
		assert.equal((await state()).operations, 0);
		record(`ordered-${layout}-old-rejection-keeps-successor-preview`);

		if (layout === 'horizontal' || layout === 'vertical')
			for (const retired of [false, true]) {
				await setup({ holdWrites: true });
				const box = await page.locator(selector()).boundingBox();
				assert(box);
				const origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
				assert(
					await edit({
						kind: 'resize',
						origin,
						edges: { left: false, top: false, right: true, bottom: true }
					})
				);
				assert(await edit({ kind: 'update', point: { x: origin.x + 24, y: origin.y + 24 } }));
				const resize = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
				await page.evaluate(
					({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[1]),
					scene
				);
				await start(1);
				await update(1);
				const move = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(1));
				assert.equal(moves(await state()).length, 0);
				if (retired) await page.evaluate(() => window.dashboardFixture.refreshIdentities());
				else await page.evaluate(() => window.dashboardFixture.editing(false));
				await page.evaluate(() => window.dashboardFixture.releaseEdits());
				assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), resize));
				if (!retired) {
					const kinds = (await state()).records.map((r) => r.kind);
					assert(
						kinds.indexOf('endEdit') < kinds.indexOf('moveDashboardWidget'),
						JSON.stringify(kinds)
					);
					await page.evaluate(() => window.dashboardFixture.releaseEdits());
				}
				assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), move));
				settled = await state();
				assert.equal(moves(settled).length, retired ? 0 : 1);
				assert.equal(settled.operations, 0);
				assert.equal(settled.active, false);
				record(
					`ordered-${layout}-queued-move-${retired ? 'retires-on-replacement' : 'waits-for-resize-history'}`
				);
			}
	}
	const scene = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 100, 'dense', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'tabs' })
	);
	await page.evaluate(
		({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[0]),
		scene
	);
	const header = `.dashboard-tab-strip > button[data-dashboard-child-id="${scene.ids[80]}"]`;
	await page.locator(header).evaluate((element) => element.focus());
	await page.evaluate(() => window.dashboardFixture.budget(8));
	await page.evaluate(() => window.dashboardFixture.configureEdits());
	const source = await page
		.locator(`.dashboard-widget-shell[data-node-id="${scene.ids[0]}"]`)
		.boundingBox();
	assert(source);
	const target = await page.locator(header).boundingBox();
	assert(target);
	assert(
		await page.evaluate(
			(origin) => window.dashboardFixture.layoutEdit(0, { kind: 'move', origin }),
			{ x: source.x + source.width / 2, y: source.y + source.height / 2 }
		)
	);
	assert(
		await page.evaluate(
			(point) => window.dashboardFixture.layoutEdit(0, { kind: 'update', point }),
			{ x: target.x + target.width / 4, y: target.y + target.height / 2 }
		)
	);
	assert(await page.evaluate(() => window.dashboardFixture.layoutEdit(0, { kind: 'finish' })));
	const moved = (await page.evaluate(() => window.dashboardFixture.editState())).records.filter(
		(r) => r.kind === 'moveDashboardWidget'
	);
	assert.equal(moved.length, 1, JSON.stringify(moved));
	assert.equal(moved[0].target_index, 79, JSON.stringify(moved));
	record('ordered-tabs-inserts-before-real-windowed-child', { targetIndex: moved[0].target_index });
}
