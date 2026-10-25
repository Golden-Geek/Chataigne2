import assert from 'node:assert/strict';

export async function barrierChecks(page, record) {
	let scene;
	const selector = () => `.dashboard-widget-shell[data-node-id="${scene.source}"]`;
	const state = () => page.evaluate(() => window.dashboardFixture.editState());
	const barriers = () => page.evaluate(() => window.dashboardFixture.barrierState());
	const writes = (s) =>
		s.records.filter((r) =>
			['setParam', 'resizeDashboardWidget', 'moveDashboardWidget'].includes(r.kind)
		).length;
	const setup = async (family, layout, options = {}) => {
		scene = await page.evaluate(
			({ family, layout }) => window.dashboardFixture.setupComposition(family, layout),
			{ family, layout }
		);
		await page.evaluate((options) => {
			window.dashboardFixture.configureEdits({ holdWrites: true });
			window.dashboardFixture.configureBarriers(options);
		}, options);
	};
	const edit = (command) =>
		page.evaluate(({ index, command }) => window.dashboardFixture.layoutEdit(index, command), {
			index: scene.sourceIndex,
			command
		});
	const begin = async (kind) => {
		const box = await page.locator(selector()).boundingBox();
		assert(box);
		const origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		assert(
			await edit(
				kind === 'move'
					? { kind, origin }
					: { kind, origin, edges: { left: false, top: false, right: true, bottom: true } }
			)
		);
		return origin;
	};
	const update = (origin, delta) =>
		edit({ kind: 'update', point: { x: origin.x + delta, y: origin.y + delta } });
	const finish = () =>
		page.evaluate((index) => window.dashboardFixture.requestLayoutFinish(index), scene.sourceIndex);
	const request = (kind, parent = scene.destination) =>
		page.evaluate(
			({ kind, index, parent }) =>
				window.dashboardFixture.requestSessionBarrier(kind, index, parent),
			{ kind, index: scene.sourceIndex, parent }
		);
	const wait = (id) =>
		page.evaluate(async (id) => {
			let timer;
			try {
				return await Promise.race([
					window.dashboardFixture.waitSessionBarrier(id),
					new Promise((_, reject) => {
						timer = setTimeout(
							() => reject(new Error('Session barrier did not settle after its receipts')),
							5000
						);
					})
				]);
			} finally {
				clearTimeout(timer);
			}
		}, id);
	const release = () => page.evaluate(() => window.dashboardFixture.releaseEdits());
	const closed = async () => {
		const s = await state();
		assert.equal(s.active, false, JSON.stringify(s));
		assert.equal(s.pending, 0);
		assert.equal(s.operations, 0);
		return s;
	};
	for (const family of ['generic', 'node', 'container'])
		for (const [layout, kind] of [
			['free', 'move'],
			['free', 'resize'],
			['horizontal', 'resize'],
			['vertical', 'resize']
		])
			for (const confirmed of [false, true])
				for (const barrier of ['save', 'undo', 'project', 'reparent']) {
					await setup(family, layout);
					const origin = await begin(kind);
					for (const delta of [24, 48]) assert(await update(origin, delta));
					const admitted = writes(await state());
					assert(admitted > 0);
					const finished = confirmed ? await finish() : null;
					const requested = await request(barrier);
					assert.equal((await barriers()).calls.length, 0);
					assert.equal((await state()).records.filter((r) => r.kind === 'undo').length, 0);
					await release();
					if (confirmed) {
						assert.equal(
							writes(await state()),
							admitted * 2,
							JSON.stringify({ family, layout, kind, barrier, s: await state() })
						);
						assert.equal((await barriers()).calls.length, 0);
						await release();
					} else assert.equal(writes(await state()), admitted);
					const result = await wait(requested);
					if (finished !== null)
						assert(
							await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), finished)
						);
					const s = await closed();
					if (barrier === 'undo') {
						const kinds = s.records.map((r) => r.kind);
						assert(kinds.indexOf('endEdit') < kinds.indexOf('undo'), JSON.stringify(kinds));
						const redo = await request('redo');
						await wait(redo);
						assert.equal((await state()).records.at(-1).kind, 'redo');
					} else {
						assert.equal((await barriers()).calls.length, 1);
						assert.equal((await barriers()).calls[0].active, false);
						assert.equal((await barriers()).calls[0].history, 1);
						if (barrier === 'save') {
							assert.equal(result.ok, true);
							assert.equal((await barriers()).savedHistoryId, 1);
						}
					}
					record(
						`${family}-${layout}-${kind}-${confirmed ? 'confirmed' : 'active'}-${barrier}-barrier`,
						{ admitted }
					);
				}

	for (const family of ['generic', 'node', 'container'])
		for (const barrier of ['save', 'undo', 'project', 'reparent']) {
			await setup(family, 'horizontal');
			await begin('move');
			const point = await page
				.locator(`[data-dashboard-surface-node-id="${scene.destination}"]`)
				.evaluate((surface) => {
					const r = surface.getBoundingClientRect();
					return { x: r.left + 32, y: r.top + 32 };
				});
			assert(await edit({ kind: 'update', point }));
			const finished = await finish();
			assert.equal(writes(await state()), 1);
			const requested = await request(barrier, scene.pageId);
			assert.equal((await barriers()).calls.length, 0);
			await release();
			await wait(requested);
			assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), finished));
			assert.equal(writes(await closed()), 1);
			record(`${family}-ordered-move-${barrier}-barrier`);
		}

	await setup('generic', 'free', { holdSave: true });
	let origin = await begin('resize');
	assert(await update(origin, 24));
	await finish();
	let saved = await request('save');
	await release();
	assert.equal((await barriers()).held, 1);
	await page.evaluate(() => window.dashboardFixture.configureEdits());
	await page.evaluate(() =>
		window.dashboardFixture.sendParam(0, 'width', { kind: 'css_value', value: 20, unit: 'rem' })
	);
	assert.equal((await barriers()).history, 2);
	await page.evaluate(() => window.dashboardFixture.releaseSave());
	assert.equal((await wait(saved)).ok, true);
	assert.equal((await barriers()).savedHistoryId, 1);
	record('save-retains-unsaved-later-history');

	await setup('generic', 'free', { rejectSave: true });
	origin = await begin('resize');
	assert(await update(origin, 24));
	await finish();
	saved = await request('save');
	await release();
	assert.equal((await wait(saved)).ok, false);
	assert.equal((await barriers()).savedHistoryId, 0);
	assert.equal((await barriers()).busy, false);
	await closed();
	assert(
		await edit({
			kind: 'resize',
			origin,
			edges: { left: false, top: false, right: true, bottom: true }
		})
	);
	await edit({ kind: 'cancel' });
	record('failed-save-releases-admission-without-marking-clean');

	await setup('generic', 'free');
	origin = await begin('resize');
	assert(await update(origin, 24));
	await finish();
	await page.evaluate(() => window.dashboardFixture.rejectSnapshot());
	const failedProject = await request('project');
	await release();
	await assert.rejects(wait(failedProject), /snapshot failed/);
	await assert.rejects(
		page.evaluate(() =>
			window.dashboardFixture.sendParam(0, 'width', { kind: 'css_value', value: 20, unit: 'rem' })
		),
		/not synchronized/
	);
	assert(await page.evaluate(() => window.dashboardFixture.recoverSnapshot()));
	await page.evaluate(() => window.dashboardFixture.configureEdits());
	await page.evaluate(() =>
		window.dashboardFixture.sendParam(0, 'width', { kind: 'css_value', value: 20, unit: 'rem' })
	);
	await closed();
	record('failed-project-retains-gate-until-snapshot-recovery');

	await setup('generic', 'free');
	origin = await begin('resize');
	for (const delta of [24, 48]) assert(await update(origin, delta));
	const admitted = writes(await state());
	await finish();
	saved = await request('save');
	await page.evaluate(() => window.dashboardFixture.replaceHost());
	await release();
	assert.equal((await wait(saved)).ok, false);
	assert.equal((await barriers()).calls.length, 0);
	assert.equal(writes(await state()), admitted);
	assert.deepEqual(await page.evaluate(() => window.dashboardFixture.replacementState()), {
		intents: [],
		active: false
	});
	await page.evaluate(() => window.dashboardFixture.restoreHost());
	await closed();
	record('host-retirement-cancels-old-barrier-without-new-host-dispatch');

	await setup('generic', 'free');
	origin = await begin('resize');
	for (const delta of [24, 48]) assert(await update(origin, delta));
	const first = await finish();
	await page.evaluate(
		({ parent, ids }) => window.dashboardFixture.chooseChild(parent, ids[1]),
		scene
	);
	const secondBox = await page
		.locator(`.dashboard-widget-shell[data-node-id="${scene.ids[1]}"]`)
		.boundingBox();
	assert(secondBox);
	const secondOrigin = {
		x: secondBox.x + secondBox.width / 2,
		y: secondBox.y + secondBox.height / 2
	};
	assert(
		await page.evaluate(
			(origin) =>
				window.dashboardFixture.layoutEdit(1, {
					kind: 'resize',
					origin,
					edges: { left: false, top: false, right: true, bottom: true }
				}),
			secondOrigin
		)
	);
	for (const delta of [24, 48])
		assert(
			await page.evaluate(
				(point) => window.dashboardFixture.layoutEdit(1, { kind: 'update', point }),
				{ x: secondOrigin.x + delta, y: secondOrigin.y + delta }
			)
		);
	const second = await page.evaluate(() => window.dashboardFixture.requestLayoutFinish(1));
	saved = await request('save');
	assert.equal((await state()).records.filter((r) => r.kind === 'beginEdit').length, 1);
	await release();
	await release();
	assert.equal((await barriers()).calls.length, 0);
	assert.equal((await state()).records.filter((r) => r.kind === 'beginEdit').length, 2);
	await release();
	assert.equal((await wait(saved)).ok, true);
	assert.equal((await barriers()).calls[0].history, 2);
	assert.equal((await state()).records.filter((r) => r.kind === 'endEdit').length, 2);
	for (const id of [first, second])
		assert(await page.evaluate((id) => window.dashboardFixture.waitLayoutFinish(id), id));
	await closed();
	record('save-drains-both-confirmed-widget-history-owners');

	await setup('generic', 'free');
	await page.evaluate(() => window.dashboardFixture.configureEdits({ holdEvents: true }));
	origin = await begin('resize');
	assert(await update(origin, 48));
	await edit({ kind: 'finish' });
	assert.equal((await state()).operations, 1);
	saved = await request('save');
	assert.equal((await wait(saved)).ok, true);
	assert.equal((await state()).operations, 1);
	await page.evaluate(() => window.dashboardFixture.releaseEditEvents());
	await closed();
	record('save-does-not-wait-for-presentation-only-publication');
}
