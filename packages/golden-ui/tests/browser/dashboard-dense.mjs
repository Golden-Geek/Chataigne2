import assert from 'node:assert/strict';

async function state(page) {
	return page.evaluate(() => {
		const layout = document.querySelector('[data-dashboard-free-total]');
		return {
			presentation: layout.dataset.dashboardFreePresentation,
			mounted: [...document.querySelectorAll('.dashboard-widget-shell')].map((shell) =>
				Number(shell.dataset.nodeId)
			),
			groups: [...layout.querySelectorAll('.dashboard-free-overview rect')].map((rect) => ({
				members: Number(rect.dataset.groupMembers),
				box: ['x', 'y', 'width', 'height'].map((name) => Number(rect.getAttribute(name)))
			})),
			text: layout.querySelector('.dashboard-free-overview')?.textContent.trim(),
			selected: window.dashboardFixture.selected()
		};
	});
}

async function gridCoverage(page, ids, editing) {
	return page.evaluate(
		({ ids, editing }) => {
			const layout = document.querySelector('[data-dashboard-free-total]');
			const bounds = layout.getBoundingClientRect();
			const scaleX = bounds.width / layout.clientWidth,
				scaleY = bounds.height / layout.clientHeight;
			const scene = document.querySelector('.dashboard-page-scene').getBoundingClientRect();
			const page = document.querySelector('.dashboard-page').getBoundingClientRect();
			const clip = editing
				? scene
				: {
						left: Math.max(scene.left, page.left),
						top: Math.max(scene.top, page.top),
						right: Math.min(scene.right, page.right),
						bottom: Math.min(scene.bottom, page.bottom)
					};
			const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
			const button = layout.querySelector('.dashboard-free-overview');
			const groups = [...layout.querySelectorAll('.dashboard-free-overview rect')].map((rect) => ({
				left: Number.parseFloat(button.style.left) + Number(rect.getAttribute('x')),
				top: Number.parseFloat(button.style.top) + Number(rect.getAttribute('y')),
				width: Number(rect.getAttribute('width')),
				height: Number(rect.getAttribute('height'))
			}));
			const shells = new Map(
				[...layout.querySelectorAll('.dashboard-widget-shell')].map((shell) => [
					Number(shell.dataset.nodeId),
					shell
				])
			);
			let visible = 0,
				maximumError = 0,
				maximumGroupError = 0;
			const missing = [];
			for (let index = 0; index < ids.length; index++) {
				const x = (index % 40) * 240,
					y = Math.floor(index / 40) * 112;
				const rect = {
					left: bounds.left + x * scaleX,
					top: bounds.top + y * scaleY,
					width: 12 * rem * scaleX,
					height: 5 * rem * scaleY
				};
				if (
					rect.left >= clip.right ||
					rect.top >= clip.bottom ||
					rect.left + rect.width <= clip.left ||
					rect.top + rect.height <= clip.top
				)
					continue;
				visible++;
				if (button) {
					// CSS style serialization rounds the overview origin; measure in client pixels.
					const error = Math.min(
						...groups.map((group) =>
							Math.max(
								0,
								(group.left - x) * scaleX,
								(group.top - y) * scaleY,
								(x + 12 * rem - group.left - group.width) * scaleX,
								(y + 5 * rem - group.top - group.height) * scaleY
							)
						)
					);
					maximumGroupError = Math.max(maximumGroupError, error);
					if (error > 0.5) missing.push(ids[index]);
				} else {
					const shell = shells.get(ids[index]);
					if (!shell) missing.push(ids[index]);
					else {
						const actual = shell.getBoundingClientRect();
						for (const key of ['left', 'top', 'width', 'height'])
							maximumError = Math.max(maximumError, Math.abs(actual[key] - rect[key]));
					}
				}
			}
			return { visible, missing, maximumError, maximumGroupError };
		},
		{ ids, editing }
	);
}

export async function denseChecks(page, record) {
	for (const authored of [1000, 10_000, 20_000, 50_000, 100_000]) {
		const count = Math.ceil((authored - 11) / 11);
		for (const editing of [false, true]) {
			const setup = await page.evaluate(
				({ count, editing }) => window.dashboardFixture.setup('free', editing, count, 'dense'),
				{ count, editing }
			);
			const initial = await state(page);
			assert.equal(initial.presentation, count > 192 ? 'overview' : 'detail');
			assert.equal(initial.mounted.length, count > 192 ? 0 : count);
			if (count > 192) {
				assert(initial.groups.length > 0 && initial.groups.length <= 256);
				assert.equal(
					initial.groups.reduce((sum, group) => sum + group.members, 0),
					count
				);
				assert.match(initial.text, /Select a widget or zoom in/);
			}
			const picked = await page.evaluate(() => window.dashboardFixture.activate(0));
			assert.equal(picked, setup.ids.at(-1), 'Exact topmost identity must survive dense grouping');
			assert((await state(page)).mounted.includes(picked));
			const control = await page
				.locator(`.dashboard-widget-shell[data-node-id="${picked}"]`)
				.evaluate((shell) => {
					const box = shell.getBoundingClientRect();
					const slot = shell.closest('.dashboard-slot').getBoundingClientRect();
					const front = document.elementFromPoint(
						box.left + box.width / 2,
						box.top + box.height / 2
					);
					return {
						label: shell.textContent,
						width: box.width,
						height: box.height,
						fit: Math.max(Math.abs(box.width - slot.width), Math.abs(box.height - slot.height)),
						box: [box.left, box.top, box.width, box.height],
						front: front?.outerHTML.slice(0, 240),
						reachable: shell.contains(front)
					};
				});
			assert.match(control.label, new RegExp(`Widget ${count - 1}`));
			assert(control.width > 0 && control.height > 0 && control.fit <= 0.5);
			assert.equal(control.reachable, true, JSON.stringify({ authored, editing, control }));
			await page.evaluate(() => window.dashboardFixture.reorder());
			assert.equal(await page.evaluate(() => window.dashboardFixture.activate(0)), setup.ids[0]);
			await page.evaluate(() => window.dashboardFixture.remove(0));
			assert.equal(await page.evaluate(() => window.dashboardFixture.activate(1)), setup.ids[1]);
			await page.evaluate(() => window.dashboardFixture.select(4));
			await page.evaluate(() => window.dashboardFixture.focus(4));
			await page.evaluate(() => {
				window.denseFocusedControl = document.activeElement;
			});
			await page.evaluate(() => window.dashboardFixture.selectAllWidgets());
			const bulk = await state(page);
			assert.equal(
				bulk.selected.length,
				count - 1,
				'Bulk logical selection must include all remaining widgets'
			);
			assert(bulk.mounted.length <= (count > 192 ? 17 : count));
			await page.evaluate(() =>
				window.dashboardFixture.update(4, 'position', { kind: 'vec2', value: [6000, 2] })
			);
			await page.evaluate(() => window.dashboardFixture.reveal(4));
			const detailed = await state(page);
			assert.equal(detailed.presentation, 'detail');
			assert.deepEqual(detailed.mounted, [setup.ids[4]]);
			assert(await page.evaluate(() => document.activeElement === window.denseFocusedControl));
			await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
			assert(
				(await state(page)).mounted.includes(setup.ids[4]),
				'Focused control must survive reversal'
			);
			await page.evaluate(() => window.dashboardFixture.remove(4));
			assert(
				await page.evaluate(
					() => document.activeElement === document.querySelector('.dashboard-page-viewport')
				)
			);
			record(`dense-overlap-${authored}-${editing ? 'edit' : 'run'}`, {
				authoredNodes: setup.authoredNodes,
				widgets: count,
				initialMounted: initial.mounted.length,
				groups: initial.groups.length,
				bulkMounted: bulk.mounted.length,
				picked,
				control,
				transitionScope: editing
					? 'Distant editing staging'
					: 'Focus retained beyond the run-mode page clip'
			});

			const gridSetup = await page.evaluate(
				({ count, editing }) => window.dashboardFixture.setup('free', editing, count, 'grid'),
				{ count, editing }
			);
			await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 0.25 }));
			const grid = await state(page);
			const coverage = await gridCoverage(page, gridSetup.ids, editing);
			assert.deepEqual(coverage.missing, []);
			assert(coverage.visible > 0 && coverage.maximumError <= 0.5, JSON.stringify(coverage));
			assert(grid.mounted.length <= 192);
			assert(grid.groups.length <= 256);
			if (grid.presentation === 'overview') assert.equal(grid.mounted.length, 0);
			const gridPicked = await page.evaluate(() => window.dashboardFixture.activate(0));
			assert.equal(gridPicked, (await page.evaluate(() => window.dashboardFixture.selected()))[0]);
			assert((await state(page)).mounted.includes(gridPicked));
			if (grid.presentation === 'overview') {
				await page.locator('.dashboard-free-overview').evaluate((button) => button.focus());
				await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 4 }));
				assert.equal((await state(page)).presentation, 'detail');
				assert(
					await page.evaluate(
						() => document.activeElement === document.querySelector('.dashboard-page-viewport')
					)
				);
			}
			record(`dense-grid-minimum-zoom-${authored}-${editing ? 'edit' : 'run'}`, {
				widgets: count,
				presentation: grid.presentation,
				mounted: grid.mounted.length,
				groups: grid.groups.length,
				coverage,
				scope: 'Viewport slice at 0.25; run mode retains the authored page clip'
			});
		}
	}
}
