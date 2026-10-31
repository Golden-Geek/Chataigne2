// M04 source intent parity through the ordinary host. No frontend, GPU or process fixtures.
// Build: cargo build --locked -p MapGyver --bin MapGyver
// Run from the repository root: node apps/mapgyver/tests/headless/media.mjs
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const directory = await mkdtemp(path.join(tmpdir(), 'mapgyver-media-parity-'));
const shutdown = path.join(directory, 'host.stop');
const listener = createServer();
listener.listen(0, '127.0.0.1');
await once(listener, 'listening');
const port = listener.address().port;
await new Promise((resolve) => listener.close(resolve));
const base = `http://127.0.0.1:${port}`;
const app = spawn(
	path.resolve('target/debug/MapGyver.exe'),
	['--headless', '--no-frontend', '--no-remote', '--automation-shutdown-file', shutdown],
	{
		windowsHide: true,
		env: { ...process.env, APPDATA: path.join(directory, 'data'), GC_UI_BIND: `127.0.0.1:${port}` },
		stdio: ['ignore', 'pipe', 'pipe']
	}
);
let log = '';
for (const stream of [app.stdout, app.stderr])
	stream.on('data', (data) => {
		log = (log + data).slice(-8192);
	});
const exited = new Promise((resolve, reject) => {
	app.once('error', reject);
	app.once('close', resolve);
});
const watchdog = setTimeout(() => app.kill(), 60000);
let token;
async function post(route, body) {
	const response = await fetch(`${base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(result)}`);
	return result;
}
const intent = (body) => post('/api/ui/intent', body);
const request = (payload) =>
	intent({ kind: 'invokeAppOperation', namespace: 'mapgyver.media', operation: 'edit', payload });
async function edit(payload) {
	const ack = await request(payload);
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
const channel = async (reference) => (await edit({ kind: 'InspectChannel', reference })).snapshot;
const fixed = { kind: 'Fixed', extent: { width: 7, height: 3 } };
let failure;
const started = Date.now();
try {
	const ready = Date.now() + 15000;
	while (!token) {
		assert(app.exitCode === null, `host exited before ready: ${log}`);
		try {
			token = (await post('/api/ui/session', {})).token;
		} catch (error) {
			if (Date.now() >= ready) throw error;
			await delay(25);
		}
	}
	assert.deepEqual(await inspect(), { assets: [], media: [] });
	const { asset } = await edit({
		kind: 'ImportAsset',
		locator: { kind: 'project_relative', path: 'first.png' },
		label: null
	});
	const sources = [];
	for (const kind of ['SolidColor', 'Picture', 'SharedTexture']) {
		const { reference: first } = await edit({ kind: 'CreateMedia', media_kind: kind, label: kind });
		const { reference: second } = await edit({
			kind: 'AddChannel',
			media: first.media,
			label: 'Second'
		});
		assert.equal(first.media, second.media);
		assert.notEqual(first.channel, second.channel);
		for (const [index, reference] of [first, second].entries()) {
			const before = await channel(reference);
			assert.equal(before.definition.size.kind, 'Automatic');
			let source;
			if (kind === 'SolidColor')
				source = {
					kind: 'SolidColor',
					rgba: index === 0 ? [0.25, 0.5, 0.75, 0.5] : [0.75, 0.5, 0.25, 0.25]
				};
			if (kind === 'Picture') source = { kind: 'Picture', asset };
			if (kind === 'SharedTexture')
				source = {
					kind: 'Spout',
					sender: `Exact Input ${index}`,
					maximum: {
						extent: { width: 16, height: 16 },
						format: 'Bgra8Unorm',
						color_encoding: 'Srgb',
						alpha_mode: 'Straight'
					}
				};
			const definition = {
				...before.definition,
				source,
				size: index === 0 ? { kind: 'Automatic' } : fixed,
				filter: 'Nearest'
			};
			await edit({ kind: 'ConfigureChannel', reference, definition });
			assert.deepEqual((await channel(reference)).definition, definition);
		}
		const peer = await channel(second);
		await edit({ kind: 'RenameMedia', media: first.media, label: `${kind} Renamed` });
		await edit({ kind: 'RenameChannel', reference: first, label: `${kind} A` });
		await edit({ kind: 'ReorderChannel', reference: first, after: second.channel });
		const ordered = (await inspect()).media.find((media) => media.id === first.media);
		assert.deepEqual(
			ordered.channels.map((channel) => channel.reference),
			[second, first]
		);
		await edit({ kind: 'RemoveChannel', reference: first });
		assert.deepEqual(await channel(second), peer);
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual(
			(await inspect()).media.find((media) => media.id === first.media),
			ordered
		);
		assert((await intent({ kind: 'redo' })).success);
		assert.deepEqual(await channel(second), peer);
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual(
			(await inspect()).media.find((media) => media.id === first.media),
			ordered
		);
		sources.push({ kind, first, second });
	}
	await edit({
		kind: 'RelinkAsset',
		asset,
		locator: { kind: 'project_relative', path: 'replacement.png' }
	});
	const picture = sources.find((source) => source.kind === 'Picture');
	for (const reference of [picture.first, picture.second])
		assert.equal((await channel(reference)).definition.source.asset, asset);
	const beforeReload = await inspect();
	await edit({ kind: 'ReloadAsset', asset });
	assert.deepEqual(await inspect(), beforeReload);
	assert((await intent({ kind: 'undo' })).success);
	assert.equal((await inspect()).assets[0].locator.path, 'first.png');
	assert((await intent({ kind: 'redo' })).success);
	assert.deepEqual(await inspect(), beforeReload);
	const solid = sources.find((source) => source.kind === 'SolidColor');
	const beforeRejection = await inspect();
	const rejected = await request({
		kind: 'ConfigureChannel',
		reference: solid.first,
		definition: (await channel(picture.first)).definition
	});
	assert.equal(rejected.success, false);
	assert.deepEqual(await inspect(), beforeRejection);
	const shared = sources.find((source) => source.kind === 'SharedTexture');
	const peer = await channel(shared.second);
	const current = await channel(shared.first);
	await edit({
		kind: 'ConfigureChannel',
		reference: shared.first,
		definition: { ...current.definition, source: { ...current.definition.source, sender: null } }
	});
	assert.equal((await channel(shared.first)).definition.source.sender, null);
	assert.deepEqual(await channel(shared.second), peer);
	const document = path.join(directory, 'sources.mapgyver');
	const saved = await inspect();
	assert.equal((await post('/api/ui/project-save', { path: document })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	assert.deepEqual(await inspect(), { assets: [], media: [] });
	const reloaded = await post('/api/ui/project-load', { path: document });
	assert.equal(path.basename(reloaded.path), 'sources.mapgyver');
	assert.equal(reloaded.recovery ?? null, null);
	assert.deepEqual(await inspect(), saved);
	for (const source of sources) {
		for (const reference of [source.first, source.second])
			assert.deepEqual((await channel(reference)).reference, reference);
	}
	await edit({ kind: 'RemoveMedia', media: picture.first.media });
	assert.equal(
		(await inspect()).media.some((media) => media.id === picture.first.media),
		false
	);
	assert.equal((await inspect()).assets[0].id, asset);
	assert((await intent({ kind: 'undo' })).success);
	assert.deepEqual(await inspect(), saved);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'M04 ordinary headless source intents, atomic rejection, shared assets, history and Save/New/Load',
			sources: sources.length,
			channels: sources.length * 2,
			stable_asset: asset,
			native_preview: 'Not instantiated; native recreation retains source-gate evidence.',
			elapsed_ms: Date.now() - started
		})
	);
} catch (error) {
	failure = error;
	console.error(log.slice(-1500));
} finally {
	await writeFile(shutdown, 'stop');
	const code = await exited;
	clearTimeout(watchdog);
	assert.equal(code, 0, 'ordinary host must return normally after its shutdown sentinel');
	console.log('M04 headless source host exited normally (0)');
}
if (failure) throw failure;
