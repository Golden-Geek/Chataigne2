// Real app + real server; fault injection uses documented WebSocket routing only.
// https://playwright.dev/docs/api/class-websocketroute
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
const page = native.page;
page.setDefaultTimeout(10000);
let token,
	stage = 'setup',
	mode = 'normal',
	shutdown = false;
const connections = [],
	held = [],
	requests = [],
	receipts = [],
	errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
await page.routeWebSocket('**', (socket) => {
	const server = socket.connectToServer();
	const connection = { socket, server, closed: false };
	connections.push(connection);
	socket.onMessage((message) => {
		let frame;
		try {
			frame = JSON.parse(String(message));
		} catch {}
		const intent = frame?.intent;
		const geometry =
			frame?.kind === 'intent' &&
			intent?.kind === 'invokeAppOperation' &&
			intent.namespace === 'mapgyver.media' &&
			intent.payload?.kind === 'Matrix' &&
			intent.payload.edit.kind === 'EditLayerGeometry';
		if (frame?.kind === 'intent' || frame?.kind === 'intentBatch')
			requests.push({
				id: frame.request_id,
				stage,
				geometry,
				kind: intent?.kind,
				client_edit_id: intent?.client_edit_id,
				operation: intent?.payload?.edit?.kind ?? intent?.payload?.kind,
				mode,
				sent: Date.now()
			});
		if (geometry && mode === 'hold') {
			held.push({ message, connection, id: frame.request_id });
			return;
		}
		if (geometry && mode === 'reject') {
			frame.intent.payload.media = '00000000-0000-0000-0000-000000000000';
			server.send(JSON.stringify(frame));
		} else server.send(message);
	});
	server.onMessage((message) => {
		let frame;
		try {
			frame = JSON.parse(String(message));
		} catch {}
		if (frame?.kind === 'control' && ['applied', 'rejected'].includes(frame.update.phase))
			receipts.push({
				id: frame.update.request_id,
				phase: frame.update.phase,
				at: Date.now(),
				errors: (frame.update.acknowledgements ?? [frame.update.acknowledgement])
					.filter((ack) => ack && !ack.success)
					.map((ack) => ({ code: ack.error_code, message: ack.error_message }))
			});
		if (!connection.closed && !shutdown) socket.send(message);
	});
});
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
async function wait(read, ready) {
	const deadline = Date.now() + 12000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
const feedback = page.locator('.user-action-activity');
const hidden = () => feedback.getAttribute('aria-hidden');
async function trace() {
	return page.evaluate(() => window.__pendingFeedbackTrace);
}
async function resetTrace() {
	await wait(hidden, (value) => value === 'true');
	await page.evaluate(() => {
		window.__pendingFeedbackTrace = [];
	});
}
try {
	// Only newly created sockets are routed; reload this owned WebView before setup.
	await page.reload();
	token = (await post('/api/ui/session', {})).token;
	await wait(
		() => connections.length,
		(value) => value > 0
	);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Pending feedback Matrix'
	});
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Feedback red'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 32, height: 24 } }
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const layer = initial.items[0].id.id;
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [80, 70], size: { width: 120, height: 90 }, duration_ms: 250 }
	});
	await matrix({
		kind: 'SetCells',
		assignments: initial.items.flatMap((row) =>
			initial.columns.map((column) => ({ layer: row.id.id, column: column.id, source }))
		)
	});
	await matrix({ kind: 'LaunchColumn', column: initial.columns[0].id, mode: 'Cut' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Pending feedback Matrix', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await page
		.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
		.getByTitle('Select Layer 1', { exact: true })
		.click();
	await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
	const panel = page.locator(
		'.workbench-editor[data-panel-id="composition"][data-spatial-kind="Matrix"]'
	);
	const ready = () =>
		wait(
			() => panel.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	async function start() {
		await ready();
		const rect = await panel.locator(`[data-layer-id="${layer}"]`).boundingBox();
		assert(rect);
		const point = { x: rect.x + rect.width * 0.43, y: rect.y + rect.height * 0.43 };
		await page.mouse.move(point.x, point.y);
		await page.mouse.down();
		return point;
	}
	async function finish() {
		const start = requests.length;
		await page.mouse.up();
		const end = await wait(
			() => requests.slice(start).find((request) => request.kind === 'endEdit'),
			Boolean
		);
		await wait(() => receipts.some((receipt) => receipt.id === end.id), Boolean);
		await ready();
	}
	await ready();
	await page.evaluate(() => {
		const element = document.querySelector('.user-action-activity');
		window.__pendingFeedbackTrace = [];
		const record = () =>
			window.__pendingFeedbackTrace.push({
				at: performance.now(),
				hidden: element.getAttribute('aria-hidden'),
				phase: element.getAttribute('data-phase'),
				text: element.textContent
			});
		new MutationObserver(record).observe(element, {
			attributes: true,
			childList: true,
			subtree: true,
			characterData: true
		});
	});
	stage = 'warmup';
	let point = await start();
	await page.mouse.move(point.x + 2, point.y);
	await finish();
	await resetTrace();
	stage = 'long fast-acknowledged gesture';
	point = await start();
	for (let i = 0; i < 40; i++) {
		await page.mouse.move(point.x + (i % 2 ? 8 : 0), point.y + 3);
		await delay(20);
	}
	await finish();
	await delay(200);
	const fast = await trace();
	assert(
		fast.every((sample) => sample.hidden === 'true'),
		JSON.stringify(fast)
	);
	const fastRequests = requests.filter((request) => request.stage === stage && request.geometry);
	assert(fastRequests.length > 10, 'long gesture must send actual geometry operations');
	const ages = fastRequests.map((request) => {
		const applied = receipts.find(
			(receipt) => receipt.id === request.id && receipt.phase === 'applied'
		);
		assert(applied, 'fast request must have its real server receipt');
		return applied.at - request.sent;
	});
	assert(Math.max(...ages) < 200, 'fast fixture actually receives fast acknowledgements');
	checks.push({
		stage,
		requests: fastRequests.length,
		max_transport_ms: Math.max(...ages),
		visible: false
	});

	stage = 'genuinely outstanding geometry';
	await resetTrace();
	mode = 'hold';
	point = await start();
	await page.mouse.move(point.x + 10, point.y);
	await wait(
		() => held.length,
		(value) => value === 1
	);
	await wait(hidden, (value) => value === 'false');
	assert.match(await feedback.textContent(), /Applying changes/);
	await delay(400);
	assert.equal(await hidden(), 'false');
	mode = 'normal';
	for (const pending of held.splice(0)) pending.connection.server.send(pending.message);
	await finish();
	await wait(hidden, (value) => value === 'true');
	const slow = await trace();
	assert(slow.some((sample) => sample.hidden === 'false'));
	assert(slow.some((sample) => sample.text.includes('Changes applied')));
	checks.push({ stage, trace: slow });

	stage = 'rejected operation';
	await resetTrace();
	const beforeRejected = await inspect();
	mode = 'reject';
	point = await start();
	await page.mouse.move(point.x + 8, point.y);
	await finish();
	await wait(
		() => feedback.getAttribute('data-phase'),
		(value) => value === 'rejected'
	);
	assert.equal(await hidden(), 'false');
	mode = 'normal';
	assert.deepEqual(await inspect(), beforeRejected);
	checks.push({ stage, trace: await trace() });
	await wait(hidden, (value) => value === 'true');

	stage = 'abandoned operation and reconnect';
	await resetTrace();
	mode = 'hold';
	point = await start();
	await page.mouse.move(point.x + 5, point.y);
	await wait(
		() => held.length,
		(value) => value === 1
	);
	await wait(hidden, (value) => value === 'false');
	await page.mouse.up();
	const pending = held.splice(0)[0];
	const previousConnections = connections.length;
	mode = 'normal';
	pending.connection.closed = true;
	await pending.connection.socket.close({ code: 1012, reason: 'Owned feedback recovery check' });
	await wait(
		() => connections.length,
		(value) => value > previousConnections
	);
	await ready();
	await wait(hidden, (value) => value === 'true');
	await delay(500);
	assert.equal(await hidden(), 'true', 'abandoned deadline cannot resurrect old progress');
	checks.push({ stage, connections: connections.length, trace: await trace() });
	const staleEnds = receipts.filter(
		(receipt) =>
			receipt.phase === 'rejected' &&
			requests.some(
				(request) =>
					request.id === receipt.id &&
					request.kind === 'endEdit' &&
					request.stage === 'abandoned operation and reconnect' &&
					typeof request.client_edit_id === 'string' &&
					receipt.errors.some(
						(error) =>
							error.message?.includes(request.client_edit_id) &&
							error.message.includes('no session is active')
					)
			)
	);
	const unexpected = receipts.filter(
		(receipt) =>
			receipt.phase === 'rejected' &&
			!staleEnds.includes(receipt) &&
			!requests.some((request) => request.id === receipt.id && request.mode === 'reject')
	);
	assert.deepEqual(unexpected, []);
	assert.deepEqual(errors, []);
	await page.screenshot({ path: path.join(directory, 'settled.png') });
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				checks,
				staleEnds,
				requests,
				receipts,
				errors,
				limit: 'Controlled transport fault injection; no input cadence or physical latency claim'
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ passed: true, pid: native.app.pid, checks: checks.length }));
} catch (error) {
	await page.mouse.up().catch(() => {});
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				stage,
				error: String(error),
				stack: error.stack,
				checks,
				requests,
				receipts,
				trace: await trace().catch(() => []),
				errors,
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	shutdown = true;
	held.length = 0;
	await native.close();
}
