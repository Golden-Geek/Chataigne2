import assert from 'node:assert/strict';

export async function compositionLifetimes(page, record, harness) {
	const { setup, start, update, edit, state, selector, scene, orderedTarget } = harness;
	const writes = (s) =>
		s.records.filter((r) => r.kind === 'setParam' || r.kind === 'resizeDashboardWidget').length;
	const requestFinish = () =>
		page.evaluate(
			(index) => window.dashboardFixture.requestLayoutFinish(index),
			scene().sourceIndex
		);
	const waitFinish = (id) =>
		page.evaluate(async (id) => {
			let timer;
			try {
				return await Promise.race([
					window.dashboardFixture.waitLayoutFinish(id),
					new Promise((_, reject) => {
						timer = setTimeout(
							() => reject(new Error('Layout finish did not settle after its receipts')),
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
		assert.equal(s.records.filter((r) => r.kind === 'beginEdit').length, 1);
		assert.equal(s.records.filter((r) => r.kind === 'endEdit').length, 1);
		return s;
	};
	const viewport = page.viewportSize();
	const changeFrame = async (change) => {
		if (change === 'root-font') await page.evaluate(() => window.dashboardFixture.rootFont(20));
		else if (change === 'local-font')
			await page.evaluate(() => window.dashboardFixture.localFont(20));
		else if (change === 'zoom' || change === 'pan')
			await page.evaluate(
				(change) =>
					window.dashboardFixture.view({
						panX: change === 'pan' ? 48 : 0,
						panY: 0,
						zoom: change === 'zoom' ? 0.8 : 1
					}),
				change
			);
		else if (change === 'viewport')
			await page.setViewportSize({ width: viewport.width - 64, height: viewport.height - 32 });
		else {
			await page
				.locator(`[data-dashboard-surface-node-id="${scene().parent}"]`)
				.evaluate((surface, change) => {
					if (change === 'parent-size') {
						const before = surface.getBoundingClientRect().width;
						surface.style.setProperty('inline-size', `${surface.clientWidth - 64}px`, 'important');
						if (Math.abs(surface.getBoundingClientRect().width - before) < 1)
							throw new Error('Parent-size probe did not change the rendered frame');
					} else {
						const horizontal = surface.scrollWidth > surface.clientWidth;
						const key = horizontal ? 'scrollLeft' : 'scrollTop';
						const before = surface[key];
						surface[key] += 32;
						if (surface[key] === before) throw new Error('Scroll probe needs real overflow');
					}
				}, change);
		}
	};
	for (const family of ['generic', 'node', 'container'])
		for (const [layout, kind] of [
			['free', 'move'],
			['free', 'resize'],
			['horizontal', 'resize'],
			['vertical', 'resize']
		]) {
			const changes = ['root-font', 'local-font', 'parent-size', 'zoom', 'pan', 'viewport'];
			if (layout !== 'free') changes.push('scroll');
			for (const change of changes) {
				await setup(family, layout, 'free', { holdWrites: true });
				if (change === 'scroll')
					await page
						.locator(`[data-dashboard-surface-node-id="${scene().parent}"]`)
						.evaluate((surface) => {
							surface.style.setProperty('inline-size', `${surface.clientWidth / 2}px`, 'important');
							surface.style.setProperty('block-size', `${surface.clientHeight / 2}px`, 'important');
						});
				const { origin } = await start(kind);
				for (const delta of [24, 48]) assert(await update(origin, delta));
				const admitted = writes(await state());
				assert(admitted > 0);
				await changeFrame(change);
				const finish = await requestFinish();
				await release();
				assert.equal(
					writes(await state()),
					admitted,
					JSON.stringify({ family, layout, kind, change, s: await state() })
				);
				await waitFinish(finish);
				const s = await closed();
				assert.equal(writes(s), admitted, JSON.stringify({ family, layout, kind, change, s }));
				assert.equal(await update(origin, 72), false);
				record(`${family}-${layout}-${kind}-retires-${change}-tail`, { admitted });
				if (change === 'viewport') await page.setViewportSize(viewport);
			}

			await setup(family, layout, 'free', { holdWrites: true });
			let { origin } = await start(kind);
			for (const delta of [24, 48]) assert(await update(origin, delta));
			const admitted = writes(await state());
			const finish = await requestFinish();
			await page.evaluate(() => window.dashboardFixture.mountCanvas(false));
			await page.evaluate(() =>
				window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
			);
			assert.equal(await page.locator(selector()).count(), 0);
			await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
			await page.evaluate(() => window.dashboardFixture.mountCanvas(true));
			await release();
			assert.equal(writes(await state()), admitted * 2);
			await release();
			assert(await waitFinish(finish));
			await closed();
			record(`${family}-${layout}-${kind}-confirmed-tail-survives-disposal`, { admitted });

			await setup(family, layout, 'free', { rejectWrites: true });
			const before = await page.locator(selector()).boundingBox();
			({ origin } = await start(kind));
			assert(await update(origin));
			await edit({ kind: 'finish' });
			await closed();
			const after = await page.locator(selector()).boundingBox();
			assert(before && after);
			const error = Math.max(
				...['x', 'y', 'width', 'height'].map((key) => Math.abs(before[key] - after[key]))
			);
			assert(error <= 0.5, JSON.stringify({ family, layout, kind, before, after, error }));
			record(`${family}-${layout}-${kind}-rejection-withdraws-draft`, { error });
		}
	for (const family of ['node', 'container'])
		for (const layout of ['horizontal', 'vertical', 'grid', 'tabs', 'accordion'])
			for (const rejected of [false, true]) {
				await setup(family, layout, 'free', { holdWrites: true, rejectWrites: rejected });
				await start('move');
				const { point, expected } = await orderedTarget(family, layout);
				assert(await edit({ kind: 'update', point }));
				const finish = await requestFinish();
				const s = await state();
				assert.equal(s.records.filter((r) => r.kind === 'moveDashboardWidget').length, 1);
				assert.equal(s.pending, 1);
				await page.evaluate(() => window.dashboardFixture.mountCanvas(false));
				assert.equal(await page.locator(selector()).count(), 0);
				await release();
				assert(await waitFinish(finish));
				assert.equal((await state()).operations, 0);
				assert.equal((await state()).active, false);
				await page.evaluate(() => window.dashboardFixture.mountCanvas(true));
				const parent = rejected ? scene().parent : expected;
				await page.evaluate(
					({ parent, source }) => window.dashboardFixture.chooseChild(parent, source),
					{ parent, source: scene().source }
				);
				assert.equal(await page.locator(selector()).count(), 1);
				assert.equal(
					await page
						.locator(selector())
						.evaluate((shell) =>
							Number(
								shell.parentElement.closest('[data-dashboard-surface-node-id]').dataset
									.dashboardSurfaceNodeId
							)
						),
					parent
				);
				record(
					`${family}-${layout}-move-${rejected ? 'rejection' : 'confirmation'}-survives-disposal`
				);
			}
	await page.evaluate(async () => {
		await window.dashboardFixture.rootFont(16);
		await window.dashboardFixture.localFont(16);
	});
}
