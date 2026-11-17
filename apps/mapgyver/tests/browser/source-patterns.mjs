// Actual bundled calibration and related paging; optional owned-window pixel-review handshakes.
import assert from 'node:assert/strict';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
import { contentLock } from './content-lock.mjs';
const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const host = await openNativeEditor();
const { page } = host;
page.setDefaultTimeout(7000);
let token,
	stage = 'prepare';
const errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(15000),
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
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack;
}
async function edit(namespace, payload) {
	return (await intent({ kind: 'invokeAppOperation', namespace, operation: 'edit', payload }))
		.result.value;
}
const screens = (payload) => edit('mapgyver.screens', payload);
const media = (payload) => edit('mapgyver.media', payload);
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
async function settled() {
	const deadline = Date.now() + 7000;
	while ((await snapshot()).history.active_edit_session) {
		assert(Date.now() < deadline, 'edit session did not settle');
		await delay(20);
	}
}
const panel = page.locator('section.workbench-editor[data-panel-id="surface-source"]');
const native = panel.locator('.native-preview');
const controls = screenParameterControls(page);
async function bound(kind, mode) {
	await page.waitForFunction(
		({ kind, mode }) => {
			const element = document.querySelector(
				'section.workbench-editor[data-panel-id="surface-source"] .native-preview'
			);
			if (!element) return false;
			const source = JSON.parse(element.getAttribute('data-preview-source'));
			return source?.kind === kind && (!mode || source.mode === mode);
		},
		{ kind, mode }
	);
	assert.equal(await native.count(), 1);
}
async function review(name) {
	if (!process.argv.includes('--review')) return;
	await writeFile(
		path.join(directory, `review-${name}.json`),
		JSON.stringify({
			pid: host.app.pid,
			stage,
			source: await native.getAttribute('data-preview-source')
		})
	);
	console.log(JSON.stringify({ readyForReview: name, pid: host.app.pid }));
	const deadline = Date.now() + 120000;
	for (;;) {
		try {
			await access(path.join(directory, `review-${name}.done`));
			break;
		} catch {}
		assert.equal(host.app.exitCode, null, 'review host exited');
		assert(Date.now() < deadline, `${name} review expired`);
		await delay(100);
	}
	await page.waitForFunction(
		() =>
			document
				.querySelector('section.workbench-editor[data-panel-id="surface-source"] .native-preview')
				?.getAttribute('data-preview-installed') === 'true'
	);
}
try {
	token = (await post('/api/ui/session', {})).token;
	const screen = (await screens({ kind: 'InspectManager' })).snapshot.screens[0];
	const reference = screen.surfaces[0].reference;
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			enabled: false,
			target: { kind: 'AutomaticSpout', extent: { width: 320, height: 180 } }
		}
	});
	const initial = {
		...screen.surfaces[0].definition,
		crop: { x: 0.1, y: 0.15, width: 0.65, height: 0.7 }
	};
	await screens({ kind: 'ConfigureSurface', reference, definition: initial });
	await screens({ kind: 'RenameSurface', reference, label: 'Calibration Surface' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: 'Calibration Surface', exact: true })
		.click();
	await page.locator('.gc-dv-tab[data-content-panel-id="surface-source"]').click();
	await panel.getByRole('application', { name: 'Surface source view', exact: true }).waitFor();
	assert.equal(await native.count(), 0);
	const preservedCrop = (await screens({ kind: 'InspectSurface', reference })).snapshot.definition
		.crop;
	stage = 'source-free ColorBars';
	const before = await snapshot();
	await controls.choice('Calibration').selectOption('ColorBars');
	await bound('Calibration', 'ColorBars');
	const bars = (await screens({ kind: 'InspectSurface', reference })).snapshot;
	assert.equal(bars.definition.source, null);
	assert.deepEqual(bars.definition.crop, preservedCrop);
	assert.equal((await snapshot()).history.undo_len, before.history.undo_len + 1);
	await review('colorbars');
	checks.push(stage);
	stage = 'source-free official Grid';
	await controls.choice('Calibration').selectOption('Grid');
	await bound('Calibration', 'Grid');
	await review('grid');
	checks.push(stage);
	stage = 'Off and assigned-source restoration';
	await controls.choice('Calibration').selectOption('Off');
	await native.waitFor({ state: 'detached' });
	const leaf = (
		await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Preserved source' })
	).reference;
	await media({
		kind: 'ConfigureChannel',
		reference: leaf,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const definition = (await screens({ kind: 'InspectSurface', reference })).snapshot.definition;
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: { ...definition, source: leaf }
	});
	await bound('Channel');
	await controls.choice('Calibration').selectOption('Grid');
	await bound('Calibration', 'Grid');
	assert.deepEqual(
		(await screens({ kind: 'InspectSurface', reference })).snapshot.definition.source,
		leaf
	);
	await controls.choice('Calibration').selectOption('Off');
	await bound('Channel');
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await bound('Calibration', 'Grid');
	checks.push(stage);
	stage = 'pattern survives source loss and recovery';
	await settled();
	await media({ kind: 'RemoveMedia', media: leaf.media });
	await bound('Calibration', 'Grid');
	assert(
		!(await media({ kind: 'InspectManager' })).snapshot.media.some((item) => item.id === leaf.media)
	);
	await intent({ kind: 'undo' });
	assert(
		(await media({ kind: 'InspectManager' })).snapshot.media.some((item) => item.id === leaf.media)
	);
	await bound('Calibration', 'Grid');
	checks.push(stage);
	stage = 'independent pattern and channel panels';
	await (await contentLock(panel)).click();
	const peer = await screens({ kind: 'CreateScreen', label: 'Channel peer Screen' });
	const peerDefinition = (await screens({ kind: 'InspectScreen', screen: peer.screen })).definition;
	await screens({
		kind: 'ConfigureScreen',
		screen: peer.screen,
		definition: { ...peerDefinition, enabled: false }
	});
	const peerSurfaceDefinition = (await screens({ kind: 'InspectSurface', reference: peer.surface }))
		.snapshot.definition;
	await screens({
		kind: 'ConfigureSurface',
		reference: peer.surface,
		definition: { ...peerSurfaceDefinition, source: leaf }
	});
	await screens({ kind: 'RenameSurface', reference: peer.surface, label: 'Channel peer Surface' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: 'Channel peer Surface', exact: true })
		.click();
	const inspectorGroup = page
		.getByRole('tab')
		.filter({ hasText: /^Inspector(?::|$)/ })
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	await inspectorGroup
		.getByRole('button', { name: 'Add panel in this group', exact: true })
		.click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Surface Source', { exact: true }) })
		.click();
	const peerId = await page
		.locator('.gc-dv-tab[data-panel-type="surfaceSource"]')
		.evaluateAll(
			(nodes) =>
				nodes.find((node) => node.dataset.contentPanelId !== 'surface-source')?.dataset
					.contentPanelId
		);
	assert(peerId);
	await page.waitForFunction((id) => {
		const node = document.querySelector(
			`section.workbench-editor[data-panel-id="${id}"] .native-preview`
		);
		return node && JSON.parse(node.getAttribute('data-preview-source'))?.kind === 'Channel';
	}, peerId);
	await bound('Calibration', 'Grid');
	await review('mixed');
	// The mixed-panel peer is not a member of the subsequent70-Surface paging fixture.
	await screens({
		kind: 'ConfigureSurface',
		reference: peer.surface,
		definition: { ...peerSurfaceDefinition, source: null }
	});
	await page
		.locator(`.gc-dv-tab[data-content-panel-id="${peerId}"]`)
		.getByRole('button', { name: 'Close panel', exact: true })
		.click();
	checks.push(stage);
	stage = 'actual related toolbar paging';
	const graph = await snapshot();
	const parent = graph.nodes.find((node) => node.uuid === screen.id);
	assert(parent);
	await intent({
		kind: 'duplicateNodes',
		created_items: Array.from({ length: 70 }, (_, index) => ({
			source: 900000 + index,
			parent: parent.node_id,
			node_type: 'mapgyver.surface',
			label: `Related ${index}`,
			initial_params: [
				{ decl_id: 'parameters/crop_size', value: { Vec2: [0.065, 0.08] } },
				{
					decl_id: 'parameters/crop_origin',
					value: { Vec2: [0.025 + (index % 10) * 0.085, 0.03 + Math.floor(index / 10) * 0.12] }
				},
				{ decl_id: 'parameters/source_media', value: { Reference: { uuid: leaf.media } } },
				{ decl_id: 'parameters/source_channel', value: { Reference: { uuid: leaf.channel } } }
			]
		}))
	});
	await panel.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	const count = async (number) =>
		page.waitForFunction(
			(number) =>
				document.querySelectorAll(
					'section.workbench-editor[data-panel-id="surface-source"] [data-related-surface]'
				).length === number,
			number
		);
	await count(64);
	await review('related');
	const history = (await snapshot()).history;
	const first = await panel
		.locator('[data-related-surface]')
		.evaluateAll((nodes) => nodes.map((node) => node.dataset.relatedSurface));
	await panel.getByRole('button', { name: 'Next related Surfaces', exact: true }).click();
	await count(6);
	const second = await panel
		.locator('[data-related-surface]')
		.evaluateAll((nodes) => nodes.map((node) => node.dataset.relatedSurface));
	assert.equal(new Set([...first, ...second]).size, 70);
	assert(
		await panel.getByRole('button', { name: 'Next related Surfaces', exact: true }).isDisabled()
	);
	await panel.getByRole('button', { name: 'Previous related Surfaces', exact: true }).click();
	await count(64);
	assert.deepEqual((await snapshot()).history, history);
	assert.equal(await native.count(), 1);
	await bound('Calibration', 'Grid');
	checks.push(stage);
	stage = 'pattern and related view reload';
	const file = path.join(directory, 'patterns.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
	assert.equal((await post('/api/ui/project-load', { path: file })).path, file);
	await page.reload();
	await page.locator('.gc-dv-tab[data-content-panel-id="surface-source"]').click();
	await bound('Calibration', 'Grid');
	await count(64);
	checks.push(stage);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{ status: 'pass', pid: host.app.pid, reviewed: process.argv.includes('--review'), checks },
			null,
			2
		)
	);
	console.log(JSON.stringify({ status: 'pass', pid: host.app.pid, checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, log: host.log() }, null, 2)
	);
	throw error;
} finally {
	await host.close();
}
