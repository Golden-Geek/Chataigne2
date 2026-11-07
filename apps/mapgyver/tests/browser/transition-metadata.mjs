// Fresh bundled UI acceptance for the actual typed Transition Time Parameters.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	token,
	stage = 'setup';
const errors = [],
	checks = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(8000),
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
async function wait(read, accept) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, stage + ': ' + JSON.stringify(value));
		await delay(25);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Transition metadata'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect(),
		row = initial.items[0].id.id,
		column = initial.columns[0].id;
	await matrix({
		kind: 'ConfigureCell',
		layer: row,
		column,
		settings: { position: [0, 0], size: null, fit_mode: 'Fit', opacity: 1, enabled: true }
	});
	const cell = (await inspect()).cells[0].id;
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(8000);
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	await page.goto(host.base);
	await page.getByRole('tab').first().click();
	const manager = page.locator('section[data-panel-id="media-manager"]');
	const inspector = page.locator('.inspector[data-panel-id="inspector"]');
	const parameter = (label) =>
		inspector
			.locator('.parameter-inspector')
			.filter({ has: page.getByText(label, { exact: true }) });
	const time = parameter('Transition Time');
	const toggle = () => time.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	const input = () => time.locator('input.number-field');
	async function select(uuid, query) {
		await page.getByRole('tab').first().click();
		await manager.getByPlaceholder('Search media…').fill(query);
		const node = (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes.find(
			(node) => node.uuid === uuid
		);
		assert(node);
		await manager
			.locator(`[data-tree-node-id="${node.node_id}"]`)
			.getByRole('button', { name: node.meta.label, exact: true })
			.click();
		await wait(
			() => inspector.getAttribute('data-inspected-uuid'),
			(value) => value === uuid
		);
		await time.waitFor();
		assert.equal(await parameter('Override transition').count(), 0);
	}
	for (const [owner, uuid, query, duration, formatted] of [
		['Column', column, 'Column 1', 1234, '00:00:01.234'],
		['Cell', cell, 'Cell', 0, '00:00:00.000']
	]) {
		stage = owner;
		await select(uuid, query);
		assert.equal(await toggle().getAttribute('aria-checked'), 'false');
		assert.equal(await parameter('Transition curve').locator('select').inputValue(), 'Smooth');
		await toggle().press('Space');
		await wait(
			() => toggle().getAttribute('aria-checked'),
			(value) => value === 'true'
		);
		await input().fill(formatted);
		await input().press('Enter');
		const transition = async () => {
			const state = await inspect();
			return owner === 'Column'
				? state.columns.find((item) => item.id === uuid).definition.transition
				: state.cells.find((item) => item.id === uuid).definition.transition;
		};
		await wait(transition, (value) => value?.duration_ms === duration && value.curve === 'Smooth');
		await toggle().click();
		await wait(transition, (value) => value === null);
		assert.equal(await input().inputValue(), formatted);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(transition, (value) => value?.duration_ms === duration);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(transition, (value) => value === null);
		await toggle().click();
		await wait(transition, (value) => value?.duration_ms === duration);
		await parameter('Transition curve').locator('select').selectOption('Linear');
		await wait(transition, (value) => value?.curve === 'Linear');
		checks.push({
			owner,
			duration,
			optional_toggle: true,
			dormant_value: true,
			keyboard: true,
			undo_redo: true,
			curves: true
		});
	}
	stage = 'Layer';
	await select(row, 'Layer 1');
	assert.equal(await input().inputValue(), '00:00:00.250');
	await input().fill('00:00:00.700');
	await input().press('Enter');
	await wait(inspect, (state) => state.items[0].layer.duration_ms === 700);
	checks.push({ owner: 'Layer', base: 250, authored: 700 });
	stage = 'project roundtrip';
	await select(column, 'Column 1');
	await toggle().click();
	await wait(inspect, (state) => state.columns[0].definition.transition === null);
	const saved = await inspect(),
		file = path.join(directory, 'transition-metadata.mapgyver');
	assert((await post('/api/ui/project-save', { path: file })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	await post('/api/ui/project-load', { path: file });
	assert.deepEqual(await inspect(), saved);
	await select(column, 'Column 1');
	assert.equal(await toggle().getAttribute('aria-checked'), 'false');
	assert.equal(await input().inputValue(), '00:00:01.234');
	assert.equal(await parameter('Transition curve').locator('select').inputValue(), 'Linear');
	await select(cell, 'Cell');
	assert.equal(await input().inputValue(), '00:00:00.000');
	assert.equal(await toggle().getAttribute('aria-checked'), 'true');
	assert.equal(await parameter('Transition curve').locator('select').inputValue(), 'Linear');
	checks.push({ project_roundtrip: true, metadata_and_values: true });
	if (process.argv.includes('--time-drag')) {
		stage = 'time drag';
		const { verifyTimeDragging } = await import('./time-drag.mjs');
		checks.push(
			...(await verifyTimeDragging({
				page,
				inspect,
				edit,
				post,
				select,
				parameter,
				column,
				cell,
				row,
				wait
			}))
		);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	await page.screenshot({ path: path.join(directory, 'inspector.png') });
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ checks, errors, rejected }, null, 2)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	console.error(JSON.stringify({ stage, errors, tail: host.log().slice(-1500) }));
	if (browser)
		await browser
			.contexts()[0]
			?.pages()[0]
			?.screenshot({ path: path.join(directory, 'failure.png') })
			.catch(() => {});
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
