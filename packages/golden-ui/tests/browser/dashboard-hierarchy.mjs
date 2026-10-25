import assert from 'node:assert/strict';

export async function hierarchyChecks(page, record) {
	for (const layout of ['tabs', 'accordion']) {
		const setup = await page.evaluate(() =>
			window.dashboardFixture.setup('free', true, 6, 'grid', true)
		);
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout }),
			layout
		);
		assert.equal(
			await page.evaluate(
				(layout) =>
					layout === 'tabs' ? window.dashboardFixture.tab(2) : window.dashboardFixture.accordion(2),
				layout
			),
			true
		);
		assert.equal(
			await page.locator(`.dashboard-widget-shell[data-node-id="${setup.ids[2]}"]`).count(),
			1
		);
		await page.evaluate(() =>
			window.dashboardFixture.view({ panX: -10000, panY: -10000, zoom: 1 })
		);
		assert.equal(await page.locator(`[data-node-id="${setup.containerId}"]`).count(), 0);
		await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
		const actual = await page.evaluate(
			(id) => ({
				frame: document.querySelector('.dashboard-page-frame')?.getAttribute('style'),
				layout: [...document.querySelectorAll('[data-dashboard-free-total]')].map((element) => ({
					total: element.dataset.dashboardFreeTotal,
					mounted: element.dataset.dashboardFreeMounted,
					box: element.getBoundingClientRect().toJSON(),
					width: element.clientWidth,
					height: element.clientHeight
				})),
				count: document.querySelectorAll(`.dashboard-widget-shell[data-node-id="${id}"]`).length,
				widgets: [...document.querySelectorAll('.dashboard-widget-shell')].map((shell) => ({
					id: shell.dataset.nodeId,
					text: shell.textContent.trim()
				})),
				budgets: [...document.querySelectorAll('[data-dashboard-budget]')].map(
					(marker) => marker.dataset.dashboardBudget
				),
				tabs: [...document.querySelectorAll('.dashboard-tab-strip button')].map((button) => ({
					text: button.textContent,
					selected: button.classList.contains('selected')
				}))
			}),
			setup.ids[2]
		);
		assert.equal(actual.count, 1, `${layout} round trip: ${JSON.stringify(actual)}`);
		record(`container-${layout}-state-retention`);
	}
	for (const target of [1000, 10_000, 20_000, 50_000, 100_000]) {
		for (const kind of ['free', 'mixed']) {
			const setup = await page.evaluate(
				({ target, kind }) => window.dashboardFixture.setupHierarchy(target, kind),
				{ target, kind }
			);
			const counts = await page.evaluate(() => ({
				mountedWidgets: document.querySelectorAll('.dashboard-widget-shell').length,
				groups: document.querySelectorAll('.dashboard-free-overview rect').length,
				mountedContainers: document.querySelectorAll('.dashboard-widget-shell.dashboard-container')
					.length,
				browsers: document.querySelectorAll('[data-dashboard-child-browser]').length
			}));
			assert(counts.mountedWidgets <= 512, JSON.stringify({ target, kind, counts }));
			assert(counts.mountedWidgets + counts.groups <= 1024, JSON.stringify(counts));
			for (const container of setup.containers) {
				assert.equal(
					await page.locator(`.dashboard-widget-shell[data-node-id="${container.id}"]`).count(),
					1
				);
			}
			if (kind === 'mixed' && target >= 10_000) {
				const container = setup.containers[1],
					id = container.ids.at(-1);
				assert(
					await page.evaluate((surface) => window.dashboardFixture.browse(surface), container.id)
				);
				assert(
					await page.evaluate(
						({ surface, id }) => window.dashboardFixture.revealChild(surface, id),
						{ surface: container.id, id }
					)
				);
				const row = await page.locator('.dashboard-child-menu .gc-context-item').allTextContents();
				assert(
					await page.evaluate(() => document.activeElement?.matches('.dashboard-child-menu input'))
				);
				assert(row.length > 0 && row.length <= 42);
				const index = setup.ids.indexOf(id);
				assert(row.some((label) => label.includes(`Widget ${index}`)));
				await page.evaluate(
					({ surface, query }) => window.dashboardFixture.searchChildren(surface, query),
					{ surface: container.id, query: `Widget ${index}` }
				);
				assert.equal(await page.locator('.dashboard-child-menu .gc-context-item').count(), 1);
				assert(
					await page.evaluate(
						({ surface, id }) => window.dashboardFixture.chooseChild(surface, id),
						{ surface: container.id, id }
					)
				);
				assert.equal(
					await page.locator(`.dashboard-widget-shell[data-node-id="${id}"]`).count(),
					1
				);
				assert(
					await page.evaluate(
						(surface) =>
							document.activeElement?.getAttribute('data-dashboard-child-browser') ===
							String(surface),
						container.id
					)
				);
			}
			record(`hierarchy-${kind}-${target}`, {
				targetAuthoredNodes: target,
				authoredNodes: setup.authoredNodes,
				authoredWidgets: setup.ids.length + setup.containers.length,
				leafWidgets: setup.ids.length,
				...counts
			});
		}
	}
	for (let family = 0; family < 6; family++) {
		const setup = await page.evaluate(() => window.dashboardFixture.setupHierarchy(1000, 'mixed'));
		const container = setup.containers[family];
		const index = setup.ids.findIndex(
			(id, index) => container.ids.includes(id) && [4, 5].includes(index % 6)
		);
		assert(index >= 0);
		const id = setup.ids[index];
		assert(
			await page.evaluate(({ surface, id }) => window.dashboardFixture.chooseChild(surface, id), {
				surface: container.id,
				id
			})
		);
		await page.evaluate((index) => window.dashboardFixture.focus(index), index);
		const focused = await page.evaluateHandle(() => document.activeElement);
		try {
			await page.evaluate(() => window.dashboardFixture.budget(0));
			assert(await focused.evaluate((element) => element === document.activeElement));
			assert((await page.locator('.dashboard-widget-shell').count()) <= 2);
			await page.evaluate(() => window.dashboardFixture.budget(512));
			assert(await focused.evaluate((element) => element === document.activeElement));
			await page.evaluate((index) => window.dashboardFixture.remove(index), index);
			assert(
				await page.evaluate(
					() => document.activeElement === document.querySelector('.dashboard-page-viewport')
				)
			);
		} finally {
			await focused.dispose();
		}
		record(`hierarchy-focus-budget-${container.layout}`, { id, index });
	}
	const deep = await page.evaluate(() => window.dashboardFixture.setupHierarchy(100_000, 'deep'));
	const initial = await page.locator('.dashboard-widget-shell').count();
	assert(initial <= 512);
	const selected = deep.ids.length - 1;
	await page.evaluate(() => window.dashboardFixture.view({ panX: -10000, panY: -10000, zoom: 1 }));
	assert.equal(await page.locator('.dashboard-widget-shell').count(), 0);
	await page.evaluate((index) => window.dashboardFixture.select(index), selected);
	const framed = await page.evaluate(async () => {
		let peak = 0;
		const observer = new MutationObserver(() => {
			peak = Math.max(peak, document.querySelectorAll('.dashboard-widget-shell').length);
		});
		observer.observe(document.querySelector('main'), { subtree: true, childList: true });
		try {
			return { ...(await window.dashboardFixture.command('view.frame')), peak };
		} finally {
			observer.disconnect();
		}
	});
	assert.equal(framed.handled, true);
	assert.equal(
		await page.locator(`.dashboard-widget-shell[data-node-id="${deep.ids[selected]}"]`).count(),
		1
	);
	const peak = await page.locator('.dashboard-widget-shell').count();
	assert(peak <= 528);
	assert(framed.peak <= 528);
	const center = await page
		.locator(`.dashboard-widget-shell[data-node-id="${deep.ids[selected]}"]`)
		.evaluate((shell) => {
			const box = shell.getBoundingClientRect(),
				viewport = shell.closest('.dashboard-page-viewport').getBoundingClientRect();
			return {
				dx: Math.abs(box.left + box.width / 2 - viewport.left - viewport.width / 2),
				dy: Math.abs(box.top + box.height / 2 - viewport.top - viewport.height / 2)
			};
		});
	assert(center.dx <= 0.5 && center.dy <= 0.5, JSON.stringify(center));
	record('hierarchy-depth-eight-frame', {
		authoredNodes: deep.authoredNodes,
		containers: deep.containers.length,
		widgets: deep.ids.length,
		initialMounted: initial,
		finalMounted: peak,
		peakMounted: framed.peak,
		center
	});
}
