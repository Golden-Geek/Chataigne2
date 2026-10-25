import assert from 'node:assert/strict';
import { compositionLifetimes } from './dashboard-composition-lifetimes.mjs';

const layouts = ['free', 'horizontal', 'vertical', 'grid', 'tabs', 'accordion'];
export async function compositionChecks(page, record) {
	let scene;
	const state = () => page.evaluate(() => window.dashboardFixture.editState());
	const selector = () => `.dashboard-widget-shell[data-node-id="${scene.source}"]`;
	const edit = (command) =>
		page.evaluate(({ index, command }) => window.dashboardFixture.layoutEdit(index, command), {
			index: scene.sourceIndex,
			command
		});
	const setup = async (family, layout = 'free', destination = 'free', options = {}) => {
		await page.evaluate(async () => {
			await window.dashboardFixture.rootFont(16);
			await window.dashboardFixture.localFont(16);
		});
		scene = await page.evaluate(
			({ family, layout, destination }) =>
				window.dashboardFixture.setupComposition(family, layout, destination),
			{ family, layout, destination }
		);
		await page.evaluate((options) => window.dashboardFixture.configureEdits(options), options);
		if (family === 'node')
			assert(
				(await page.locator(selector() + ' [data-fixture-node-content]').innerText()).includes(
					'Registered renderer:'
				)
			);
		assert((await page.locator('.dashboard-widget-shell').count()) <= 1024);
	};
	const start = async (kind) => {
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
		return { box, origin };
	};
	const update = (origin, delta = 24) =>
		edit({ kind: 'update', point: { x: origin.x + delta, y: origin.y + delta } });
	const finished = async () => {
		assert(await edit({ kind: 'finish' }));
		const s = await state();
		assert.equal(s.active, false);
		assert.equal(s.pending, 0);
		assert.equal(s.operations, 0);
		return s;
	};
	const targetPoint = async (layout, after = false) => {
		const id = scene.anchors[1];
		const target = page.locator(
			layout === 'tabs' || layout === 'accordion'
				? `[data-dashboard-surface-node-id="${scene.destination}"] [data-dashboard-child-id="${id}"]`
				: `.dashboard-widget-shell[data-node-id="${id}"]`
		);
		const box = await target.boundingBox();
		assert(box, JSON.stringify({ layout, scene }));
		return {
			x: box.x + box.width * (after ? 0.75 : 0.25),
			y: box.y + box.height * (after ? 0.75 : 0.25)
		};
	};
	const orderedTarget = async (family, layout) => {
		let point, expected;
		if (family === 'container' && (layout === 'tabs' || layout === 'accordion')) {
			const header = await page
				.locator(`[data-dashboard-child-id="${scene.destination}"]`)
				.boundingBox();
			assert(header);
			point = { x: header.x + header.width * 0.75, y: header.y + header.height * 0.75 };
			expected = scene.parent;
		} else if (family === 'container') {
			const dest = await page
				.locator(`[data-dashboard-surface-node-id="${scene.destination}"]`)
				.boundingBox();
			assert(dest);
			point = {
				x: dest.x + Math.min(48, dest.width / 4),
				y: dest.y + Math.min(48, dest.height / 4)
			};
			expected = scene.destination;
		} else {
			point = await page.locator('.dashboard-page-frame').evaluate((frame) => {
				const r = frame.getBoundingClientRect();
				return { x: r.right - 24, y: r.bottom - 24 };
			});
			expected = scene.pageId;
		}
		return { point, expected };
	};
	for (const family of ['container', 'node'])
		for (const layout of layouts) {
			await setup(family, layout);
			const { box, origin } = await start('move');
			if (layout === 'free') {
				assert(await update(origin));
				const s = await finished();
				assert(s.records.some((r) => r.kind === 'setParam'));
				const after = await page.locator(selector()).boundingBox();
				assert(after);
				assert(
					after.x > box.x + 10 && after.y > box.y + 10,
					JSON.stringify({ family, layout, box, after })
				);
			} else {
				const { point, expected } = await orderedTarget(family, layout);
				assert(await edit({ kind: 'update', point }));
				const s = await finished();
				const moves = s.records.filter((r) => r.kind === 'moveDashboardWidget');
				assert.equal(moves.length, 1, JSON.stringify({ family, layout, s }));
				assert.equal(moves[0].parent, expected);
			}
			assert.equal(await page.locator(selector()).count(), 1);
			record(`${family}-move-under-${layout}-parent`, { authoredNodes: scene.authoredNodes });
			await setup(family, layout);
			if (['free', 'horizontal', 'vertical'].includes(layout)) {
				const { box, origin } = await start('resize');
				assert(await update(origin));
				await finished();
				const after = await page.locator(selector()).boundingBox();
				assert(after);
				assert(
					Math.abs(after.x - box.x) <= 0.5 && Math.abs(after.y - box.y) <= 0.5,
					JSON.stringify({ family, layout, box, after })
				);
				assert(
					layout === 'vertical' ? after.height > box.height + 10 : after.width > box.width + 10,
					JSON.stringify({ family, layout, box, after })
				);
				record(`${family}-resize-under-${layout}-parent`);
			} else {
				assert.equal(
					await edit({
						kind: 'resize',
						origin: { x: 100, y: 100 },
						edges: { left: false, top: false, right: true, bottom: true }
					}),
					false
				);
				assert.equal((await state()).records.length, 0);
				record(`${family}-refuses-resize-under-${layout}`);
			}
		}
	for (const family of ['generic', 'node', 'container'])
		for (const layout of layouts) {
			await setup(family, 'free', layout);
			const measure = () =>
				page.locator(`[data-dashboard-surface-node-id="${scene.parent}"]`).evaluate((element) => {
					const r = element.getBoundingClientRect(),
						style = getComputedStyle(element);
					return {
						x: r.x,
						y: r.y,
						width: r.width,
						height: r.height,
						clientWidth: element.clientWidth,
						clientHeight: element.clientHeight,
						scrollLeft: element.scrollLeft,
						scrollTop: element.scrollTop,
						font: style.fontSize,
						rootFont: getComputedStyle(document.documentElement).fontSize
					};
				});
			const frameBefore = await measure();
			await start('move');
			const point = await targetPoint(layout);
			assert(await edit({ kind: 'update', point }));
			const frameAfter = await measure();
			const s = await finished();
			const moves = s.records.filter((r) => r.kind === 'moveDashboardWidget');
			assert.equal(
				moves.length,
				1,
				JSON.stringify({ family, layout, s, point, frameBefore, frameAfter })
			);
			assert.equal(moves[0].parent, scene.destination);
			if (layout !== 'free') assert.equal(moves[0].target_index, 1, JSON.stringify(moves));
			if (layout === 'tabs' || layout === 'accordion') {
				assert.equal(await page.locator(`[data-dashboard-child-id="${scene.source}"]`).count(), 1);
				await page.evaluate(
					({ destination, source }) => window.dashboardFixture.chooseChild(destination, source),
					scene
				);
			}
			assert.equal(await page.locator(selector()).count(), 1);
			if (family === 'node')
				assert(
					(await page.locator(selector() + ' [data-fixture-node-content]').innerText()).includes(
						'Registered renderer:'
					)
				);
			assert((await page.locator('.dashboard-widget-shell').count()) <= 1024);
			record(`${family}-moves-into-${layout}-destination`, {
				targetIndex: moves[0].target_index ?? null
			});
		}
	await compositionLifetimes(page, record, {
		setup,
		start,
		update,
		edit,
		state,
		selector,
		orderedTarget,
		scene: () => scene
	});
}
