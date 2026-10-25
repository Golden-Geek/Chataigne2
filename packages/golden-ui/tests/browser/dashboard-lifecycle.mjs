import assert from 'node:assert/strict';

export async function lifecycleChecks(page, record) {
	const setup = await page.evaluate(() => window.dashboardFixture.setupHierarchy(1000, 'mixed'));
	const container = setup.containers[1],
		index = 65;
	assert(
		await page.evaluate(({ surface, id }) => window.dashboardFixture.chooseChild(surface, id), {
			surface: container.id,
			id: setup.ids[index]
		})
	);
	await page.evaluate((index) => window.dashboardFixture.focus(index), index);
	await page.evaluate(() => window.dashboardFixture.budget(0));
	await page
		.locator(`[data-dashboard-child-browser="${container.id}"]`)
		.evaluate((button) => button.focus());
	await page.evaluate(() => window.dashboardFixture.budget(512));
	const focused = await page.evaluate(() => ({
		page: document.activeElement === document.querySelector('.dashboard-page-viewport'),
		active: document.activeElement?.outerHTML.slice(0, 180)
	}));
	assert.equal(focused.page, true, JSON.stringify(focused));
	record('browser-header-focus-after-budget-restoration');

	const tabs = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 600, 'dense', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'tabs' })
	);
	await page.evaluate(() => window.dashboardFixture.select(500));
	assert.equal(
		await page.locator(`.dashboard-widget-shell[data-node-id="${tabs.ids[500]}"]`).count(),
		0
	);
	assert.equal(
		(await page.evaluate(() => window.dashboardFixture.command('view.frame'))).handled,
		true
	);
	assert.equal(
		await page.locator(`.dashboard-widget-shell[data-node-id="${tabs.ids[500]}"]`).count(),
		1,
		'Frame must reveal the inactive selected tab'
	);
	record('frame-reveals-inactive-oversized-tab');

	const flow = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 600, 'dense', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'vertical' })
	);
	await page.evaluate(() =>
		window.dashboardFixture.selectMany(Array.from({ length: 48 }, (_, index) => index))
	);
	const seen = await page.evaluate(
		async (ids) => {
			const seen = new Set();
			const observe = () => {
				for (const shell of document.querySelectorAll('.dashboard-widget-shell'))
					if (ids.includes(Number(shell.dataset.nodeId))) seen.add(Number(shell.dataset.nodeId));
			};
			const observer = new MutationObserver(observe);
			observer.observe(document.querySelector('main'), { subtree: true, childList: true });
			observe();
			try {
				await window.dashboardFixture.command('view.frame');
				observe();
				return [...seen];
			} finally {
				observer.disconnect();
			}
		},
		flow.ids.slice(0, 48)
	);
	assert.equal(
		seen.length,
		48,
		'Frame must measure targets outside the latest selected-control cohort'
	);
	record('frame-materializes-bulk-flow-selection', { targets: 48, seen: seen.length });
	for (const layout of ['horizontal', 'vertical', 'grid']) {
		const fixture = await page.evaluate(() =>
			window.dashboardFixture.setup('free', true, 40, 'dense', true)
		);
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout }),
			layout
		);
		await page.evaluate(() => window.dashboardFixture.selectMany([20, 39]));
		const before = await page
			.locator(`.dashboard-widget-shell[data-node-id="${fixture.ids[39]}"]`)
			.evaluate((shell) => {
				const box = shell.getBoundingClientRect();
				return [box.left, box.top, box.width, box.height];
			});
		await page.evaluate(() => window.dashboardFixture.budget(16));
		const after = await page
			.locator(`.dashboard-widget-shell[data-node-id="${fixture.ids[39]}"]`)
			.evaluate((shell) => {
				const box = shell.getBoundingClientRect();
				return [box.left, box.top, box.width, box.height];
			});
		assert(
			before.every((value, index) => Math.abs(value - after[index]) <= 0.5),
			JSON.stringify({
				layout,
				before,
				after,
				spacers: await page.locator('.dashboard-flow-spacer').evaluateAll((elements) =>
					elements.map((element) => ({
						style: element.getAttribute('style'),
						basis: getComputedStyle(element).flexBasis,
						width: element.getBoundingClientRect().width
					}))
				)
			})
		);
		record(`bounded-flow-preserves-${layout}-geometry`, { before, after });
	}
	const units = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 20, 'dense', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'horizontal' })
	);
	for (let index = 0; index < 20; index++) {
		await page.evaluate(
			(index) =>
				window.dashboardFixture.update(index, 'width', { kind: 'css_value', value: 1, unit: 'px' }),
			index
		);
		if (index >= 6 && index < 19)
			await page.evaluate((index) => window.dashboardFixture.enabled(index, 'width', false), index);
	}
	for (const [index, unit] of ['px', 'rem', 'em', 'percent', 'vw', 'vh'].entries())
		await page.evaluate(
			({ index, unit }) =>
				window.dashboardFixture.update(index, 'width', {
					kind: 'css_value',
					value: 0.123456,
					unit
				}),
			{ index, unit }
		);
	await page.evaluate(() => window.dashboardFixture.select(19));
	const unitBox = () =>
		page.locator(`.dashboard-widget-shell[data-node-id="${units.ids[19]}"]`).evaluate((shell) => {
			const box = shell.getBoundingClientRect();
			return [box.left, box.top, box.width, box.height];
		});
	const unitBefore = await unitBox();
	await page.evaluate(() => window.dashboardFixture.budget(8));
	const unitAfter = await unitBox();
	assert(
		unitBefore.every((value, index) => Math.abs(value - unitAfter[index]) <= 0.5),
		JSON.stringify({ unitBefore, unitAfter })
	);
	record('bounded-flow-mixed-units-and-auto-size', { before: unitBefore, after: unitAfter });

	for (const layout of ['horizontal', 'vertical', 'grid', 'accordion', 'tabs']) {
		const data = await page.evaluate(() =>
			window.dashboardFixture.setup('free', true, 40, 'dense', true)
		);
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout }),
			layout
		);
		if (layout === 'accordion')
			assert(
				await page.evaluate(({ surface, id }) => window.dashboardFixture.chooseChild(surface, id), {
					surface: data.containerId,
					id: data.ids[18]
				})
			);
		await page.evaluate(() => window.dashboardFixture.selectMany([0, 18]));
		const expected = await page.evaluate(
			({ ids, layout }) => {
				const root = document.querySelector('.dashboard-container-surface');
				const elements =
					layout === 'tabs'
						? [...root.querySelectorAll('.dashboard-tab-strip button')]
						: ids.map((id) =>
								root.querySelector(
									`.dashboard-widget-shell[data-node-id="${id}"] .dashboard-widget-label`
								)
							);
				const first = elements[0],
					last = layout === 'tabs' ? elements[18] : elements[1];
				const start = document.createTreeWalker(first, NodeFilter.SHOW_TEXT).nextNode();
				const end = document.createTreeWalker(last, NodeFilter.SHOW_TEXT).nextNode();
				const range = document.createRange();
				range.setStart(start, 0);
				range.setEnd(end, end.textContent.length);
				const selection = window.getSelection();
				selection.removeAllRanges();
				selection.addRange(range);
				return selection.toString();
			},
			{ ids: [data.ids[0], data.ids[18]], layout }
		);
		assert(expected.length > 0);
		await page.evaluate(() => window.dashboardFixture.budget(8));
		assert.equal(
			await page.evaluate(() => window.getSelection().toString()),
			expected,
			`${layout} native range must survive budget changes`
		);
		await page.evaluate(() => window.getSelection().removeAllRanges());
		await page.evaluate(() => window.dashboardFixture.budget(8));
		const released = await page.locator('.dashboard-widget-shell').count();
		assert(released <= 3, `${layout} must release the native range cohort: ${released}`);
		record(`bounded-native-range-${layout}`, {
			characters: expected.length,
			releasedMounted: released
		});
	}
	const scroll = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 600, 'dense', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'horizontal' })
	);
	assert(
		await page.evaluate(({ surface, id }) => window.dashboardFixture.chooseChild(surface, id), {
			surface: scroll.containerId,
			id: scroll.ids[599]
		})
	);
	const scrollState = () =>
		page.locator('.dashboard-container-surface').evaluate((surface) => {
			const browser = surface.querySelector('[data-dashboard-child-browser]'),
				box = browser.getBoundingClientRect(),
				bounds = surface.getBoundingClientRect();
			return {
				left: surface.scrollLeft,
				top: surface.scrollTop,
				headerVisible:
					box.left >= bounds.left - 0.5 &&
					box.right <= bounds.right + 0.5 &&
					box.top >= bounds.top - 0.5 &&
					box.bottom <= bounds.bottom + 0.5
			};
		});
	const beforeScroll = await scrollState();
	assert(beforeScroll.left > 1000);
	assert(beforeScroll.headerVisible, JSON.stringify(beforeScroll));
	await page.evaluate(() => window.dashboardFixture.selectMany([]));
	await page.evaluate(() => window.dashboardFixture.view({ panX: -10000, panY: -10000, zoom: 1 }));
	assert.equal(await page.locator('.dashboard-container-surface').count(), 0);
	await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 1 }));
	const afterScroll = await scrollState();
	assert(
		Math.abs(beforeScroll.left - afterScroll.left) <= 0.5,
		JSON.stringify({ beforeScroll, afterScroll })
	);
	record('local-scroll-state-round-trip', { before: beforeScroll, after: afterScroll });
	for (const layout of ['horizontal', 'vertical', 'accordion', 'tabs']) {
		const data = await page.evaluate(() =>
			window.dashboardFixture.setup('free', true, 600, 'dense', true)
		);
		await page.evaluate(
			(layout) =>
				window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: layout }),
			layout
		);
		await page.evaluate(() =>
			window.dashboardFixture.selectMany(Array.from({ length: 100 }, (_, index) => index))
		);
		const result = await page.evaluate(
			async (ids) => {
				const seen = new Set();
				let peak = 0;
				let panes;
				const read = () => {
					if (!panes && document.querySelector('.dashboard-frame-measurement'))
						panes = [...document.querySelectorAll('.dashboard-tab-body > .dashboard-slot')]
							.slice(0, 3)
							.map((slot) => ({
								class: slot.className,
								rect: slot.getBoundingClientRect().toJSON(),
								widget: slot
									.querySelector('.dashboard-widget-shell')
									?.getBoundingClientRect()
									.toJSON(),
								body: slot.parentElement.getBoundingClientRect().toJSON()
							}));
					const widgets = [...document.querySelectorAll('.dashboard-widget-shell')];
					peak = Math.max(peak, widgets.length);
					for (const widget of widgets)
						if (ids.includes(Number(widget.dataset.nodeId)))
							seen.add(Number(widget.dataset.nodeId));
				};
				const observer = new MutationObserver(read);
				observer.observe(document.querySelector('main'), { subtree: true, childList: true });
				read();
				try {
					return {
						...(await window.dashboardFixture.command('view.frame')),
						seen: [...seen],
						peak,
						panes
					};
				} finally {
					observer.disconnect();
				}
			},
			data.ids.slice(0, 100)
		);
		assert.equal(result.handled, true);
		assert.equal(result.seen.length, 100);
		assert(result.peak <= 49, JSON.stringify({ layout, peak: result.peak }));
		let paneError = 0;
		if (layout === 'tabs') {
			assert(result.panes?.length >= 2);
			for (const pane of result.panes)
				for (const key of ['x', 'y', 'width', 'height'])
					paneError = Math.max(paneError, Math.abs(pane.rect[key] - result.panes[0].rect[key]));
			assert(paneError <= 0.5, JSON.stringify(result.panes));
		}
		const target = data.ids[layout === 'tabs' ? 0 : 99];
		const center = await page
			.locator(`.dashboard-widget-shell[data-node-id="${target}"]`)
			.evaluate((shell) => {
				let box = shell.getBoundingClientRect();
				let left = box.left,
					top = box.top,
					right = box.right,
					bottom = box.bottom;
				const frame = shell.closest('.dashboard-page-frame'),
					viewport = shell.closest('.dashboard-page-viewport').getBoundingClientRect();
				for (
					let parent = shell.parentElement;
					parent && parent !== frame;
					parent = parent.parentElement
				) {
					const style = getComputedStyle(parent),
						bounds = parent.getBoundingClientRect();
					const sx = bounds.width / Math.max(parent.offsetWidth, 1),
						sy = bounds.height / Math.max(parent.offsetHeight, 1);
					if (style.overflowX !== 'visible') {
						left = Math.max(left, bounds.left + parent.clientLeft * sx);
						right = Math.min(right, bounds.left + (parent.clientLeft + parent.clientWidth) * sx);
					}
					if (style.overflowY !== 'visible') {
						top = Math.max(top, bounds.top + parent.clientTop * sy);
						bottom = Math.min(bottom, bounds.top + (parent.clientTop + parent.clientHeight) * sy);
					}
				}
				return {
					visible: right > left && bottom > top,
					surface: (() => {
						const element = shell.closest('.dashboard-container-surface'),
							box = element.getBoundingClientRect();
						const sx = box.width / Math.max(element.offsetWidth, 1),
							sy = box.height / Math.max(element.offsetHeight, 1);
						let left = box.left + element.clientLeft * sx,
							top = box.top + element.clientTop * sy;
						let right = left + element.clientWidth * sx,
							bottom = top + element.clientHeight * sy;
						for (
							let parent = element.parentElement;
							parent && parent !== frame;
							parent = parent.parentElement
						) {
							const style = getComputedStyle(parent),
								rect = parent.getBoundingClientRect();
							const x = rect.width / Math.max(parent.offsetWidth, 1),
								y = rect.height / Math.max(parent.offsetHeight, 1);
							if (style.overflowX !== 'visible') {
								left = Math.max(left, rect.left + parent.clientLeft * x);
								right = Math.min(right, rect.left + (parent.clientLeft + parent.clientWidth) * x);
							}
							if (style.overflowY !== 'visible') {
								top = Math.max(top, rect.top + parent.clientTop * y);
								bottom = Math.min(bottom, rect.top + (parent.clientTop + parent.clientHeight) * y);
							}
						}
						return {
							scrollX: ['auto', 'scroll'].includes(getComputedStyle(element).overflowX),
							scrollY: ['auto', 'scroll'].includes(getComputedStyle(element).overflowY),
							dx: Math.abs((left + right) / 2 - viewport.left - viewport.width / 2),
							dy: Math.abs((top + bottom) / 2 - viewport.top - viewport.height / 2),
							scrollTop: element.scrollTop,
							scrollHeight: element.scrollHeight,
							clientHeight: element.clientHeight,
							clientWidth: element.clientWidth
						};
					})(),
					dx: Math.abs((left + right) / 2 - viewport.left - viewport.width / 2),
					dy: Math.abs((top + bottom) / 2 - viewport.top - viewport.height / 2)
				};
			});
		const selection = {
			dx: center.surface.scrollX ? center.surface.dx : center.dx,
			dy: center.surface.scrollY ? center.surface.dy : center.dy
		};
		assert(
			center.visible && selection.dx <= 0.5 && selection.dy <= 0.5,
			JSON.stringify({ layout, center, panes: result.panes })
		);
		record(`bulk-frame-100-${layout}`, {
			targets: 100,
			peak: result.peak,
			center,
			selection,
			paneError
		});
	}
	await page.evaluate(() => window.dashboardFixture.setup('free', true, 600, 'dense', true));
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'layout_kind', { kind: 'enum', value: 'tabs' })
	);
	await page.evaluate(() => window.dashboardFixture.select(500));
	const changed = await page.evaluate(() => window.dashboardFixture.frameWithReorder());
	assert.equal(changed.handled, true);
	assert.equal(changed.after, changed.before);
	record('frame-cancels-after-source-reorder', changed);
}
