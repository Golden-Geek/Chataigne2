import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

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
	const stop = page.locator('[data-matrix-stop-all]');
	const button = stop.getByRole('button', { name: 'Trigger Stop All', exact: true });
	await button.waitFor();
	const parameter = Number(await stop.getAttribute('data-parameter-id'));
	assert(Number.isSafeInteger(parameter) && parameter > 0);
	assert(await stop.getAttribute('data-parameter-uuid'));
	assert.equal(
		await page.getByRole('button', { name: 'Matrix runtime details', exact: true }).count(),
		0
	);
	assert.equal(await page.locator('.runtime-feedback').count(), 0);
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
