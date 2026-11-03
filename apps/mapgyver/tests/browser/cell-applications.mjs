// The actual Matrix action materializes a sparse Cell; generic widgets author its settings.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyCellApplicationsUi({ base, edit }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Cell app review'
	});
	const { items } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'AddLayers', parent: null, count: 1 }
	});
	const { columns } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'AddColumns', count: 5 }
	});
	const inspect = async () =>
		(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } })).snapshot;
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Cell app control');
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
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
		assert.equal((await inspect()).cells.length, 0);
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('region', { name: 'Media Manager', exact: true })
			.getByRole('button', { name: 'Cell app review', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await grid.getByTitle('Layer 1 / Column 3', { exact: true }).click();
		await page.getByRole('button', { name: 'Edit Cell app control', exact: true }).click();
		const state = await wait(inspect, (value) => value.cells.length === 1);
		const cell = state.cells[0];
		assert.equal(cell.layer, items[0].id);
		assert.equal(cell.definition.column, columns[2]);
		assert.equal(cell.media, null);
		assert.equal(cell.definition.application, cell.id);
		await parameter('App file').waitFor();
		assert.equal(await parameter('Window').count(), 0);
		const executable = path.resolve('target/debug/application_process_fixture.exe');
		const input = parameter('App file').locator('input.path-input');
		await input.fill(executable);
		await input.press('Tab');
		const application = async () =>
			(
				await edit({
					kind: 'Applications',
					edit: { kind: 'Inspect', application: cell.definition.application }
				})
			).snapshot;
		await wait(application, (value) => value.definition.executable === executable);
		for (const [label, value] of [
			['Preload columns before', '1'],
			['Keep columns after', '2']
		]) {
			const input = parameter(label).locator('input.number-field');
			await input.fill(value);
			await input.press('Enter');
		}
		await page
			.locator('.inspector')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await page.getByRole('menuitem', { name: 'Argument', exact: true }).click();
		const argument = parameter('Value').locator('input,textarea');
		await argument.fill('literal & value');
		await argument.press('Tab');
		await wait(application, (value) => value.definition.arguments[0] === 'literal & value');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(application, (value) => value.definition.arguments[0] === '');
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(application, (value) => value.definition.arguments[0] === 'literal & value');
		const catalog = await edit({
			kind: 'Applications',
			edit: { kind: 'InspectCatalog', search: '', start: 0, count: 128 }
		});
		assert.equal(catalog.page.total, 0, 'Cell settings are not standalone definitions');
		const runtime = (await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } }))
			.snapshot;
		assert(
			runtime.applications.every((value) => value.process_id === null),
			'Authoring never launches'
		);
		await parameter('App file').getByRole('button', { name: 'Clear', exact: true }).click();
		await wait(application, (value) => value.definition.executable === null);
		await wait(
			() => parameter('Window').count(),
			(count) => count === 0
		);
		await wait(
			() => parameter('Value').count(),
			(count) => count === 0
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(application, (value) => value.definition.executable === executable);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m08-cell-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual sparse Cell action, generic File/pre/post/Argument editing, hidden inactive controls, stable Cell ownership, Undo/Redo and no standalone catalog/process side effects',
				application: cell.definition.application,
				browser: browser.version()
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m08-cell-applications-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-6500)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
