// M04 source intent parity through the ordinary host. No GPU or process fixtures.
// Build: cargo build --locked -p MapGyver --bin MapGyver
// MAPGYVER_TEST_BINARY can select an isolated or packaged executable instead of target/debug.
// Run from the repository root: node apps/mapgyver/tests/headless/media.mjs
// Add --ui-reload to verify the bundled Reload Picture control in the same isolated host.
// Add --resolution-ui to verify shared Resolution through the generic Golden Inspector.
// Add --channels-ui to verify the media-only manager and typed Channels collection.
// Add --media-script-ui for app-template script creation and named source API callbacks.
// Add --media-command-ui for typed command creation, generic editing and firing/history.
// Add --reference-ui for Reference creation, generic target/resolution and live mirrored identities.
// Add --media-preview-ui for independent follow/lock/grid/persisted Media Preview panels.
// Add --preview-continuity-ui for simultaneous grids, real reconnect and New/Load lock recovery.
// Add --navigation-ui for shared smooth zoom, camera cancellation and project generation replacement.
// Add --screen-selection-ui for shared Inspector, hover/selected handles and independent corner selection.
// Add --screen-keyboard-ui for output-pixel repeat, grouped history and focus/project fencing.
// Add --screen-translation-ui for backend-owned Alt-drag from Surface bodies/corners.
// Add --screen-fit-ui for backend Fill/Fit icon actions and grouped history.
// Add --runtime-footer-ui for app footer contribution and explicit unavailable native metrics.
// Add --screen-details-ui for responsive bounded Screen/Surface outliner supplements.
// Add --owned-media-ui for Composition Reference/Alt-copy drops and real child-media ownership.
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
	'--cell-application-ui',
	'--screen-ui',
	'--surface-source-ui',
	'--composition-ui',
	'--parameter-ui',
	'--resolution-ui',
	'--channels-ui',
	'--media-script-ui',
	'--media-command-ui',
	'--reference-ui',
	'--media-preview-ui',
	'--preview-continuity-ui',
	'--navigation-ui',
	'--screen-selection-ui',
	'--screen-keyboard-ui',
	'--screen-translation-ui',
	'--screen-fit-ui',
	'--runtime-footer-ui',
	'--screen-details-ui',
	'--owned-media-ui',
	'--matrix-owned-media-ui',
	'--unavailable-reference-ui',
	'--matrix-ui',
	'--matrix-parameters-ui',
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
	path.resolve(process.env.MAPGYVER_TEST_BINARY ?? 'target/debug/MapGyver.exe'),
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
					sender: `Exact Input ${index}`
				};
			const definition = {
				...before.definition,
				source,
				size: { kind: 'Automatic' },
				filter: 'Nearest'
			};
			await edit({ kind: 'ConfigureMediaSampling', media: reference.media, filter: 'Nearest' });
			await edit({ kind: 'ConfigureChannel', reference, source });
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
		source: (await channel(picture.first)).definition.source
	});
	assert.equal(rejected.success, false);
	assert.deepEqual(await inspect(), beforeRejection);
	const shared = sources.find((source) => source.kind === 'SharedTexture');
	const peer = await channel(shared.second);
	const current = await channel(shared.first);
	await edit({
		kind: 'ConfigureChannel',
		reference: shared.first,
		source: { ...current.definition.source, sender: null }
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
		(await readFile(document, 'utf8')).replaceAll('mapgyver.media.v5', 'mapgyver.media')
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
	if (process.argv.includes('--cell-application-ui')) {
		const { verifyCellApplicationsUi } = await import('../browser/cell-applications.mjs');
		await verifyCellApplicationsUi({ base, edit, inspectMedia: inspect });
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
		await verifyCompositionUi({ base, edit, post, directory, inspectMedia: inspect });
	}
	if (process.argv.includes('--parameter-ui')) {
		const { verifyParameterUi } = await import('../browser/parameters.mjs');
		await verifyParameterUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--resolution-ui')) {
		const { verifyMediaResolutionUi } = await import('../browser/media-resolution.mjs');
		await verifyMediaResolutionUi({ base, edit, inspectMedia: inspect });
		const { verifyProceduralMediaUi } = await import('../browser/procedural-media.mjs');
		await verifyProceduralMediaUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--channels-ui')) {
		const { verifyChannelsUi } = await import('../browser/channels.mjs');
		await verifyChannelsUi({ base, edit, inspectMedia: inspect });
		const { verifySharedTextureUi } = await import('../browser/shared-texture.mjs');
		await verifySharedTextureUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--media-script-ui')) {
		const { verifyMediaScriptUi } = await import('../browser/media-script.mjs');
		await verifyMediaScriptUi({ base, inspectMedia: inspect });
	}
	if (process.argv.includes('--media-command-ui')) {
		const { verifyMediaCommandUi } = await import('../browser/media-command.mjs');
		await verifyMediaCommandUi({ base, edit, intent, post, inspectMedia: inspect });
	}
	if (process.argv.includes('--reference-ui')) {
		const { verifyReferenceUi } = await import('../browser/reference.mjs');
		await verifyReferenceUi({ base, edit, inspectMedia: inspect });
	}
	if (process.argv.includes('--media-preview-ui')) {
		const { verifyMediaPreviewUi } = await import('../browser/media-preview.mjs');
		await verifyMediaPreviewUi({ base, edit });
	}
	if (process.argv.includes('--preview-continuity-ui')) {
		const { verifyPreviewContinuity } = await import('../browser/preview-continuity.mjs');
		await verifyPreviewContinuity({ base, edit, post, directory });
	}
	if (process.argv.includes('--navigation-ui')) {
		const { verifyViewportNavigation } = await import('../browser/viewport-navigation.mjs');
		await verifyViewportNavigation({ base, edit, post, directory });
	}
	if (process.argv.includes('--screen-selection-ui')) {
		const { verifyScreenSelection } = await import('../browser/screen-selection.mjs');
		await verifyScreenSelection({ base, post });
	}
	if (process.argv.includes('--screen-keyboard-ui')) {
		const { verifyScreenKeyboard } = await import('../browser/screen-keyboard.mjs');
		await verifyScreenKeyboard({ base, post });
	}
	if (process.argv.includes('--screen-translation-ui')) {
		const { verifyScreenTranslation } = await import('../browser/screen-translation.mjs');
		await verifyScreenTranslation({ base, post });
	}
	if (process.argv.includes('--screen-fit-ui')) {
		const { verifyScreenFit } = await import('../browser/screen-fit.mjs');
		await verifyScreenFit({ base, post, edit, directory });
	}
	if (process.argv.includes('--runtime-footer-ui')) {
		const { verifyRuntimeFooter } = await import('../browser/runtime-footer.mjs');
		await verifyRuntimeFooter({ base, post });
	}
	if (process.argv.includes('--screen-details-ui')) {
		const { verifyScreenDetails } = await import('../browser/screen-details.mjs');
		await verifyScreenDetails({ base, post });
	}
	if (process.argv.includes('--owned-media-ui')) {
		const { verifyOwnedMediaUi } = await import('../browser/owned-media.mjs');
		await verifyOwnedMediaUi({ base, edit, post, directory, inspectMedia: inspect });
	}
	if (process.argv.includes('--matrix-owned-media-ui')) {
		const { verifyMatrixOwnedMediaUi } = await import('../browser/matrix-owned-media.mjs');
		await verifyMatrixOwnedMediaUi({ base, edit, post, directory, inspectMedia: inspect });
	}
	if (process.argv.includes('--unavailable-reference-ui')) {
		const { verifyUnavailableReferenceUi } = await import('../browser/unavailable-reference.mjs');
		await verifyUnavailableReferenceUi({ base, edit, post, directory, inspectMedia: inspect });
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
	if (process.argv.includes('--matrix-parameters-ui')) {
		const { verifyMatrixParametersUi } = await import('../browser/matrix-parameters.mjs');
		await verifyMatrixParametersUi({ base, edit });
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
