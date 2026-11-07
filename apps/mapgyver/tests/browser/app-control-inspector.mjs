// Actual bundled Inspector: authored controls survive without per-Inspector runtime readers.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { clearMatrixScene } from './matrix-scene.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	token,
	stage = 'setup';
const checks = [],
	errors = [],
	sent = [];
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
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
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
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
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
		label: 'Authored control scene'
	});
	await clearMatrixScene(edit, reference.media);
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const layer = (await matrix({ kind: 'AddLayers', parent: null, count: 1 })).items[0].id;
	const { columns } = await matrix({ kind: 'AddColumns', parent: null, count: 80 });
	await matrix({ kind: 'RenameColumn', column: columns[0], label: 'Control owner A' });
	await matrix({ kind: 'RenameColumn', column: columns[1], label: 'Control owner B' });
	const { group } = await matrix({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Control owner group'
	});
	await matrix({
		kind: 'MoveColumnItem',
		item: { kind: 'Column', id: columns.at(-1) },
		parent: group,
		after: null
	});
	await matrix({
		kind: 'ConfigureCell',
		layer,
		column: columns[0],
		settings: {
			position: [0, 0],
			size: null,
			fit_mode: 'Fit',
			opacity: 1,
			enabled: true
		}
	});
	const cell = (await matrix({ kind: 'Inspect' })).snapshot.cells[0].id;
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(8000);
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent') sent.push(frame.intent);
				if (frame.kind === 'intentBatch') sent.push(...frame.intents);
			} catch {}
		})
	);
	const rejected = observeRejectedRequests(page, () => stage);
	await page.goto(host.base);
	await page.getByRole('tab').first().click();
	const manager = page.locator('section[data-panel-id="media-manager"]');
	const search = manager.getByPlaceholder('Search media…');
	const inspector = page.locator('.inspector[data-panel-id="inspector"]');
	const parameter = (label) =>
		inspector
			.locator('.parameter-inspector')
			.filter({ has: page.getByText(label, { exact: true }) });
	const app = async (application) =>
		(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application } })).snapshot;
	async function row(uuid) {
		const node = (await snapshot()).nodes.find((node) => node.uuid === uuid);
		assert(node, uuid);
		return { node, row: manager.locator(`[data-tree-node-id="${node.node_id}"]`) };
	}
	async function select(uuid, query) {
		await page.getByRole('tab').first().click();
		await search.fill(query);
		const target = await row(uuid);
		await target.row.getByRole('button', { name: target.node.meta.label, exact: true }).click();
		await wait(
			() => inspector.getAttribute('data-inspected-uuid'),
			(value) => value === uuid
		);
		await parameter('App file').waitFor();
		return target;
	}
	async function clean() {
		assert.equal(await inspector.getByText('Runtime', { exact: true }).count(), 0);
		assert.equal(
			await inspector.getByRole('region', { name: 'Application runtime', exact: true }).count(),
			0
		);
		assert.equal(
			await inspector.getByRole('button', { name: 'Refresh runtime status', exact: true }).count(),
			0
		);
		assert(
			!/configuration in use|runtime reconciliation|Application supervision/.test(
				await inspector.innerText()
			)
		);
	}
	const executable = path.resolve('target/debug/application_process_fixture.exe');
	const cases = [
		['Column', columns[0], 'Control owner A'],
		['Cell', cell, 'Cell'],
		['ColumnGroup', group, 'Control owner group']
	];
	for (const [kind, uuid, query] of cases) {
		stage = kind + ' empty';
		await select(uuid, query);
		await clean();
		assert.equal(await parameter('Window').count(), 0);
		const file = parameter('App file').locator('input.path-input');
		const address = await parameter('App file').getAttribute('data-node-id');
		await file.fill(executable);
		await file.press('Tab');
		await wait(
			() => app(uuid),
			(value) => value.definition.executable === executable
		);
		stage = kind + ' configured';
		await parameter('Window').waitFor();
		await clean();
		await parameter('Ready when').locator('select').selectOption('Spout');
		await parameter('Spout sender').waitFor();
		await parameter('Ready when').locator('select').selectOption('ProcessStarted');
		await wait(
			() => parameter('Spout sender').count(),
			(value) => value === 0
		);
		const graph = await snapshot();
		const owner = graph.nodes.find((node) => node.uuid === uuid);
		const control = owner.children
			.map((id) => graph.nodes.find((node) => node.node_id === id))
			.find((node) => node.node_type === 'mapgyver.app_control.v1');
		assert(control);
		await inspector
			.locator(`.node-inspector[data-node-id="${control.node_id}"] > .node-header > .node-title`)
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Watch in Inspector', exact: true }).click();
		await wait(
			() => inspector.getAttribute('data-inspected-uuid'),
			(value) => value === control.uuid
		);
		await clean();
		assert.equal(await parameter('App file').getAttribute('data-node-id'), address);
		await parameter('App file').getByRole('button', { name: 'Clear', exact: true }).click();
		await wait(
			() => app(uuid),
			(value) => value.definition.executable === null
		);
		await wait(
			() => parameter('Window').count(),
			(value) => value === 0
		);
		await clean();
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => app(uuid),
			(value) => value.definition.executable === executable
		);
		await parameter('Window').waitFor();
		checks.push({
			kind,
			owner: uuid,
			empty: true,
			configured: true,
			conditional: true,
			generic_direct: true,
			clear_undo: true
		});
	}
	stage = 'mixed selection';
	await select(columns[0], 'Control owner');
	const target = await row(group);
	await target.row
		.getByRole('button', { name: target.node.meta.label, exact: true })
		.click({ modifiers: ['Control'] });
	await wait(
		() => manager.locator('[role="treeitem"][aria-selected="true"]').count(),
		(value) => value === 2
	);
	await clean();
	checks.push({ mixed_selection: true });
	stage = 'offscreen selected Column';
	await select(columns[0], 'Control owner A');
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const viewport = grid;
	await viewport.evaluate((element) => {
		element.scrollLeft = element.scrollWidth;
	});
	await wait(
		() => grid.locator(`[role="columnheader"][data-column-id="${columns[0]}"]`).count(),
		(value) => value === 0
	);
	assert.equal(await inspector.getAttribute('data-inspected-uuid'), columns[0]);
	await parameter('App file')
		.locator('input.path-input')
		.fill(executable + '.missing.exe');
	await parameter('App file').locator('input.path-input').press('Tab');
	await wait(
		() => app(columns[0]),
		(value) => value.definition.executable === executable + '.missing.exe'
	);
	await clean();
	checks.push({ offscreen_column: true, live_generic_edit: true });
	stage = 'disabled owner';
	const enabled = inspector
		.getByRole('switch', { name: 'Toggle node enabled', exact: true })
		.first();
	await enabled.click();
	await wait(
		() => app(columns[0]),
		(value) => value.enablement === 'DisabledByParent'
	);
	await parameter('App file').waitFor();
	await clean();
	await enabled.click();
	await wait(
		() => app(columns[0]),
		(value) => value.enablement === 'Enabled'
	);
	checks.push({ disabled_owner: true, authored_fields_preserved: true });
	const selectionRuntimeReads = sent.filter(
		(intent) =>
			intent.payload?.kind === 'Applications' && intent.payload.edit?.kind === 'InspectRuntime'
	).length;
	assert(
		selectionRuntimeReads <= 2,
		'one workbench bootstrap; Inspector selection never mounts more readers'
	);
	stage = 'project replacement';
	const document = path.join(directory, 'authored-inspectors.mapgyver');
	assert((await post('/api/ui/project-save', { path: document })).ok);
	assert((await post('/api/ui/project-new', {})).ok);
	await post('/api/ui/project-load', { path: document });
	await select(group, 'Control owner group');
	assert.equal(await parameter('App file').locator('input.path-input').inputValue(), executable);
	await clean();
	checks.push({ save_new_load: true });
	stage = 'headless Group process lifecycle';
	const configuration = (await app(group)).definition;
	await edit({
		kind: 'Applications',
		edit: {
			kind: 'Configure',
			application: group,
			definition: { ...configuration, window: 'Hidden' }
		}
	});
	const runtime = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } })).snapshot;
	await matrix({ kind: 'LaunchColumn', column: columns.at(-1), mode: 'Cut' });
	const live = await wait(runtime, (value) =>
		value.applications.some(
			(status) =>
				status.phase === 'Ready' &&
				status.process_id &&
				status.consumers.some((consumer) => consumer.application === group)
		)
	);
	const process = live.applications.find((status) =>
		status.consumers.some((consumer) => consumer.application === group)
	);
	await matrix({ kind: 'ClearPlayback' });
	await wait(runtime, (value) =>
		value.applications.every((status) => status.process_id === null && status.phase === 'Idle')
	);
	checks.push({
		group_process_ready_idle: true,
		pid: process.process_id,
		runtime: process.runtime
	});
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	const inspectorReads = sent.filter(
		(intent) => intent.payload?.kind === 'Applications' && intent.payload.edit?.kind === 'Inspect'
	);
	assert.equal(inspectorReads.length, 0, 'generic Inspector needs no app snapshot/controller');
	await page.screenshot({ path: path.join(directory, 'inspector.png') });
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				checks,
				errors,
				rejected,
				perInspectorQueries: inspectorReads.length,
				selectionRuntimeReads,
				scope:
					'Bundled UI authoring and actual headless Group process lifecycle; no desktop input or native cadence claim.'
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			result: 'pass',
			checks,
			errors,
			perInspectorQueries: inspectorReads.length,
			selectionRuntimeReads
		})
	);
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
