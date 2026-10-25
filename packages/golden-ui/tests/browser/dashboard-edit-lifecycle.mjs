import assert from 'node:assert/strict';

export async function editLifecycleChecks(page, record) {
	for (const layout of ['free', 'horizontal', 'vertical', 'grid', 'tabs', 'accordion']) {
		const setup = async () => {
			const scene = await page.evaluate(() =>
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
			return scene;
		};
		const previews = () => page.locator('.dashboard-drop-preview');
		const preview = (commit = false) =>
			page.evaluate(({ index, commit }) => window.dashboardFixture.movePreview(0, index, commit), {
				index: layout === 'free' ? null : 3,
				commit
			});
		const initial = await setup();
		const source = await page
			.locator(`.dashboard-widget-shell[data-node-id="${initial.ids[0]}"]`)
			.elementHandle();
		assert(source);
		try {
			await preview();
			assert.equal(await previews().count(), 1, layout);
			assert((await previews().innerText()).includes('Widget 0'));
			assert(
				await source.evaluate((node) => node.isConnected),
				`${layout}: keep active control instance`
			);
			await page.evaluate(() => window.dashboardFixture.budget(0));
			assert.equal(await previews().count(), 1, layout);
			await page.evaluate(() => window.dashboardFixture.budget(512));
			assert.equal(await previews().count(), 1, layout);
			assert(
				await source.evaluate((node) => node.isConnected),
				`${layout}: keep source through budget changes`
			);
			await page.evaluate(() => window.dashboardFixture.movePreview(null));
			assert.equal(await previews().count(), 0, `${layout} cancellation`);
			assert.equal(await page.locator('.dashboard-hidden-drag-source').count(), 0);
			record(`move-preview-budget-and-cancel-${layout}`);
		} finally {
			await source.dispose();
		}

		await setup();
		await preview();
		await page.evaluate(() => window.dashboardFixture.remove(0));
		assert.equal(await previews().count(), 0, `${layout}: removing source must retire preview`);
		record(`move-preview-source-removal-${layout}`);

		await setup();
		await preview();
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', {
					kind: 'enum',
					value: layout === 'free' ? 'horizontal' : 'free'
				}),
			layout
		);
		assert.equal(await previews().count(), 0, `${layout}: changing layout must retire preview`);
		record(`move-preview-layout-change-${layout}`);

		await setup();
		await preview();
		await page.evaluate(() => window.dashboardFixture.editing(false));
		assert.equal(await previews().count(), 0, `${layout}: leaving edit mode`);
		await preview();
		assert.equal(await previews().count(), 0, `${layout}: run mode ignores preview`);
		await page.evaluate(() => window.dashboardFixture.editing(true));
		assert.equal(await previews().count(), 0, `${layout}: retired preview must not return`);
		record(`move-preview-mode-change-${layout}`);

		await setup();
		await preview();
		await page.evaluate(() => window.dashboardFixture.refreshIdentities());
		assert.equal(await previews().count(), 0, `${layout}: replacement at same ids`);
		record(`move-preview-identity-replacement-${layout}`);

		await setup();
		await preview();
		await page.evaluate(() => window.dashboardFixture.reparent(0));
		assert.equal(await previews().count(), 0, `${layout}: reparenting retires old path`);
		record(`move-preview-reparenting-${layout}`);

		const scene = await setup();
		await preview(true);
		assert.equal(await previews().count(), 1, `${layout}: pending commit`);
		const order = await page.evaluate(
			(index) => window.dashboardFixture.acknowledgeMove(0, index),
			layout === 'free' ? 599 : 3
		);
		assert.deepEqual(
			order,
			layout === 'free'
				? scene.ids.slice(1, 6)
				: [scene.ids[1], scene.ids[2], scene.ids[3], scene.ids[0], scene.ids[4]]
		);
		assert.equal(await previews().count(), 0, `${layout}: acknowledged commit`);
		assert.equal(await page.locator('.dashboard-hidden-drag-source').count(), 0);
		record(`move-commit-acknowledgement-${layout}`);

		await setup();
		await preview(true);
		assert.equal(await previews().count(), 1);
		await page.evaluate(() => window.dashboardFixture.movePreview(null));
		assert.equal(await previews().count(), 0, `${layout}: rejected commit`);
		record(`move-commit-rejection-${layout}`);

		await setup();
		await preview(true);
		await preview(true);
		await page.evaluate(() => window.dashboardFixture.cancelMoveCommit(0));
		assert.equal(
			await previews().count(),
			1,
			`${layout}: stale rejection cannot cancel newer commit`
		);
		await page.evaluate(() => window.dashboardFixture.cancelMoveCommit(1));
		assert.equal(await previews().count(), 0);
		record(`move-commit-stale-rejection-${layout}`);

		if (layout === 'horizontal') {
			await setup();
			await preview(true);
			assert.equal(await previews().count(), 1);
			await page.waitForFunction(
				() => document.querySelector('.dashboard-drop-preview') === null,
				null,
				{ timeout: 3500 }
			);
			assert.equal(await page.locator('.dashboard-hidden-drag-source').count(), 0);
			record('move-commit-expiry');
		}
	}
}
