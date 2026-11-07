// Ordinary native WebView input over canonical Golden Parameter editors.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
const page = native.page;
page.setDefaultTimeout(15000);
let token,
	stage = 'setup';
const errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
const receipts = await observeNativeRejectedRequests(page, () => stage);
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(10000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function wait(read, ready, timeout = 15000) {
	const deadline = Date.now() + timeout;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(40);
	}
}
const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}(?::|$)`) });
const inspector = (label) =>
	page
		.locator('.inspector .parameter-inspector')
		.filter({ has: page.getByText(label, { exact: true }) });
const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Inline Matrix controls'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const { items: groups } = await matrix({ kind: 'AddGroup', parent: null, label: 'Inline Group' });
	const { items: layers } = await matrix({ kind: 'AddLayers', parent: groups[0].id, count: 1 });
	await matrix({
		kind: 'RenameItem',
		item: { kind: 'Layer', id: layers[0].id },
		label: 'Nested inline Layer'
	});
	await tab('Medias').click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Inline Matrix controls', exact: true })
		.click();
	await tab('Matrix').click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const ready = () =>
		wait(
			() => grid.locator('xpath=ancestor::section[@data-panel-id]').getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	const row = (id) => grid.locator(`.row-label[data-row-id="${id}"]`);
	const input = (id) => row(id).locator('[data-matrix-parameter="opacity"] input.number-field');
	const blend = (id) => row(id).locator('[data-matrix-parameter="blend"] select');
	const settings = async (id) => (await inspect()).items.find((item) => item.id.id === id).settings;
	const original = await inspect();
	for (const target of [
		original.items[0],
		original.items.find((item) => item.id.id === groups[0].id),
		original.items.find((item) => item.id.id === layers[0].id)
	]) {
		stage = `inline editing ${target.label}`;
		await input(target.id.id).waitFor();
		await ready();
		await row(target.id.id).click({ position: { x: 6, y: 6 } });
		await wait(
			() => row(target.id.id).getAttribute('aria-selected'),
			(v) => v === 'true'
		);
		await inspector('Opacity').locator('input.number-field').waitFor();
		assert.equal(
			await row(target.id.id)
				.locator('[data-matrix-parameter="opacity"]')
				.getAttribute('data-parameter-id'),
			await inspector('Opacity').getAttribute('data-node-id')
		);
		assert.equal(
			await row(target.id.id)
				.locator('[data-matrix-parameter="blend"]')
				.getAttribute('data-parameter-id'),
			await inspector('Blend Mode').getAttribute('data-node-id')
		);
		const geometry = await row(target.id.id).evaluate((element) => {
			const header = element.getBoundingClientRect(),
				title = element.querySelector('.row-title').getBoundingClientRect();
			return {
				offset: title.top - header.top,
				rem: parseFloat(getComputedStyle(document.documentElement).fontSize)
			};
		});
		assert(geometry.offset < geometry.rem, 'Label is at the top of its header');
		const before = await inspect();
		await input(target.id.id).fill('0.375');
		await input(target.id.id).press('Enter');
		await wait(
			() => settings(target.id.id),
			(v) => v.opacity === 0.375
		);
		await wait(
			() => inspector('Opacity').locator('input.number-field').inputValue(),
			(v) => Number(v) === 0.375
		);
		const after = await inspect();
		await undo();
		await wait(inspect, (v) => isDeepStrictEqual(v, before));
		await redo();
		await wait(inspect, (v) => isDeepStrictEqual(v, after));
		for (const mode of ['Add', 'Multiply', 'Screen', 'Normal']) {
			await blend(target.id.id).selectOption(mode);
			await wait(
				() => settings(target.id.id),
				(v) => v.blend === mode
			);
			await wait(
				() => inspector('Blend Mode').locator('select').inputValue(),
				(v) => v === mode
			);
		}
		await inspector('Opacity').locator('input.number-field').fill('0.625');
		await inspector('Opacity').locator('input.number-field').press('Enter');
		await wait(
			() => input(target.id.id).inputValue(),
			(v) => Number(v) === 0.625
		);
		checks.push({
			kind: target.id.kind,
			id: target.id.id,
			canonical_parameters: true,
			history: true,
			inspector_sync: true,
			label_top: true
		});
	}
	const target = original.items[0];
	stage = 'slider gesture is one edit and never a row drag';
	await ready();
	await row(target.id.id).focus();
	await row(target.id.id).press(' ');
	await wait(
		() => row(target.id.id).getAttribute('aria-selected'),
		(v) => v === 'true'
	);
	const before = await inspect();
	const slider = row(target.id.id).getByRole('slider');
	const bounds = await slider.boundingBox();
	assert(bounds);
	await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
	await page.mouse.down();
	await page.mouse.move(bounds.x + bounds.width / 2 + 20, bounds.y + bounds.height / 2, {
		steps: 8
	});
	await page.mouse.up();
	await wait(
		() => settings(target.id.id),
		(v) => v.opacity !== before.items[0].settings.opacity
	);
	await ready();
	assert.deepEqual(
		(await inspect()).items.map((v) => v.id),
		before.items.map((v) => v.id)
	);
	const dragged = await inspect();
	await undo();
	await wait(inspect, (v) => isDeepStrictEqual(v, before));
	await redo();
	await wait(inspect, (v) => isDeepStrictEqual(v, dragged));
	await ready();
	stage = 'Column blank area and keyboard selection preserve launch';
	const column = grid.locator(`[role="columnheader"][data-column-id="${original.columns[0].id}"]`);
	const blank = await column.evaluate((element) => {
		const rect = element.getBoundingClientRect();
		for (let y = rect.top + 2; y < rect.bottom - 1; y += 2) {
			for (let x = rect.right - 2; x > rect.left; x -= 2) {
				if (document.elementFromPoint(x, y) === element) return { x, y };
			}
		}
		return null;
	});
	assert(blank, 'Column header has a clickable background');
	await page.mouse.click(blank.x, blank.y);
	await wait(
		() => column.getAttribute('aria-selected'),
		(v) => v === 'true'
	);
	const second = grid.locator(`[role="columnheader"][data-column-id="${original.columns[1].id}"]`);
	await ready();
	await second.focus();
	await second.press('Enter');
	await wait(
		() => second.getAttribute('aria-selected'),
		(v) => v === 'true'
	);
	await column
		.getByRole('button', { name: `Launch ${original.columns[0].label}`, exact: true })
		.click();
	await wait(
		async () => (await matrix({ kind: 'InspectPlayback' })).snapshot,
		(v) => v.desired_column === original.columns[0].id
	);
	await ready();
	stage = 'rename keeps generic inputs and canonical identities';
	await row(target.id.id).getByTitle(`Select ${target.label}`, { exact: true }).dblclick();
	const rename = row(target.id.id).getByRole('textbox', {
		name: `Rename ${target.label}`,
		exact: true
	});
	await rename.fill('Renamed inline Layer');
	await rename.press('Enter');
	await wait(inspect, (v) => v.items[0].label === 'Renamed inline Layer');
	await row(target.id.id).getByTitle('Select Renamed inline Layer', { exact: true }).waitFor();
	await page.screenshot({ path: path.join(directory, 'inline-controls.png') });
	stage = 'sparse1000 by1000 and selected offscreen Inspector';
	await matrix({ kind: 'AddLayers', parent: null, count: 995 });
	await matrix({ kind: 'AddColumns', count: 997 });
	await wait(
		() => grid.getAttribute('aria-rowcount'),
		(v) => v === '1002',
		30000
	);
	await ready();
	await row(target.id.id).focus();
	await row(target.id.id).press('Enter');
	await inspector('Opacity').locator('input.number-field').waitFor();
	await grid.hover();
	await page.mouse.wheel(0, 1000000);
	await wait(
		() => row(target.id.id).count(),
		(v) => v === 0,
		30000
	);
	const mounted = await grid.locator('[data-matrix-row-controls]').count();
	assert(mounted <= 64);
	await inspector('Opacity').locator('input.number-field').fill('0.25');
	await inspector('Opacity').locator('input.number-field').press('Enter');
	await wait(
		() => settings(target.id.id),
		(v) => v.opacity === 0.25
	);
	await grid.hover();
	await page.mouse.wheel(0, -1000000);
	await wait(
		() => input(target.id.id).inputValue(),
		(v) => Number(v) === 0.25,
		30000
	);
	const sparse = await inspect();
	assert.equal(sparse.items.length, 1000);
	assert.equal(sparse.columns.length, 1000);
	assert.equal(sparse.cells.length, 0);
	stage = 'save and same-ID reload';
	const project = path.join(directory, 'Inline.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	const loaded = await post('/api/ui/project-load', { path: project });
	assert.equal(path.resolve(loaded.path), project);
	assert.equal(loaded.recovery ?? null, null);
	await wait(inspect, (v) => isDeepStrictEqual(v.items, sparse.items), 30000);
	await wait(
		() => input(target.id.id).inputValue(),
		(v) => Number(v) === 0.25,
		30000
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	assert(receipts.observed.requests > 10 && receipts.observed.receipts > 0);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				checks,
				sparse: { rows: 1000, columns: 1000, cells: 0, mounted_controls: mounted },
				slider_grouped_history: true,
				header_keyboard: true,
				rename: true,
				offscreen_inspector: true,
				reload: true,
				errors,
				receipts
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ passed: true, pid: native.app.pid, checks: checks.length }));
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				stage,
				error: String(error),
				stack: error.stack,
				checks,
				errors,
				receipts,
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
