import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyColumnGroupsUi({ page, edit, nativePid, nativeLog }) {
	const directory = path.resolve(process.env.MAPGYVER_GROUP_UI_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const errors = [];
	const receipts = await observeNativeRejectedRequests(page, () => 'Column Group UI');
	page.on('pageerror', (error) => errors.push(String(error)));
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Column Group review'
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	async function wait(read, check) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (check(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Column Group review', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const band = (id) => grid.locator(`[data-column-group-id="${id}"]`);
	async function context(id) {
		await wait(
			() => band(id).getAttribute('draggable'),
			(value) => value === 'true'
		);
		await band(id)
			.getByRole('button')
			.click({ button: 'right', position: { x: 10, y: 10 } });
	}
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	async function drag(source, target, position) {
		await wait(
			() => source.getAttribute('draggable'),
			(value) => value === 'true'
		);
		await source.hover(position ? { position } : {});
		await page.mouse.down();
		try {
			// Two target moves reliably deliver dragover in WebView2 (Playwright input docs).
			await target.hover();
			await target.hover();
		} finally {
			await page.mouse.up();
		}
	}
	await grid.getByRole('button', { name: 'Add Column or Column Group', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Column Group', exact: true }).click();
	let state = await wait(inspect, (s) => s.column_groups.length === 1);
	const group = state.column_groups[0].id;
	assert(state.column_groups[0].empty);
	await band(group).getByRole('button').click();
	await parameter('App file').waitFor();
	assert.equal(await parameter('Window').count(), 0);
	const application = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: group } })).snapshot;
	const file = path.resolve('target/debug/application_process_fixture.exe');
	await parameter('App file').locator('input.path-input').fill(file);
	await parameter('App file').locator('input.path-input').press('Tab');
	await wait(application, (s) => s.definition.executable === file);
	assert.equal(await parameter('Window').locator('select').inputValue(), 'Normal');
	await parameter('Window').locator('select').selectOption('Hidden');
	await wait(application, (s) => s.definition.window === 'Hidden');
	await undo();
	await wait(application, (s) => s.definition.window === 'Normal');
	await redo();
	await wait(application, (s) => s.definition.window === 'Hidden');
	await band(group).getByRole('button').dblclick();
	await band(group).getByRole('textbox').fill('Show Group');
	await band(group).getByRole('textbox').press('Enter');
	await wait(inspect, (s) => s.column_groups[0].label === 'Show Group');
	await context(group);
	await page.getByRole('menuitem', { name: 'Add Column in group', exact: true }).click();
	state = await wait(inspect, (s) => s.columns.some((c) => c.parent === group));
	await context(group);
	await page.getByRole('menuitem', { name: 'Add nested Column Group', exact: true }).click();
	state = await wait(inspect, (s) => s.column_groups.length === 2);
	const nested = state.column_groups.find((g) => g.parent === group).id;
	await band(nested).waitFor();
	const moving = state.columns.find((c) => c.parent === null).id;
	const beforeMove = await inspect();
	await page.getByRole('menu').waitFor({ state: 'hidden' });
	const movingHeader = grid.locator(`[data-matrix-drop="Column"][data-column-id="${moving}"]`);
	await wait(
		() => movingHeader.getAttribute('draggable'),
		(value) => value === 'true'
	);
	await wait(
		() => band(nested).getAttribute('draggable'),
		(value) => value === 'true'
	);
	await drag(movingHeader, band(nested), { x: 16, y: 10 });
	await wait(inspect, (s) => s.columns.find((c) => c.id === moving)?.parent === nested);
	await undo();
	await wait(inspect, (s) => isDeepStrictEqual(s, beforeMove));
	await wait(
		() => band(nested).getAttribute('draggable'),
		(value) => value === 'true'
	);
	await drag(band(nested), grid.locator('.corner'));
	await wait(inspect, (s) => s.column_groups.find((g) => g.id === nested)?.parent === null);
	await undo();
	await wait(inspect, (s) => isDeepStrictEqual(s, beforeMove));
	for (const [action, count] of [
		['Duplicate Column Group', 4],
		['Delete Column Group', 0],
		['Ungroup Columns', 1]
	]) {
		const before = await inspect();
		await context(group);
		await page.getByRole('menuitem', { name: action, exact: true }).click();
		const after = await wait(inspect, (s) => s.column_groups.length === count);
		await undo();
		await wait(inspect, (s) => isDeepStrictEqual(s, before));
		await redo();
		await wait(inspect, (s) => isDeepStrictEqual(s, after));
		await undo();
		await wait(inspect, (s) => isDeepStrictEqual(s, before));
	}
	await band(group).getByRole('button').click();
	await parameter('Window').locator('select').waitFor();
	await matrix({ kind: 'AddColumns', parent: null, count: 1000 });
	await wait(
		() => grid.evaluate((element) => element.scrollWidth),
		(width) => width > 10000
	);
	await grid.evaluate((element) => {
		element.scrollLeft = element.scrollWidth;
		element.dispatchEvent(new Event('scroll'));
	});
	await wait(
		() => band(group).count(),
		(count) => count === 0
	);
	await parameter('Window').locator('select').selectOption('Normal');
	await wait(application, (s) => s.definition.window === 'Normal');
	const mounted = await grid.getByRole('gridcell').count();
	assert(mounted < 200, `bounded sparse lifetime: ${mounted}`);
	assert.equal((await inspect()).cells.length, 0);
	await grid.evaluate((element) => {
		element.scrollLeft = 0;
		element.dispatchEvent(new Event('scroll'));
	});
	await band(group).waitFor();
	assert.equal(
		await page.locator('.user-action-activity.visible[data-phase="rejected"]').count(),
		0
	);
	await page.screenshot({ path: path.join(directory, 'column-groups.png') });
	await writeFile(path.join(directory, 'receipts.json'), JSON.stringify(receipts, null, 2));
	await writeFile(path.join(directory, 'native.log'), nativeLog());
	assert.deepEqual(receipts.rejected, []);
	assert.deepEqual(errors, []);
	const result = {
		result: 'pass',
		nativePid,
		group,
		nested,
		mounted,
		page_errors: errors,
		scope:
			'actual native WebView group creation, typed standard Inspector Normal/Hidden/history, inline rename, nesting, trusted Column/group drag, copy/delete/ungroup history and offscreen Inspector with sparse horizontal virtualization'
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
}
