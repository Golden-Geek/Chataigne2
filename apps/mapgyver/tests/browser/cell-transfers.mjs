// Actual bundled grid input in headless Chromium; no desktop input or native timing claim.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { prepareCellScrollTransfers, verifyCellScrollTransfers } from './cell-scroll-transfers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	token,
	stage = 'setup';
const checks = [],
	requests = [],
	errors = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function operation(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return { value: ack.result.value, history: ack.history };
}
async function wait(read, ready) {
	const deadline = Date.now() + 6500;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const matrix = (
		await operation({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Grid Cell Transfers' })
	).value.reference;
	const other = (
		await operation({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Cross Matrix Transfers' })
	).value.reference;
	const edit = (value) => operation({ kind: 'Matrix', media: matrix.media, edit: value });
	const read = async (media = matrix.media) => {
		const result = await operation({ kind: 'Matrix', media, edit: { kind: 'Inspect' } });
		return { value: result.value.snapshot, history: result.history };
	};
	const initial = (await read()).value,
		otherInitial = (await read(other.media)).value;
	assert.equal(initial.items.length, 3);
	assert.equal(initial.columns.length, 3);
	const rows = initial.items.map((item) => item.id.id),
		columns = initial.columns.map((column) => column.id);
	const a = (
		await edit({
			kind: 'CreateCellMedia',
			layer: rows[0],
			column: columns[0],
			media_kind: 'SolidColor'
		})
	).value.cell;
	const b = (
		await edit({
			kind: 'CreateCellMedia',
			layer: rows[1],
			column: columns[1],
			media_kind: 'SolidColor'
		})
	).value.cell;
	const prepared = (await edit({ kind: 'EnsureCell', layer: rows[2], column: columns[0] })).value
		.cell;
	const external = (
		await operation({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'External Cell Source'
		})
	).value.reference;
	const sparse = await prepareCellScrollTransfers(operation);
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6500);
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				const edit = frame.intent?.payload?.edit;
				if (edit && ['TransferCell', 'DropSource'].includes(edit.kind))
					requests.push({ stage, ...edit });
			} catch {}
		})
	);
	await page.goto(host.base);
	const tab = (name) =>
		page.getByRole('tab').filter({ hasText: new RegExp('^' + name + '(?::|$)') });
	async function show(label, media) {
		await tab('Medias').click();
		await page.getByRole('button', { name: label, exact: true }).click();
		await tab('Matrix').click();
		await wait(
			() => page.locator('section[data-matrix-target]').getAttribute('data-matrix-target'),
			(value) => value === media
		);
	}
	await show('Grid Cell Transfers', matrix.media);
	const cell = (row, column) =>
		page.locator(
			`[data-cell-drop][data-cell-layer="${row}"][data-cell-column="${column}"] button.cell`
		);
	const source = (id) => page.locator(`button.cell[data-cell-id="${id}"]`);
	const position = (value, id) => value.cells.find((cell) => cell.id === id);
	const at = (value, id, row, column) => {
		const cell = position(value, id);
		return cell?.layer === row && cell.definition.column === column;
	};
	async function ready() {
		const canonical = (await read()).value;
		for (const item of canonical.cells) {
			await wait(
				() => cell(item.layer, item.definition.column).getAttribute('data-cell-id'),
				(value) => value === item.id
			);
		}
		await wait(() => source(a).isEnabled(), Boolean);
	}
	async function drag(id, row, column, { alt = false, cancel = false } = {}) {
		await ready();
		const from = await source(id).boundingBox(),
			to = await cell(row, column).boundingBox();
		assert(from && to);
		// Keep input on the Cell body, away from its centered Add and top trigger actions.
		const start = { x: from.x + from.width / 4, y: from.y + from.height * 0.65 };
		await page.mouse.move(start.x, start.y);
		await page.mouse.down();
		await page.mouse.move(start.x + 12, start.y + 2, { steps: 3 });
		if (alt) await page.keyboard.down('Alt');
		await page.mouse.move(to.x + to.width / 2 + 2, to.y + to.height / 2 + 2, { steps: 8 });
		await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2);
		if (cancel) await page.keyboard.press('Escape');
		await page.mouse.up();
		if (alt) await page.keyboard.up('Alt');
	}
	async function clickBody(locator, options = {}) {
		const box = await locator.boundingBox();
		assert(box);
		await locator.click({ ...options, position: { x: box.width / 4, y: box.height * 0.65 } });
	}
	async function history(before, after, media = matrix.media) {
		assert.equal(after.history.undo_len, before.history.undo_len + 1);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => read(media),
			(value) => JSON.stringify(value.value) === JSON.stringify(before.value)
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => read(media),
			(value) => JSON.stringify(value.value) === JSON.stringify(after.value)
		);
	}
	stage = 'normal occupied swap';
	let before = await read();
	await drag(a, rows[1], columns[1]);
	let after = await wait(
		read,
		(value) => at(value.value, a, rows[1], columns[1]) && at(value.value, b, rows[0], columns[0])
	);
	await history(before, after);
	checks.push({ stage, exact_history: true });
	stage = 'normal empty move';
	before = await read();
	await drag(a, rows[2], columns[2]);
	after = await wait(read, (value) => at(value.value, a, rows[2], columns[2]));
	await history(before, after);
	checks.push({ stage, exact_history: true });
	const created = {};
	stage = 'same-address drop';
	before = await read();
	const sameCount = requests.length;
	await drag(a, rows[2], columns[2]);
	await wait(
		() => requests.length,
		(value) => value === sameCount + 1
	);
	assert.deepEqual((await read()).value, before.value);
	assert.equal((await read()).history.undo_len, before.history.undo_len);
	checks.push({ stage, exact_noop: true });
	for (const [action, row, column] of [
		['Copy', rows[0], columns[1]],
		['Reference', rows[0], columns[2]]
	]) {
		stage = `Alt ${action}`;
		before = await read();
		const count = requests.length;
		await drag(a, row, column, { alt: true });
		await page.getByRole('menuitem', { name: new RegExp('^' + action + '\\b') }).waitFor();
		assert.deepEqual((await read()).value, before.value);
		assert.equal(requests.length, count);
		await page.getByRole('menuitem', { name: new RegExp('^' + action + '\\b') }).click();
		after = await wait(read, (value) => value.value.cells.length === before.value.cells.length + 1);
		const result = after.value.cells.find(
			(cell) => !before.value.cells.some((old) => old.id === cell.id)
		);
		assert(result);
		created[action] = result.id;
		const original = position(before.value, a);
		assert.deepEqual(result.definition.settings, original.definition.settings);
		assert.equal(
			result.definition.source.channel === original.definition.source.channel,
			action === 'Reference'
		);
		assert.equal(
			result.definition.application,
			action === 'Reference' ? original.definition.application : result.id
		);
		await history(before, after);
		checks.push({ stage, deferred_choice: true, ownership: true, exact_history: true });
	}
	stage = 'Alt explicit Move';
	before = await read();
	await drag(created.Copy, rows[1], columns[0], { alt: true });
	await page.getByRole('menuitem', { name: /^Move\b/ }).click();
	after = await wait(read, (value) => at(value.value, created.Copy, rows[1], columns[0]));
	await history(before, after);
	checks.push({ stage, exact_history: true });
	for (const action of ['Move', 'Copy', 'Reference']) {
		stage = `occupied ${action} rejection`;
		before = await read();
		await drag(a, rows[0], columns[0], { alt: true });
		await page.getByRole('menuitem', { name: new RegExp('^' + action + '\\b') }).click();
		await page
			.getByRole('alert')
			.filter({ hasText: /occupied/i })
			.waitFor();
		assert.deepEqual((await read()).value, before.value);
		assert.equal((await read()).history.undo_len, before.history.undo_len);
		checks.push({ stage, no_partial_state: true });
	}
	stage = 'Alt menu cancellation';
	before = await read();
	let count = requests.length;
	await drag(a, rows[1], columns[2], { alt: true });
	await page.getByRole('menuitem', { name: /^Move\b/ }).waitFor();
	await page.keyboard.press('Escape');
	assert.deepEqual((await read()).value, before.value);
	assert.equal(requests.length, count);
	checks.push({ stage, no_intent: true });
	stage = 'drag Escape cancellation';
	before = await read();
	count = requests.length;
	await drag(a, rows[1], columns[2], { cancel: true });
	assert.deepEqual((await read()).value, before.value);
	assert.equal(requests.length, count);
	checks.push({ stage, no_intent: true });
	stage = 'prepared Cell move';
	before = await read();
	await drag(prepared, rows[2], columns[1]);
	after = await wait(read, (value) => at(value.value, prepared, rows[2], columns[1]));
	assert.equal(position(after.value, prepared).media, null);
	await history(before, after);
	checks.push({ stage, exact_history: true });
	stage = 'keyboard Copy Paste';
	before = await read();
	await source(created.Copy).click();
	await page.keyboard.press('Control+c');
	await clickBody(cell(rows[1], columns[2]));
	await wait(() => cell(rows[1], columns[2]).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	after = await wait(read, (value) => value.value.cells.length === before.value.cells.length + 1);
	const keyboard = after.value.cells.find(
		(cell) => !before.value.cells.some((old) => old.id === cell.id)
	).id;
	await history(before, after);
	checks.push({ stage, exact_history: true });
	stage = 'keyboard rejected Cut then Move';
	await source(keyboard).click();
	await page.keyboard.press('Control+x');
	before = await read();
	await clickBody(cell(rows[0], columns[0]));
	await wait(() => cell(rows[0], columns[0]).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	await page
		.getByRole('alert')
		.filter({ hasText: /occupied/i })
		.waitFor();
	assert.deepEqual((await read()).value, before.value);
	await clickBody(cell(rows[0], columns[1]));
	await wait(() => cell(rows[0], columns[1]).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	after = await wait(read, (value) => at(value.value, keyboard, rows[0], columns[1]));
	await history(before, after);
	checks.push({ stage, retained_cut: true, exact_history: true });
	stage = 'successful Cut clears clipboard';
	before = await read();
	count = requests.length;
	await clickBody(cell(rows[1], columns[2]));
	await wait(() => cell(rows[1], columns[2]).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	assert.deepEqual((await read()).value, before.value);
	assert.equal(requests.length, count);
	checks.push({ stage, no_intent: true });
	stage = 'context Reference paste';
	await source(created.Reference).click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^Copy Cell\b/ }).click();
	before = await read();
	await clickBody(cell(rows[2], columns[0]), { button: 'right' });
	await page.getByRole('menuitem', { name: 'Paste Cell as Reference', exact: true }).click();
	after = await wait(read, (value) => value.value.cells.length === before.value.cells.length + 1);
	await history(before, after);
	checks.push({ stage, exact_history: true });
	stage = 'cross-Matrix Reference keyboard paste';
	await show('Cross Matrix Transfers', other.media);
	const otherRows = otherInitial.items.map((item) => item.id.id),
		otherColumns = otherInitial.columns.map((column) => column.id);
	before = await read(other.media);
	await clickBody(cell(otherRows[0], otherColumns[0]));
	await wait(() => cell(otherRows[0], otherColumns[0]).isEnabled(), Boolean);
	await page.keyboard.press('Control+Alt+v');
	after = await wait(
		() => read(other.media),
		(value) => value.value.cells.length === 1
	);
	await history(before, after, other.media);
	checks.push({ stage, exact_history: true });
	stage = 'cross-Matrix Copy keyboard paste';
	await show('Grid Cell Transfers', matrix.media);
	await source(created.Copy).click();
	await page.keyboard.press('Control+c');
	await show('Cross Matrix Transfers', other.media);
	before = await read(other.media);
	await clickBody(cell(otherRows[1], otherColumns[1]));
	await wait(() => cell(otherRows[1], otherColumns[1]).isEnabled(), Boolean);
	await page.keyboard.press('Control+v');
	after = await wait(
		() => read(other.media),
		(value) => value.value.cells.length === 2
	);
	await history(before, after, other.media);
	checks.push({ stage, exact_history: true });
	await show('Grid Cell Transfers', matrix.media);
	for (const [copy, column] of [
		[false, columns[1]],
		[true, columns[2]]
	]) {
		stage = copy
			? 'external media Alt-copy remains distinct'
			: 'external media default Reference remains distinct';
		before = await read();
		await ready();
		const label = page
			.locator('section[aria-label="Medias"]')
			.getByRole('button', { name: 'External Cell Source', exact: true });
		assert.equal(await label.getAttribute('draggable'), 'true');
		const from = await label.boundingBox(),
			to = await cell(rows[1], column).boundingBox();
		assert(from && to);
		await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
		await page.mouse.down();
		await page.mouse.move(from.x + from.width / 2 + 12, from.y + from.height / 2 + 2, { steps: 3 });
		if (copy) await page.keyboard.down('Alt');
		await page.mouse.move(to.x + to.width / 4 + 2, to.y + to.height * 0.65, { steps: 8 });
		await page.mouse.move(to.x + to.width / 4, to.y + to.height * 0.65);
		await page.mouse.up();
		if (copy) await page.keyboard.up('Alt');
		after = await wait(read, (value) => value.value.cells.length === before.value.cells.length + 1);
		const inserted = after.value.cells.find(
			(item) => !before.value.cells.some((old) => old.id === item.id)
		);
		assert(inserted);
		assert.equal(inserted.definition.source.channel === external.channel, !copy);
		assert.equal(await page.getByRole('menuitem', { name: /^Move\b/ }).count(), 0);
		const delivered = requests.filter((request) => request.stage === stage);
		assert.equal(delivered.length, 1);
		assert.equal(delivered[0].kind, 'DropSource');
		assert.equal(delivered[0].copy, copy);
		await history(before, after);
		checks.push({ stage, exact_history: true, external_policy: true });
	}
	await verifyCellScrollTransfers({
		scene: sparse,
		page,
		read,
		wait,
		show,
		history,
		checks,
		setStage: (value) => {
			stage = value;
		}
	});
	assert.deepEqual(errors, []);
	assert(requests.some((value) => value.action === 'Drop'));
	assert(requests.some((value) => value.action === 'Move'));
	assert(requests.some((value) => value.action === 'Copy'));
	assert(requests.some((value) => value.action === 'Reference'));
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: host.app.pid,
				checks,
				requests,
				errors,
				headless: true,
				native_pixels: false
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, requests, errors }, null, 2)
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
	await writeFile(path.join(directory, 'host.log'), host.log());
}
