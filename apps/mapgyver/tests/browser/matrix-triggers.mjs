import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { verifyMatrixScopeControls } from './matrix-scope-controls.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the successfully compiled current app');
const native = await openNativeEditor();
const { page } = native;
const errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
const sent = [];
const client = await page.context().newCDPSession(page);
client.on('Network.webSocketFrameSent', ({ response }) => {
	try {
		const frame = JSON.parse(response.payloadData);
		if (frame.kind === 'intent') sent.push(frame.intent);
		if (frame.kind === 'intentBatch') sent.push(...frame.intents);
	} catch {}
});
await client.send('Network.enable');
let token;
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
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
async function wait(read, ready) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, JSON.stringify(value));
		await delay(50);
	}
}

async function verifyMatrixNavigation() {
	const label = 'Matrix navigation controls';
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label });
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const original = (await matrix({ kind: 'Inspect' })).snapshot.columns.at(-1).id;
	const { group: firstGroup } = await matrix({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Transport Group A'
	});
	const { group: nestedGroup } = await matrix({
		kind: 'AddColumnGroup',
		parent: firstGroup,
		label: 'Nested Group'
	});
	const {
		columns: [first, second]
	} = await matrix({ kind: 'AddColumns', parent: nestedGroup, count: 2 });
	const { group: secondGroup } = await matrix({
		kind: 'AddColumnGroup',
		parent: null,
		label: 'Transport Group B'
	});
	const {
		columns: [third]
	} = await matrix({ kind: 'AddColumns', parent: secondGroup, count: 1 });

	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix: Matrix navigation controls$/ })
		.waitFor();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const toolbar = grid.getByRole('toolbar', { name: 'Matrix actions', exact: true });
	await toolbar.waitFor();
	const gridBox = await grid.boundingBox();
	const toolbarBox = await toolbar.boundingBox();
	assert(gridBox && toolbarBox);
	assert(Math.abs(gridBox.x - toolbarBox.x) < 2 && Math.abs(gridBox.y - toolbarBox.y) < 2);
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	const controls = new Map([
		['Previous Column', 'previous_column'],
		['Next Column', 'next_column'],
		['Previous Column Group', 'previous_column_group'],
		['Next Column Group', 'next_column_group'],
		['Trigger Stop All', 'stop_all']
	]);
	const graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const byId = new Map(graph.nodes.map((node) => [node.node_id, node]));
	const owner = graph.nodes.find((node) => node.uuid === reference.media);
	const parameters = owner.children
		.map((id) => byId.get(id))
		.find((node) => node?.decl_id === 'parameters');
	assert.equal(await toolbar.getByRole('button').count(), controls.size);
	const triggerIds = new Map();
	for (const [name, field] of controls) {
		const button = toolbar.getByRole('button', { name, exact: true });
		const trigger = parameters.children
			.map((id) => byId.get(id))
			.find((node) => node?.decl_id === `parameters/${field}`);
		assert(trigger, `${name} has an authored Parameter on the selected Matrix`);
		assert.equal(await button.textContent(), '');
		assert(await button.getAttribute('title'));
		assert.equal(await button.getAttribute('data-owner-uuid'), reference.media);
		assert.equal(await button.getAttribute('data-matrix-control'), field);
		assert.equal(Number(await button.getAttribute('data-parameter-id')), trigger.node_id);
		assert.equal(await button.getAttribute('data-parameter-uuid'), trigger.uuid);
		triggerIds.set(name, trigger.node_id);
	}
	const pulse = async (name, key) => {
		const button = toolbar.getByRole('button', { name, exact: true });
		await wait(() => button.isEnabled(), Boolean);
		const before = sent.length;
		if (key) await button.press(key);
		else await button.click();
		await wait(
			() => sent.slice(before),
			(intents) =>
				intents.some(
					(intent) =>
						intent.kind === 'setParam' &&
						intent.node === triggerIds.get(name) &&
						Array.isArray(intent.value?.Trigger)
				)
		);
	};
	const step = async (name, column, key) => {
		await pulse(name, key);
		await wait(playback, (state) => state.desired_column === column);
	};
	await matrix({ kind: 'LaunchColumn', column: original, mode: 'Cut' });
	await step('Next Column', first);
	await step('Next Column', second, 'Enter');
	await step('Previous Column', first, 'Space');
	await step('Next Column Group', third);
	await step('Previous Column Group', first, 'Enter');
	await pulse('Trigger Stop All', 'Space');
	await wait(playback, (state) => state.desired_column === null && state.layers.length === 0);
	return 'top-left Matrix icon toolbar click/keyboard navigation crosses ordinary and nested columns, steps only top-level groups, and stops through its real Parameter';
}

let failure;
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Typed Matrix controls'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const original = (await matrix({ kind: 'Inspect' })).snapshot;
	const row = original.items[0].id.id;
	const column = original.columns[1].id;
	await matrix({
		kind: 'ConfigureCell',
		layer: row,
		column,
		settings: {
			position: [0, 0],
			size: null,
			fit_mode: 'Fit',
			opacity: 1,
			enabled: true
		}
	});
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
	await tab('Medias').click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Typed Matrix controls', exact: true })
		.click();
	await tab('Matrix').click();
	const button = page.getByRole('button', { name: 'Trigger Stop All', exact: true });
	await button.waitFor();
	const parameter = Number(await button.getAttribute('data-parameter-id'));
	assert(Number.isSafeInteger(parameter) && parameter > 0);
	assert(await button.getAttribute('data-parameter-uuid'));
	assert.equal(
		await page.getByRole('button', { name: 'Matrix runtime details', exact: true }).count(),
		0
	);
	assert.equal(await page.locator('.runtime-feedback').count(), 0);
	const sparseColumn = original.columns[2].id;
	const sparse = page
		.getByRole('gridcell')
		.filter({
			has: page.locator('.source-label')
		})
		.and(page.locator(`[title="${original.items[0].label} / ${original.columns[2].label}"]`));
	assert.equal(original.cells.length, 0);
	await sparse.dblclick({ position: { x: 24, y: 24 } });
	await wait(playback, (value) => value.layers.some((entry) => entry.column === sparseColumn));
	const afterFirst = (await matrix({ kind: 'Inspect' })).snapshot;
	const created = afterFirst.cells.find(
		(cell) => cell.layer === row && cell.definition.column === sparseColumn
	);
	assert(created && created.media === null, 'double-click materializes a normal empty Cell');
	assert.equal(afterFirst.cells.length, 2);
	const pulse = await matrix({ kind: 'LaunchCell', layer: row, column: sparseColumn, mode: 'Cut' });
	assert.equal(pulse.kind, 'MatrixControlInvoked');
	assert.equal(pulse.owner, created.id);
	assert.equal(typeof pulse.parameter, 'string');
	for (const name of ['Trigger Cell', 'Trigger Cell (Cut)']) {
		const bounds = await sparse.boundingBox();
		assert(bounds);
		await sparse.click({ button: 'right', position: { x: 8, y: bounds.height - 8 } });
		await page.getByRole('menuitem', { name, exact: true }).click();
		await wait(playback, (value) => value.layers.some((entry) => entry.column === sparseColumn));
	}
	assert.equal((await matrix({ kind: 'Inspect' })).snapshot.cells.length, 2);
	await button.click();
	await wait(playback, (value) => value.layers.length === 0);
	checks.push(
		'sparse Cell double-click, both context actions and transport share real control ownership without duplicate Cells'
	);
	const file = path.join(directory, 'authored.mapgyver');
	await post('/api/ui/project-save', { path: file });
	const history = (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).history;
	await matrix({ kind: 'LaunchColumn', column: original.columns[0].id, mode: 'Cut' });
	await matrix({ kind: 'LaunchCell', layer: row, column, mode: 'Cut' });
	assert.equal((await playback()).layers.length, 1);
	await button.click();
	await wait(
		() => sent,
		(intents) =>
			intents.some(
				(intent) =>
					intent.kind === 'setParam' &&
					intent.node === parameter &&
					Array.isArray(intent.value?.Trigger)
			)
	);
	await wait(playback, (value) => value.desired_column === null && value.layers.length === 0);
	checks.push(
		'bundled Stop All widget invokes its real Parameter and clears Column plus independent Cell'
	);
	for (let cycle = 0; cycle < 30; cycle++) {
		await matrix({ kind: 'LaunchCell', layer: row, column, mode: 'Cut' });
		const ack = await post('/api/ui/intent', {
			kind: 'setParam',
			node: parameter,
			value: { Trigger: [] },
			behaviour: 'Append'
		});
		assert(ack.success, JSON.stringify(ack));
		assert.equal((await playback()).layers.length, 0);
	}
	assert.deepEqual((await post('/api/ui/snapshot', { scope: 'wholeGraph' })).history, history);
	checks.push('30 rapid direct-parameter transport stop cycles preserve authored history');
	await post('/api/ui/project-load', { path: file });
	await page.reload();
	assert.equal((await playback()).desired_column, null);
	assert.equal((await playback()).layers.length, 0);
	await tab('Medias').click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Typed Matrix controls', exact: true })
		.click();
	await tab('Matrix').click();
	await button.waitFor();
	await matrix({ kind: 'LaunchCell', layer: row, column, mode: 'Cut' });
	await button.click();
	await wait(playback, (value) => value.layers.length === 0);
	checks.push('project reload neither replays controls nor loses the real Stop All widget');
	checks.push(await verifyMatrixScopeControls({ page, matrix, wait, sent }));
	checks.push(await verifyMatrixNavigation());
	assert.deepEqual(errors, []);
	await page.screenshot({ path: path.join(directory, 'stop-all.png') });
} catch (error) {
	failure = error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: failure ? 'failed' : 'pass',
				pid: native.app.pid,
				checks,
				errors,
				failure: failure ? String(failure.stack ?? failure) : null,
				limit:
					'Functional bundled WebView and HTTP Parameter control check; no GPU timing or native final-use retirement qualification.'
			},
			null,
			2
		)
	);
}
if (failure) throw failure;
console.log(JSON.stringify({ result: 'pass', checks }));
