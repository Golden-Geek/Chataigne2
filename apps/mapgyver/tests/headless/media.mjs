// M04 source intent parity through the ordinary host. No GPU or process fixtures.
// Build: cargo build --locked -p MapGyver --bin MapGyver
// Run from the repository root: node apps/mapgyver/tests/headless/media.mjs
// Add --ui-reload to verify the bundled Reload Picture control in the same isolated host.
// Add --resolution-ui to verify shared Resolution through the generic Golden Inspector.
// Add --channels-ui to verify the media-only manager and typed Channels collection.
// Add --source-ui for the complete M04 source authoring matrix in the Golden tree/Inspector.
// Add --composition-review for M06 parity and a saved ordinary-app review scene.
// Add --matrix-review for M07 parity and an initially unlaunched Matrix review scene.
// Add --application-ui for M08 real bundled browser form/history verification.
// Add --screen-ui for M05 real bundled Screen source/gesture/history verification.
// Add --surface-source-ui for M05 real Source crop/form/calibration/history verification.
// Add --composition-ui for M06 real Composition form/gesture/history verification.
// Add --matrix-ui for M07 real Matrix grid/authoring/playback-feedback verification.
// Add --matrix-project-ui for M07 actual File New/upload and editor replacement verification.
// Add --composition-project-ui for M06 actual New-during-drag and review-upload verification.
// Add --column-applications-ui for M08 actual column UI and owned-process lifecycle verification.
// Add --source-drop-ui for real Media/Channel outliner drags into Screen/Composition editors.
// Add --catalog-ui for object-only manager creation menus.
// Add --free-warp-ui for held invalid-shape editing and upside-down recovery.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

assert(
	!(process.argv.includes('--screen-ui') && process.argv.includes('--surface-source-ui')),
	'Run the Screen and Surface Source UI checks separately'
);
const withUi = [
	'--source-ui',
	'--ui-reload',
	'--application-ui',
	'--screen-ui',
	'--surface-source-ui',
	'--composition-ui',
	'--parameter-ui',
	'--resolution-ui',
	'--channels-ui',
	'--matrix-ui',
	'--matrix-project-ui',
	'--composition-project-ui',
	'--column-applications-ui',
	'--source-drop-ui',
	'--workbench-ui',
	'--catalog-ui',
	'--free-warp-ui'
].some((flag) => process.argv.includes(flag));
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
	[
		'--headless',
		...(withUi ? [] : ['--no-frontend']),
		'--no-remote',
		'--automation-shutdown-file',
		shutdown
	],
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
				size: { kind: 'Automatic' },
				filter: 'Nearest'
			};
			await edit({ kind: 'ConfigureChannel', reference, definition });
			assert.deepEqual((await channel(reference)).definition, definition);
		}
		await edit({ kind: 'ConfigureMediaResolution', media: first.media, size: fixed });
		for (const reference of [first, second])
			assert.deepEqual((await channel(reference)).definition.size, fixed);
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
	const unsupported = path.join(directory, 'unsupported-media-schema.mapgyver');
	await writeFile(
		unsupported,
		(await readFile(document, 'utf8')).replaceAll('mapgyver.media.v2', 'mapgyver.media')
	);
	await assert.rejects(
		() => post('/api/ui/project-load', { path: unsupported }),
		/Unsupported MapGyver media schema/
	);
	assert.deepEqual(await inspect(), saved, 'Rejected schema load must preserve the live project');
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
	if (process.argv.includes('--ui-reload')) {
		const { verifyPictureReloadUi } = await import('../browser/picture-reload.mjs');
		await verifyPictureReloadUi({ base, inspect, picture, asset });
	}
	if (process.argv.includes('--source-ui')) {
		const { verifySourcesUi } = await import('../browser/sources.mjs');
		await verifySourcesUi({ base, inspect });
	}
	if (process.argv.includes('--application-ui')) {
		const { verifyApplicationUi } = await import('../browser/applications.mjs');
		await verifyApplicationUi({ base, edit });
	}
	if (process.argv.includes('--screen-ui')) {
		const { verifyScreenUi } = await import('../browser/screens.mjs');
		await verifyScreenUi({ base, post, media: await inspect(), inspectMedia: inspect });
	}
	if (process.argv.includes('--free-warp-ui')) {
		const { verifyFreeWarpUi } = await import('../browser/free-warp.mjs');
		await verifyFreeWarpUi({ base, post });
	}
	if (process.argv.includes('--surface-source-ui')) {
		const { verifySurfaceSourceUi } = await import('../browser/surface-source.mjs');
		await verifySurfaceSourceUi({ base, post, media: await inspect(), inspectMedia: inspect });
	}
	if (process.argv.includes('--composition-review')) {
		const { verifyComposition } = await import('./composition.mjs');
		await verifyComposition({ post, intent, edit });
	}
	if (process.argv.includes('--composition-ui')) {
		const { verifyCompositionUi } = await import('../browser/composition.mjs');
		await verifyCompositionUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--parameter-ui')) {
		const { verifyParameterUi } = await import('../browser/parameters.mjs');
		await verifyParameterUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--resolution-ui')) {
		const { verifyMediaResolutionUi } = await import('../browser/media-resolution.mjs');
		await verifyMediaResolutionUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--channels-ui')) {
		const { verifyChannelsUi } = await import('../browser/channels.mjs');
		await verifyChannelsUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--matrix-review')) {
		const { verifyMatrix } = await import('./matrix.mjs');
		await verifyMatrix({ post, intent, edit });
	}
	if (process.argv.includes('--matrix-ui')) {
		const { verifyMatrixUi } = await import('../browser/matrix.mjs');
		await verifyMatrixUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--matrix-project-ui')) {
		const { verifyMatrixProjectUi } = await import('../browser/matrix-project.mjs');
		await verifyMatrixProjectUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--composition-project-ui')) {
		const { verifyCompositionProjectUi } = await import('../browser/composition-project.mjs');
		await verifyCompositionProjectUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--column-applications-ui')) {
		const { verifyColumnApplicationsUi } = await import('../browser/column-applications.mjs');
		await verifyColumnApplicationsUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--workbench-ui')) {
		const { verifyWorkbenchUi } = await import('../browser/workbench.mjs');
		await verifyWorkbenchUi({ base, post, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--catalog-ui')) {
		const { verifyCreationCatalogsUi } = await import('../browser/catalogs.mjs');
		await verifyCreationCatalogsUi({ base });
	}
	if (process.argv.includes('--source-drop-ui')) {
		const { verifySourceDropUi } = await import('../browser/source-drop.mjs');
		await verifySourceDropUi({ base, post, edit, inspectMedia: inspect });
	}
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
