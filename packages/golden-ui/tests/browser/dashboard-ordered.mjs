import assert from 'node:assert/strict';

export async function orderedChecks(page, record) {
	for (const layout of ['horizontal', 'vertical']) {
		let scene, origin;
		const axis = layout === 'horizontal' ? 'width' : 'height';
		const selector = (index = 0) => `.dashboard-widget-shell[data-node-id="${scene.ids[index]}"]`;
		const state = () => page.evaluate(() => window.dashboardFixture.editState());
		const writes = (s) => s.records.filter((r) => r.kind === 'resizeDashboardWidget').length;
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
				async ({ layout, axis }) => {
					await window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout });
					await window.dashboardFixture.nestLayoutFields(0);
					await window.dashboardFixture.enabled(0, axis, false);
				},
				{ layout, axis }
			);
			await page.evaluate(
				({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[0]),
				scene
			);
			await page.evaluate((options) => window.dashboardFixture.configureEdits(options), options);
		};
		const start = async (index = 0, leading = false) => {
			const box = await page.locator(selector(index)).boundingBox();
			assert(box);
			origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
			assert(
				await edit(
					{
						kind: 'resize',
						origin,
						edges: { left: leading, top: leading, right: !leading, bottom: !leading }
					},
					index
				)
			);
			return box;
		};
		const update = (delta = 24, index = 0) =>
			edit({ kind: 'update', point: { x: origin.x + delta, y: origin.y + delta } }, index);
		const closed = async (ends = 1) => {
			const s = await state();
			assert.equal(s.active, false);
			assert.equal(s.pending, 0);
			assert.equal(s.records.filter((r) => r.kind === 'endEdit').length, ends, JSON.stringify(s));
			return s;
		};
		const sameBox = async (expected) => {
			const actual = await page.locator(selector()).boundingBox();
			assert(actual);
			const error = Math.max(
				...['x', 'y', 'width', 'height'].map((key) => Math.abs(actual[key] - expected[key]))
			);
			assert(error <= 0.5, JSON.stringify({ layout, expected, actual, error }));
			return error;
		};
		await setup();
		const before = await start();
		assert(await update());
		assert(await edit({ kind: 'finish' }));
		const after = await page.locator(selector()).boundingBox();
		assert(after);
		const delta = after[axis] - before[axis];
		assert(Math.abs(delta - 24) <= 0.5, JSON.stringify({ layout, before, after, delta }));
		assert.equal(writes(await closed()), 1);
		record(`ordered-${layout}-resize-follows-pointer-size`, { delta });

		await setup();
		const box = await page.locator(selector()).boundingBox();
		assert(box);
		origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		const begin = {
			kind: 'resize',
			origin,
			edges: { left: false, top: false, right: true, bottom: true }
		};
		assert.deepEqual(
			await page.evaluate(
				(commands) => window.dashboardFixture.layoutEdits(0, commands),
				[
					begin,
					{ kind: 'update', point: { x: origin.x + 24, y: origin.y + 24 } },
					{ kind: 'cancel' }
				]
			),
			[true, true, true]
		);
		assert.equal(writes(await closed()), 0);
		await sameBox(box);
		record(`ordered-${layout}-cancel-drops-unsent-size`);
		await setup();
		await start();
		assert(
			await page.evaluate(
				(commands) => window.dashboardFixture.layoutEdits(0, commands),
				[
					{ kind: 'update', point: { x: origin.x + 24, y: origin.y + 24 } },
					{ kind: 'update', point: { x: origin.x + 48, y: origin.y + 48 } },
					{ kind: 'update', point: { x: origin.x + 72, y: origin.y + 72 } },
					{ kind: 'finish' }
				]
			)
		);
		assert.equal(writes(await closed()), 1);
		record(`ordered-${layout}-coalesces-final-samples`);

		await setup({ holdBegin: true });
		await start();
		assert(await update());
		assert.equal(writes(await state()), 0);
		await page.evaluate(() => window.dashboardFixture.editing(false));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert.equal(writes(await closed()), 0);
		record(`ordered-${layout}-closes-late-begin-on-mode-exit`);
		for (const retirement of ['mode', 'layout', 'removed', 'reparented', 'replaced']) {
			await setup({ holdWrites: true });
			await start();
			assert(await update());
			const admitted = writes(await state());
			assert.equal(admitted, 1);
			assert(await update(48));
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
			assert.equal(writes(await closed(retirement === 'replaced' ? 0 : 1)), admitted);
			assert.equal(await update(72), false);
			record(`ordered-${layout}-retires-${retirement}`, { admitted });
		}
		await setup({ holdWrites: true });
		await start();
		await update();
		await update(48);
		const requested = await page.locator(selector()).boundingBox();
		assert(requested);
		const finish = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		const finishError = await sameBox(requested);
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
		);
		assert.equal(await page.locator(selector()).count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		const remountError = await sameBox(requested);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert.equal(writes(await state()), 2);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), finish));
		await closed();
		record(`ordered-${layout}-confirmed-size-survives-disposal`, { finishError, remountError });

		await setup({ holdEvents: true });
		const raw = await page.evaluate((axis) => window.dashboardFixture.paramValue(0, axis), axis);
		await start();
		await update(48);
		const draft = await page.locator(selector()).boundingBox();
		assert(draft);
		assert(await edit({ kind: 'finish' }));
		const pendingError = await sameBox(draft);
		assert.deepEqual(
			await page.evaluate((axis) => window.dashboardFixture.paramValue(0, axis), axis),
			raw
		);
		assert.equal((await state()).operations, 1);
		await page.evaluate(() => window.dashboardFixture.releaseEditEvents());
		assert.equal((await state()).operations, 0);
		const appliedError = await sameBox(draft);
		record(`ordered-${layout}-size-and-enablement-wait-for-events`, { pendingError, appliedError });
		await setup({ rejectWrites: true });
		const rejectedBox = await start();
		await update();
		assert(await edit({ kind: 'finish' }));
		await closed();
		await sameBox(rejectedBox);
		assert.equal((await state()).operations, 0);
		record(`ordered-${layout}-rejection-withdraws-size-draft`);

		await setup({ holdWrites: true });
		await page.evaluate(() => window.dashboardFixture.focus(0));
		await start();
		await update();
		const first = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		await page.evaluate(
			({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[1]),
			scene
		);
		await start(1);
		await update(24, 1);
		assert.equal(writes(await state()), 1);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), first));
		const kinds = (await state()).records.map((r) => r.kind);
		const begins = kinds.flatMap((kind, index) => (kind === 'beginEdit' ? [index] : []));
		assert.equal(begins.length, 2);
		assert(kinds.indexOf('endEdit') < begins[1], JSON.stringify(kinds));
		const second = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(1));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), second));
		await closed(2);
		record(`ordered-${layout}-serializes-widget-history`, { kinds });

		for (const unit of ['rem', 'em', 'percent', 'vw', 'vh']) {
			await setup();
			await page.evaluate(
				async ({ axis, unit }) => {
					await window.dashboardFixture.rootFont(19);
					await window.dashboardFixture.localFont(21);
					await window.dashboardFixture.update(0, axis, {
						kind: 'css_value',
						value: unit === 'percent' ? 25 : unit === 'vw' || unit === 'vh' ? 10 : 12,
						unit
					});
					await window.dashboardFixture.enabled(0, axis, true);
				},
				{ axis, unit }
			);
			const before = await start(0, true);
			await update(-24);
			assert(await edit({ kind: 'finish' }));
			const after = await page.locator(selector()).boundingBox();
			assert(after);
			const delta = after[axis] - before[axis];
			assert(Math.abs(delta - 24) <= 0.5, JSON.stringify({ layout, unit, before, after, delta }));
			assert.equal(
				(await page.evaluate((axis) => window.dashboardFixture.paramValue(0, axis), axis)).unit,
				unit
			);
			record(`ordered-${layout}-leading-resize-preserves-${unit}`, { delta });
			await page.evaluate(async () => {
				await window.dashboardFixture.rootFont(16);
				await window.dashboardFixture.localFont(16);
			});
		}
	}
}
