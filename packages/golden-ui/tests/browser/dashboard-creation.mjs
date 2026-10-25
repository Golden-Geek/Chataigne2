import assert from 'node:assert/strict';

export async function creationChecks(page, record) {
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
			return scene;
		};
		const start = async (index = 0) => {
			const { request } = await page.evaluate(
				(index) => window.dashboardFixture.startCreation(index),
				index
			);
			assert.notEqual(request, null, `${layout}: creation must reach transport`);
			return request;
		};
		const resolve = (request, success = false) =>
			page.evaluate(
				({ request, success }) => window.dashboardFixture.resolveCreation(request, success),
				{ request, success }
			);
		const preview = () => page.locator('.dashboard-drop-preview');

		await setup();
		const first = await start(),
			second = await start(1);
		assert.equal(await preview().count(), 1);
		assert((await preview().innerText()).includes('Create 1'));
		await resolve(first);
		assert.equal(
			await preview().count(),
			1,
			`${layout}: stale rejection must preserve newer creation`
		);
		await resolve(second);
		assert.equal(await preview().count(), 0);
		record(`creation-overlapping-rejections-${layout}`);

		await setup();
		const removed = await start();
		await page.evaluate(() => window.dashboardFixture.remove(0));
		assert.equal(await preview().count(), 0, `${layout}: removed source`);
		await resolve(removed);
		assert.equal(await preview().count(), 0);
		record(`creation-source-removal-${layout}`);

		await setup();
		const replaced = await start();
		await page.evaluate(() => window.dashboardFixture.refreshIdentities());
		assert.equal(await preview().count(), 0, `${layout}: source/owner replacement`);
		await resolve(replaced);
		record(`creation-identity-replacement-${layout}`);

		await setup();
		const changed = await start();
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', {
					kind: 'enum',
					value: layout === 'free' ? 'horizontal' : 'free'
				}),
			layout
		);
		assert.equal(await preview().count(), 0, `${layout}: changed destination layout`);
		await resolve(changed);
		record(`creation-layout-change-${layout}`);

		await setup();
		const stopped = await start();
		await page.evaluate(() => window.dashboardFixture.editing(false));
		assert.equal(await preview().count(), 0, `${layout}: leaving edit mode`);
		assert.equal(
			(await page.evaluate(() => window.dashboardFixture.startCreation(1))).request,
			null
		);
		await page.evaluate(() => window.dashboardFixture.editing(true));
		const resumed = await start(1);
		await resolve(stopped);
		assert.equal(await preview().count(), 1, `${layout}: old mode's result`);
		await resolve(resumed);
		record(`creation-mode-retirement-${layout}`);

		const reordered = await setup();
		const reorder = await start();
		await page.evaluate((id) => window.dashboardFixture.reorder(id), reordered.containerId);
		assert.equal(await preview().count(), 0, `${layout}: changed insertion order`);
		await resolve(reorder);
		record(`creation-order-change-${layout}`);

		const acknowledged = await setup();
		const acknowledgement = await start();
		const result = await resolve(acknowledgement, true);
		assert.equal(result.result, true);
		assert.equal(await preview().count(), 0);
		assert(
			await page.evaluate(
				({ surface, child }) => window.dashboardFixture.chooseChild(surface, child),
				{ surface: acknowledged.containerId, child: result.id }
			)
		);
		assert(
			(
				await page.locator(`.dashboard-widget-shell[data-node-id="${result.id}"]`).innerText()
			).includes(`Created ${acknowledgement}`)
		);
		record(`creation-acknowledgement-${layout}`);

		await setup();
		await page.evaluate(() => window.dashboardFixture.selectSurface());
		const budget = await start();
		await page.evaluate(() => window.dashboardFixture.budget(0));
		assert.equal(await preview().count(), 1, `${layout}: retained destination at zero budget`);
		await page.evaluate(() => window.dashboardFixture.budget(512));
		assert.equal(await preview().count(), 1);
		await resolve(budget);
		record(`creation-budget-round-trip-${layout}`);

		await setup();
		const retired = await start();
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -1000000, panY: -1000000, zoom: 1 })
		);
		assert.equal(await preview().count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		assert.equal(
			await preview().count(),
			0,
			`${layout}: disposed destination must not restore pending work`
		);
		const current = await start(1);
		await resolve(retired);
		assert.equal(await preview().count(), 1);
		await resolve(current);
		record(`creation-destination-round-trip-${layout}`);

		if (layout === 'horizontal') {
			await setup();
			const expired = await start();
			await page.waitForFunction(() => !document.querySelector('.dashboard-drop-preview'), null, {
				timeout: 3500
			});
			await resolve(expired);
			assert.equal(await preview().count(), 0);
			record('creation-pending-expiry');
		}
	}
}
