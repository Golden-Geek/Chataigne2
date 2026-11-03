import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixDragUi({ base, edit }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Drag grid'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const [a, b, c] = initial.items;
	await matrix({
		kind: 'CreateCellMedia',
		layer: a.id.id,
		column: initial.columns[0].id,
		media_kind: 'SolidColor'
	});
	const {
		items: [group]
	} = await matrix({ kind: 'AddGroup', parent: null, label: 'Stack' });
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Matrix drag sorting');
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('region', { name: 'Media Manager', exact: true })
			.getByRole('button', { name: 'Drag grid', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const row = (id) => grid.locator(`[data-matrix-drop="Item"][data-row-id="${id}"]`);
		const column = (id) => grid.locator(`[data-matrix-drop="Column"][data-column-id="${id}"]`);
		async function drag(source, target, position, predicate) {
			await wait(
				() => grid.getByRole('button', { name: 'Add layer', exact: true }).isEnabled(),
				Boolean
			);
			const before = await inspect();
			await page.waitForFunction(
				(ids) =>
					JSON.stringify(
						[...document.querySelectorAll('[data-matrix-drop="Item"]')].map(
							(element) => element.dataset.rowId
						)
					) === JSON.stringify(ids),
				before.items.map((row) => row.id.id)
			);
			const sourceBounds = await source.boundingBox(),
				targetBounds = await target.boundingBox();
			await page.mouse.move(sourceBounds.x + 20, sourceBounds.y + 30);
			await page.mouse.down();
			await page.mouse.move(targetBounds.x + position.x, targetBounds.y + position.y, { steps: 5 });
			await page.mouse.move(targetBounds.x + position.x, targetBounds.y + position.y);
			await page.mouse.up();
			const after = await wait(inspect, predicate);
			assert.notDeepEqual(after, before);
			assert.deepEqual(
				after.cells,
				before.cells,
				'moving rows/Columns retains exact owned Cell/media identities'
			);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
		}
		await drag(
			row(c.id.id),
			row(b.id.id),
			{ x: 40, y: 4 },
			(state) => state.items[1].id.id === c.id.id
		);
		await drag(
			row(a.id.id),
			row(group.id),
			{ x: 40, y: 60 },
			(state) => state.items.find((row) => row.id.id === a.id.id).parent === group.id
		);
		await drag(
			row(b.id.id),
			row(group.id),
			{ x: 40, y: 60 },
			(state) => state.items.find((row) => row.id.id === b.id.id).parent === group.id
		);
		await drag(
			row(b.id.id),
			row(a.id.id),
			{ x: 40, y: 4 },
			(state) => state.items.filter((row) => row.parent === group.id)[0].id.id === b.id.id
		);
		await drag(
			row(group.id),
			row(c.id.id),
			{ x: 40, y: 4 },
			(state) => state.items[0].id.id === group.id
		);
		await drag(
			row(a.id.id),
			grid.getByRole('toolbar', { name: 'Matrix actions', exact: true }),
			{ x: 200, y: 25 },
			(state) => state.items.at(-1).id.id === a.id.id && state.items.at(-1).parent === null
		);
		await drag(
			column(initial.columns[2].id),
			column(initial.columns[1].id),
			{ x: 3, y: 20 },
			(state) => state.columns[1].id === initial.columns[2].id
		);
		await drag(
			column(initial.columns[0].id),
			column(initial.columns[2].id),
			{ x: 115, y: 20 },
			(state) => state.columns[0].id === initial.columns[2].id
		);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		const beforeCancel = await inspect();
		const bounds = await row(c.id.id).boundingBox();
		await page.mouse.move(bounds.x + 20, bounds.y + 30);
		await page.mouse.down();
		await page.mouse.move(bounds.x + 60, bounds.y + 50, { steps: 5 });
		await page.keyboard.press('Escape');
		await page.mouse.up();
		assert.deepEqual(await inspect(), beforeCancel, 'Escape cancels an unfinished drag');
		assert.equal(await grid.locator('[data-matrix-drop-position]').count(), 0);
		await grid.getByRole('button', { name: 'Launch Column 1', exact: true }).click();
		const playback = await wait(
			async () => (await matrix({ kind: 'InspectPlayback' })).snapshot,
			(state) => state.desired_column === initial.columns[0].id
		);
		await drag(
			column(initial.columns[1].id),
			column(initial.columns[2].id),
			{ x: 3, y: 20 },
			(state) => state.columns[0].id === initial.columns[1].id
		);
		assert.equal(
			(await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column,
			playback.desired_column
		);
		const { items: added } = await matrix({ kind: 'AddLayers', parent: null, count: 128 });
		const beforeLargeDrag = await inspect();
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(count) => count === String(beforeLargeDrag.items.length + 1)
		);
		const moving = await row(c.id.id).boundingBox();
		await page.mouse.move(moving.x + 20, moving.y + 30);
		await page.mouse.down();
		await page.mouse.move(moving.x + 60, moving.y + 50, { steps: 5 });
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.dispatchEvent(new Event('scroll'));
		});
		await wait(
			() => row(c.id.id).count(),
			(count) => count === 0
		);
		const destination = row(added.at(-1).id);
		await destination.waitFor();
		const landing = await destination.boundingBox();
		assert(
			(await grid.getByRole('rowheader').count()) < 30,
			'held drag does not mount the whole Matrix'
		);
		await page.mouse.move(landing.x + 40, landing.y + landing.height - 4, { steps: 5 });
		await page.mouse.move(landing.x + 40, landing.y + landing.height - 4);
		await page.mouse.up();
		const reordered = await wait(inspect, (state) => state.items.at(-1).id.id === c.id.id);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeLargeDrag));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, reordered));
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m07-drag-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Actual Layer/Group before/inside/root drags, complete subtree retention, Column before/after sort, exact one-step Undo/Redo, Escape cancellation, no implicit launch, stable active Column identity and bounded dragging after the source row leaves the viewport',
				browser: browser.version()
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-drag-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
