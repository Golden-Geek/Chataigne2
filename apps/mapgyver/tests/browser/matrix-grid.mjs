import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixGridUi({ base, edit }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Workbench grid'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Matrix square grid');
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('region', { name: 'Media Manager', exact: true })
			.getByRole('button', { name: 'Workbench grid', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const cell = (row = 'Layer 1', column = 'Column 1') =>
			grid.getByTitle(`${row} / ${column}`, { exact: true });
		await cell().waitFor();
		const bounds = await cell().boundingBox();
		assert(Math.abs(bounds.width - bounds.height) <= 1, JSON.stringify(bounds));
		assert((await grid.boundingBox()).height > 500, 'grid uses panel height');
		assert.equal(await page.locator('.editor-toolbar').count(), 0);
		await grid
			.getByRole('button', { name: 'Add Media to Layer 1 / Column 1', exact: true })
			.click();
		await page.getByRole('menuitem', { name: 'Solid Color', exact: true }).click();
		const created = await wait(inspect, (state) => state.cells.some((cell) => cell.media));
		const originalCell = created.cells[0];
		for (const [oldLabel, label, field] of [
			['Layer 1', 'Stage A', 'items'],
			['Column 1', 'Scene A', 'columns']
		]) {
			const before = await inspect();
			await grid.getByTitle(`Select ${oldLabel}`, { exact: true }).dblclick();
			const input = grid.getByRole('textbox', { name: `Rename ${oldLabel}`, exact: true });
			await input.fill(label);
			await input.press('Enter');
			const after = await wait(inspect, (state) =>
				state[field].some((entry) => entry.label === label)
			);
			await undo();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await redo();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
		}
		const beforeRemoval = await inspect();
		await grid
			.getByRole('button', { name: 'Remove media from Stage A / Scene A', exact: true })
			.click();
		const removed = await wait(inspect, (state) => state.cells[0]?.media === null);
		assert.deepEqual(removed.cells[0].definition, {
			...beforeRemoval.cells[0].definition,
			source: null
		});
		await undo();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeRemoval));
		assert.equal((await inspect()).cells[0].media, originalCell.media);
		await redo();
		await wait(inspect, (state) => isDeepStrictEqual(state, removed));
		await cell('Stage A', 'Scene A').click({ button: 'right', position: { x: 5, y: 5 } });
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		const opacity = page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Opacity', { exact: true }) })
			.locator('input.number-field');
		await opacity.waitFor();
		await grid.getByTitle('Select Layer 2', { exact: true }).click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Duplicate row', exact: true }).click();
		await wait(inspect, (state) => state.items.length === 4);
		await undo();
		await wait(inspect, (state) => state.items.length === 3);
		await grid.getByRole('button', { name: 'Add group', exact: true }).click();
		const grouped = await wait(inspect, (state) =>
			state.items.some((row) => row.id.kind === 'Group')
		);
		const group = grouped.items.find((row) => row.id.kind === 'Group');
		await grid.getByTitle(`Select ${group.label}`, { exact: true }).click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Add layer in group', exact: true }).click();
		await wait(inspect, (state) => state.items.some((row) => row.parent === group.id.id));
		await cell('Stage A', 'Scene A').click({ button: 'right', position: { x: 5, y: 5 } });
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		await opacity.waitFor();
		await page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Enabled', { exact: true }) })
			.locator('input[type=checkbox]')
			.waitFor();
		await grid.getByTitle(`Select ${group.label}`, { exact: true }).waitFor({ state: 'visible' });
		await page.screenshot({ path: 'target/m07-grid-ui.png' });
		await matrix({ kind: 'AddLayers', parent: null, count: 128 });
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(count) => count === '134'
		);
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.dispatchEvent(new Event('scroll'));
		});
		await wait(
			() => cell('Stage A', 'Scene A').count(),
			(count) => count === 0
		);
		assert((await grid.getByRole('rowheader').count()) < 30, 'bounded virtual rows');
		await opacity.fill('0.375');
		await opacity.press('Tab');
		await wait(
			inspect,
			(state) =>
				state.cells.find((cell) => cell.id === originalCell.id)?.definition.settings.opacity ===
				0.375
		);
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Square full-panel grid; contextual owned-media creation/removal and structural actions; inline rename and exact history; bounded offscreen Cell Inspector; no implicit launch',
				browser: browser.version()
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-grid-ui-failure.png' }).catch(() => {});
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
