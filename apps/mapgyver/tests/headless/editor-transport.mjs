// Sequential public WebSocket edits/reads without a desktop, recorder or injected input.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const listener = createServer();
await new Promise((resolve) => listener.listen(0, '127.0.0.1', resolve));
const port = listener.address().port;
await new Promise((resolve) => listener.close(resolve));
const base = `http://127.0.0.1:${port}`;
const stop = path.join(directory, 'host.stop');
const app = spawn(
	path.resolve(process.env.MAPGYVER_BIN ?? 'target/debug/MapGyver.exe'),
	['--headless', '--no-frontend', '--no-remote', '--automation-shutdown-file', stop],
	{
		windowsHide: true,
		env: {
			...process.env,
			APPDATA: path.join(directory, 'profile'),
			GC_UI_BIND: `127.0.0.1:${port}`
		},
		stdio: ['ignore', 'pipe', 'pipe']
	}
);
let log = '',
	socket,
	sequence = 0;
const pending = new Map();
const exit = new Promise((resolve, reject) => {
	app.once('error', reject);
	app.once('close', resolve);
});
for (const stream of [app.stdout, app.stderr])
	stream.on('data', (data) => {
		log = (log + data).slice(-65536);
	});
async function request(intent) {
	const request_id = `editor-${++sequence}`;
	const started = performance.now();
	const result = new Promise((resolve, reject) => {
		const timer = setTimeout(() => {
			pending.delete(request_id);
			reject(new Error(`Timeout ${request_id}`));
		}, 5000);
		pending.set(request_id, {
			resolve: (ack) => {
				clearTimeout(timer);
				resolve(ack);
			},
			phases: []
		});
	});
	socket.send(JSON.stringify({ kind: 'intent', request_id, intent, include_self_events: true }));
	const ack = await result;
	assert(ack.success, JSON.stringify(ack));
	return { ack, ms: performance.now() - started };
}
const intent = async (value) => (await request(value)).ack;
const edit = async (payload) =>
	(
		await intent({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload
		})
	).result.value;
try {
	const deadline = Date.now() + 15000;
	while (true) {
		assert.equal(app.exitCode, null, log);
		try {
			if ((await fetch(`${base}/api/ui/health`, { signal: AbortSignal.timeout(500) })).ok) break;
		} catch {}
		assert(Date.now() < deadline, log);
		await delay(25);
	}
	const session = await (
		await fetch(`${base}/api/ui/session`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: '{}'
		})
	).json();
	socket = new WebSocket(`ws://127.0.0.1:${port}/api/ui/ws`, [
		'golden-ui',
		`gc-session.${session.token}`
	]);
	socket.addEventListener('message', (event) => {
		const frame = JSON.parse(event.data);
		if (frame.kind !== 'control') return;
		const update = frame.update,
			waiting = pending.get(update.request_id);
		if (!waiting) return;
		waiting.phases.push(update.phase);
		if (update.acknowledgement) {
			pending.delete(update.request_id);
			waiting.resolve(update.acknowledgement);
		}
	});
	await new Promise((resolve, reject) => {
		socket.addEventListener('open', resolve, { once: true });
		socket.addEventListener('error', reject, { once: true });
	});
	const source = (
		await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Transport source' })
	).reference;
	const composition = (
		await edit({ kind: 'CreateMedia', media_kind: 'Composition', label: 'Transport composition' })
	).reference;
	const operation = (value) => ({ kind: 'Composition', reference: composition, edit: value });
	const instance = (await edit(operation({ kind: 'AddInstances', sources: [source] }))).instances[0]
		.instance;
	const initial = (await edit(operation({ kind: 'Inspect' }))).snapshot.instances[0];
	await intent({ kind: 'beginEdit', client_edit_id: 'transport', label: 'Continuous geometry' });
	const samples = [];
	for (let step = 1; step <= 90; step++) {
		const payload = operation({
			kind: 'EditInstanceGeometry',
			instance,
			edit: {
				mode: 'Translate',
				origin: initial.definition.transform,
				start: { x: 0, y: 0 },
				point: { x: step, y: step }
			}
		});
		const { ack, ms } = await request({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload
		});
		assert.deepEqual(
			ack.result.value.snapshot.definition.transform.position,
			initial.definition.transform.position.map((v) => v + step)
		);
		const read = await request({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: operation({ kind: 'Inspect' })
		});
		assert.deepEqual(read.ack.result.value.snapshot.instances[0], ack.result.value.snapshot);
		samples.push({ edit_ms: ms, read_ms: read.ms });
	}
	await intent({ kind: 'endEdit', client_edit_id: 'transport' });
	const summary = Object.fromEntries(
		['edit_ms', 'read_ms'].map((key) => {
			const values = samples.map((sample) => sample[key]).sort((a, b) => a - b);
			return [key, { median: values[45], p95: values[85], max: values[89] }];
		})
	);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ pid: app.pid, samples, summary }, null, 2)
	);
	console.log(JSON.stringify(summary));
} finally {
	socket?.close();
	await writeFile(stop, 'stop');
	const code = await Promise.race([
		exit,
		delay(10000).then(() => {
			throw new Error(`Headless shutdown timed out: ${log}`);
		})
	]);
	await writeFile(path.join(directory, 'host.log'), log);
	assert.equal(code, 0, log);
}
