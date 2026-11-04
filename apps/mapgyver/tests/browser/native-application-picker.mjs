// Run from the workspace root with MAPGYVER_TEST_BINARY pointing at a checked native build.
// Computer Use selects the owned fixture in each actual Windows dialog; no dialog interception.
import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';

const native = await openNativeEditor();
const page = native.page;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const rejected = observeRejectedRequests(page, () => 'native App file picker');
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
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
const parameter = (label) =>
	page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
async function wait(read, predicate) {
	const deadline = Date.now() + 180_000;
	while (true) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `native picker verification timeout: ${JSON.stringify(value)}`);
		await delay(100);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Native picker acceptance'
	});
	const state = (await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } }))
		.snapshot;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Native picker acceptance', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const fixture = path.resolve('target/debug/application_process_fixture.exe');
	const owners = [];
	async function verifyPicker(owner, kind) {
		const read = async () =>
			(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: owner } }))
				.snapshot;
		await parameter('App file').waitFor();
		assert.equal((await read()).definition.executable, null);
		await parameter('App file').getByRole('button', { name: 'Browse', exact: true }).click();
		console.log(
			JSON.stringify({
				action: 'select-owned-fixture-in-native-dialog',
				kind,
				pid: native.app.pid,
				fixture
			})
		);
		await wait(read, (snapshot) => snapshot.definition.executable === fixture);
		await parameter('Window').waitFor();
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(read, (snapshot) => snapshot.definition.executable === null);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(read, (snapshot) => snapshot.definition.executable === fixture);
		const before = await read();
		await parameter('App file').getByRole('button', { name: 'Browse', exact: true }).click();
		console.log(JSON.stringify({ action: 'cancel-native-dialog', kind, pid: native.app.pid }));
		await parameter('App file')
			.getByRole('button', { name: 'Browse', exact: true })
			.waitFor({ timeout: 180_000 });
		assert.deepEqual(await read(), before, 'Cancel must not alter authored settings or revision');
		owners.push({ kind, owner, selected: fixture, undo_redo: true, cancel_unchanged: true });
	}
	await grid.getByTitle('Select Column 1', { exact: true }).click();
	await verifyPicker(state.columns[0].application, 'Column');
	await grid.getByTitle('Layer 1 / Column 1', { exact: true }).click({ position: { x: 5, y: 5 } });
	await grid
		.getByRole('gridcell', { selected: true })
		.click({ button: 'right', position: { x: 5, y: 5 } });
	await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
	const cell = (await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } }))
		.snapshot.cells[0];
	await verifyPicker(cell.definition.application, 'Cell');
	const runtime = (await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } })).snapshot;
	assert(
		runtime.applications.every((entry) => entry.process_id === null),
		'File selection never launches a process'
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'actual Windows Column/Cell App file picker, typed settings, conditional controls, exact Undo/Redo and Cancel',
			owners,
			page_errors: errors,
			rejected
		})
	);
} finally {
	await native.close();
	console.log('Native picker host exited normally (0)');
}
