import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixDefaultsUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => '3x3 Matrix creation');
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
		const manager = page.getByRole('region', { name: 'Media Manager', exact: true });
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name: 'Matrix', exact: true }).click();
		const media = (
			await wait(inspectMedia, (state) =>
				state.media.some((media) => media.media_kind === 'Matrix')
			)
		).media.find((media) => media.media_kind === 'Matrix');
		const inspect = async () =>
			(await edit({ kind: 'Matrix', media: media.id, edit: { kind: 'Inspect' } })).snapshot;
		const original = await inspect();
		assert.equal(original.items.length, 3);
		assert.equal(original.columns.length, 3);
		assert.equal(original.cells.length, 0);
		assert.deepEqual(
			original.items.map((row) => row.label),
			['Layer 1', 'Layer 2', 'Layer 3']
		);
		assert.deepEqual(
			original.columns.map((column) => column.label),
			['Column 1', 'Column 2', 'Column 3']
		);
		assert.equal(media.channels.length, 4);
		await manager.getByRole('button', { name: media.label, exact: true }).click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(count) => count === '4'
		);
		assert.equal(await grid.getAttribute('aria-colcount'), '4');
		assert.equal(await grid.getByRole('gridcell').count(), 9);
		for (const column of original.columns)
			await grid.getByRole('button', { name: `Launch ${column.label}`, exact: true }).waitFor();
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspectMedia, (state) => !state.media.some((entry) => entry.id === media.id));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspectMedia, (state) => state.media.some((entry) => entry.id === media.id));
		assert.deepEqual(await inspect(), original);
		assert.equal(
			(await edit({ kind: 'Matrix', media: media.id, edit: { kind: 'InspectPlayback' } })).snapshot
				.desired_column,
			null
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m07-defaults-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Actual catalog creation, 3x3 grid, sparse addresses, four outputs, one-step Undo/Redo with stable identities and no launch'
			})
		);
	} finally {
		await browser.close();
	}
}
