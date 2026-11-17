// Bundled Surface Source behavior using public operations and actual toolbar/canvas input.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

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
		signal: AbortSignal.timeout(10000),
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
async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const screens = (payload) => edit('mapgyver.screens', payload);
const media = (payload) => edit('mapgyver.media', payload);
const inspect = async () => (await screens({ kind: 'InspectManager' })).snapshot;
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
const sourcePanel = page.locator('section.workbench-editor[data-panel-id="surface-source"]');
const contours = sourcePanel.locator('[data-related-surface]');
async function count(expected) {
	await page.waitForFunction(
		(expected) =>
			document.querySelectorAll(
				'section.workbench-editor[data-panel-id="surface-source"] [data-related-surface]'
			).length === expected,
		expected
	);
}
async function target(reference) {
	await page.waitForFunction(
		(reference) =>
			document
				.querySelector('section.workbench-editor[data-panel-id="surface-source"]')
				?.getAttribute('data-surface-target') === reference.surface,
		reference
	);
}
async function configure(reference, source, crop, label) {
	const definition = (await screens({ kind: 'InspectSurface', reference })).snapshot.definition;
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: { ...definition, source, crop }
	});
	await screens({ kind: 'RenameSurface', reference, label });
}
try {
	token = (await post('/api/ui/session', {})).token;
	const leaf = (
		await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Related source' })
	).reference;
	const other = (
		await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Other source' })
	).reference;
	const aliasMedia = (
		await media({ kind: 'CreateReference', target: leaf.media, label: 'Related mirror' })
	).media;
	const alias = (await media({ kind: 'InspectManager' })).snapshot.media.find(
		(entry) => entry.id === aliasMedia
	).channels[0].reference;
	const a = (await inspect()).screens[0].surfaces[0].reference;
	const b = (await screens({ kind: 'CreateScreen', label: 'Peer Screen' })).surface;
	const c = (await screens({ kind: 'CreateScreen', label: 'Mirror Screen' })).surface;
	const d = (await screens({ kind: 'CreateScreen', label: 'Other Screen' })).surface;
	await configure(a, leaf, { x: 0.05, y: 0.15, width: 0.3, height: 0.6 }, 'Source A');
	await configure(b, leaf, { x: 0.55, y: 0.2, width: 0.3, height: 0.5 }, 'Source B');
	await configure(c, alias, { x: 0.4, y: 0.6, width: 0.1, height: 0.25 }, 'Source C');
	await configure(d, other, { x: 0, y: 0, width: 1, height: 1 }, 'Source D');
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: 'Source A', exact: true })
		.click();
	await tab('surface-source').click();
	await target(a);
	stage = 'toggle and shared native source';
	await count(0);
	const before = await snapshot();
	await sourcePanel.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	await count(2);
	assert.deepEqual(
		(
			await contours.evaluateAll((nodes) => nodes.map((node) => node.dataset.relatedSurface))
		).sort(),
		[b.surface, c.surface].sort()
	);
	assert.equal(await sourcePanel.locator('.native-preview').count(), 1);
	assert.deepEqual((await snapshot()).history, before.history);
	checks.push(stage);
	stage = 'contour selection and Inspector';
	await sourcePanel
		.getByRole('button', { name: 'Select Source B on Peer Screen', exact: true })
		.click();
	await target(b);
	await page
		.locator('.inspector-header .title-text')
		.filter({ hasText: /^Source B$/ })
		.waitFor();
	await count(2);
	assert.equal(await sourcePanel.locator('.native-preview').count(), 1);
	checks.push(stage);
	stage = 'locked target and live peer geometry';
	await (await contentLock(sourcePanel)).click();
	await sourcePanel.getByRole('button', { name: 'Select Source A on Screen', exact: true }).focus();
	await page.keyboard.press('Enter');
	await target(b);
	await page
		.locator('.inspector-header .title-text')
		.filter({ hasText: /^Source A$/ })
		.waitFor();
	assert.equal(
		await sourcePanel
			.getByRole('button', { name: 'Select Source A on Screen', exact: true })
			.getAttribute('aria-pressed'),
		'true'
	);
	await screenParameterControls(page).setNumber('Crop origin', 0.1);
	await page.waitForFunction(
		(id) =>
			Math.abs(
				Number(document.querySelector(`[data-related-surface="${id}"]`)?.getAttribute('x')) - 0.1
			) < 0.005,
		a.surface
	);
	checks.push(stage);
	stage = 'independent Source panel';
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
	const peerPanel = page.locator(`section.workbench-editor[data-panel-id="${peerId}"]`);
	await peerPanel.getByRole('button', { name: 'Show related Surfaces', exact: true }).waitFor();
	assert.equal(await peerPanel.locator('[data-related-surface]').count(), 0);
	await target(b);
	await count(2);
	await peerPanel.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	await page.waitForFunction(
		(id) =>
			document.querySelectorAll(
				`section.workbench-editor[data-panel-id="${id}"] [data-related-surface]`
			).length === 2,
		peerId
	);
	assert.equal(await peerPanel.locator('.native-preview').count(), 1);
	await target(b);
	await tab(peerId).getByRole('button', { name: 'Close panel', exact: true }).click();
	checks.push(stage);
	stage = 'source changes and Undo';
	const definition = (await screens({ kind: 'InspectSurface', reference: c })).snapshot.definition;
	await screens({
		kind: 'ConfigureSurface',
		reference: c,
		definition: { ...definition, source: other }
	});
	await count(1);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await count(2);
	checks.push(stage);
	stage = 'persisted toggle and saved locked target';
	await persistedPanelState(page, 'surface-source', { related: true, locked: true });
	const file = path.join(directory, 'related.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
	assert.equal((await post('/api/ui/project-load', { path: file })).path, file);
	await page.reload();
	await tab('surface-source').click();
	await target(b);
	await count(2);
	assert.equal(await sourcePanel.getAttribute('data-content-locked'), 'true');
	await sourcePanel.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	await count(0);
	assert.equal(await sourcePanel.locator('.native-preview').count(), 1);
	checks.push(stage);
	stage = 'source loss and recovery';
	await sourcePanel.getByRole('button', { name: 'Show related Surfaces', exact: true }).click();
	await count(2);
	await media({ kind: 'RemoveMedia', media: leaf.media });
	await count(0);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await count(2);
	await page.screenshot({ path: path.join(directory, 'related.png') });
	assert.deepEqual(errors, []);
	checks.push(stage);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ status: 'pass', pid: host.app.pid, checks }, null, 2)
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
