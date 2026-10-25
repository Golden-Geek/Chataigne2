import assert from 'node:assert/strict';

export async function confirmedChecks(page, record) {
	for (const mode of ['move', 'resize']) {
		const scene = await page.evaluate(() =>
			window.dashboardFixture.setup('free', true, 600, 'dense', true)
		);
		await page.evaluate(
			({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[0]),
			scene
		);
		await page.evaluate(() => window.dashboardFixture.configureEdits({ holdWrites: true }));
		const selector = `.dashboard-widget-shell[data-node-id="${scene.ids[0]}"]`;
		const first = await page.locator(selector).boundingBox();
		assert(first);
		const origin = { x: first.x + first.width / 2, y: first.y + first.height / 2 };
		const command =
			mode === 'move'
				? { kind: 'move', origin }
				: { kind: 'resize', origin, edges: { left: false, top: false, right: true, bottom: true } };
		assert(
			await page.evaluate((command) => window.dashboardFixture.layoutEdit(0, command), command)
		);
		for (const delta of [24, 48])
			assert(
				await page.evaluate(
					(point) => window.dashboardFixture.layoutEdit(0, { kind: 'update', point }),
					{ x: origin.x + delta, y: origin.y + delta }
				)
			);
		const requested = await page.locator(selector).boundingBox();
		assert(requested);
		const finish = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		const waiting = await page.locator(selector).boundingBox();
		assert(waiting);
		const error = Math.max(
			...['x', 'y', 'width', 'height'].map((key) => Math.abs(requested[key] - waiting[key]))
		);
		assert(error <= 0.5, JSON.stringify({ mode, requested, waiting, error }));
		record(`confirmed-${mode}-retains-final-presentation`, { error });
		const admitted = (
			await page.evaluate(() => window.dashboardFixture.editState())
		).records.filter((r) => r.kind === 'setParam').length;
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
		);
		assert.equal(await page.locator(selector).count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		const restored = await page.locator(selector).boundingBox();
		assert(restored);
		const restoredError = Math.max(
			...['x', 'y', 'width', 'height'].map((key) => Math.abs(requested[key] - restored[key]))
		);
		assert(restoredError <= 0.5, JSON.stringify({ mode, requested, restored, restoredError }));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		const state = await page.evaluate(() => window.dashboardFixture.editState());
		assert.equal(
			state.records.filter((r) => r.kind === 'setParam').length,
			admitted * 2,
			`${mode}: confirmed tail must survive disposal`
		);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), finish));
		assert.equal((await page.evaluate(() => window.dashboardFixture.editState())).active, false);
		record(`confirmed-${mode}-survives-view-disposal`, { admitted, restoredError });
	}
	for (const mode of ['move', 'resize']) {
		let scene;
		const selector = (index = 0) => `.dashboard-widget-shell[data-node-id="${scene.ids[index]}"]`;
		const state = () => page.evaluate(() => window.dashboardFixture.editState());
		const writes = (value) => value.records.filter((r) => r.kind === 'setParam').length;
		const setup = async (options) => {
			scene = await page.evaluate(() =>
				window.dashboardFixture.setup('free', true, 600, 'dense', true)
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
			const origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
			const command =
				mode === 'move'
					? { kind: 'move', origin }
					: {
							kind: 'resize',
							origin,
							edges: { left: false, top: false, right: true, bottom: true }
						};
			assert(
				await page.evaluate(
					({ index, command }) => window.dashboardFixture.layoutEdit(index, command),
					{ index, command }
				)
			);
			return origin;
		};
		const update = async (origin, delta, index = 0) => {
			assert(
				await page.evaluate(
					({ index, point }) =>
						window.dashboardFixture.layoutEdit(index, { kind: 'update', point }),
					{ index, point: { x: origin.x + delta, y: origin.y + delta } }
				)
			);
		};
		const sameBox = async (expected) => {
			const actual = await page.locator(selector()).boundingBox();
			assert(actual);
			const error = Math.max(
				...['x', 'y', 'width', 'height'].map((key) => Math.abs(actual[key] - expected[key]))
			);
			assert(error <= 0.5, JSON.stringify({ mode, expected, actual, error }));
			return error;
		};

		await setup({ holdWrites: true });
		await page.evaluate(() => window.dashboardFixture.focus(0));
		assert(
			await page.locator(selector()).evaluate((shell) => shell.contains(document.activeElement))
		);
		let origin = await start();
		await update(origin, 24);
		const admitted = writes(await state());
		const first = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
		await page.evaluate(
			({ containerId, ids }) => window.dashboardFixture.chooseChild(containerId, ids[1]),
			scene
		);
		assert.equal(await page.locator(selector()).count(), 1);
		origin = await start(1);
		await update(origin, 24, 1);
		assert.equal(
			writes(await state()),
			admitted,
			'second widget must wait for the first history group'
		);
		assert.equal((await state()).records.filter((r) => r.kind === 'beginEdit').length, 1);
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), first));
		const ordered = (await state()).records.map((r) => r.kind);
		const begins = ordered.flatMap((kind, index) => (kind === 'beginEdit' ? [index] : []));
		assert.equal(begins.length, 2, JSON.stringify(ordered));
		assert(ordered.indexOf('endEdit') < begins[1], JSON.stringify(ordered));
		const second = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(1));
		await page.evaluate(() => window.dashboardFixture.releaseEdits());
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), second));
		assert.equal((await state()).active, false);
		assert.equal((await state()).records.filter((r) => r.kind === 'endEdit').length, 2);
		record(`confirmed-${mode}-serializes-independent-widget-history`, { admitted, ordered });

		await setup({ holdEvents: true });
		const key = mode === 'move' ? 'position' : 'width';
		const rawBefore = await page.evaluate((key) => window.dashboardFixture.paramValue(0, key), key);
		origin = await start();
		await update(origin, 48);
		const requested = await page.locator(selector()).boundingBox();
		assert(requested);
		assert(await page.evaluate(() => window.dashboardFixture.layoutEdit(0, { kind: 'finish' })));
		const finalError = await sameBox(requested);
		assert.deepEqual(
			await page.evaluate((key) => window.dashboardFixture.paramValue(0, key), key),
			rawBefore
		);
		assert.equal((await state()).operations, 1);
		assert((await state()).events > 0);
		assert((await page.evaluate(() => window.dashboardFixture.command('view.frame'))).handled);
		const center = await page.locator(selector()).evaluate((shell) => {
			const box = shell.getBoundingClientRect(),
				viewport = shell.closest('.dashboard-page-viewport').getBoundingClientRect();
			return {
				dx: Math.abs(box.x + box.width / 2 - viewport.x - viewport.width / 2),
				dy: Math.abs(box.y + box.height / 2 - viewport.y - viewport.height / 2)
			};
		});
		assert(center.dx <= 0.5 && center.dy <= 0.5, JSON.stringify({ mode, center }));
		record(`confirmed-${mode}-frames-projected-geometry`, center);
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
		);
		assert.equal(await page.locator(selector()).count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		const restoredError = await sameBox(requested);
		await page.evaluate(() => window.dashboardFixture.releaseEditEvents());
		assert.equal((await state()).operations, 0);
		assert.notDeepEqual(
			await page.evaluate((key) => window.dashboardFixture.paramValue(0, key), key),
			rawBefore
		);
		const catchupError = await sameBox(requested);
		record(`confirmed-${mode}-retains-through-late-events`, {
			finalError,
			restoredError,
			catchupError
		});

		for (const retirement of ['mode', 'removed']) {
			await setup({ holdWrites: true });
			origin = await start();
			await update(origin, 24);
			const prefix = writes(await state());
			await update(origin, 48);
			const finish = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(0));
			if (retirement === 'mode') await page.evaluate(() => window.dashboardFixture.editing(false));
			else await page.evaluate(() => window.dashboardFixture.remove(0));
			await page.evaluate(() => window.dashboardFixture.releaseEdits());
			assert.equal(writes(await state()), prefix * (retirement === 'mode' ? 2 : 1));
			if (retirement === 'mode') await page.evaluate(() => window.dashboardFixture.releaseEdits());
			assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), finish));
			assert.equal((await state()).active, false);
			assert.equal((await state()).operations, 0);
			record(
				`confirmed-${mode}-${retirement === 'mode' ? 'survives-mode-exit' : 'cancels-retired-source'}`,
				{ prefix }
			);
		}
	}
}
