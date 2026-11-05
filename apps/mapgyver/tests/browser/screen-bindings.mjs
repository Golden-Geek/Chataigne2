import assert from 'node:assert/strict';
import path from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyScreenBindings({ page, edit, post, directory, nativePid }) {
	const output = path.resolve(process.env.MAPGYVER_SCREEN_BINDING_DIRECTORY);
	await mkdir(output, { recursive: true });
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => 'Screen bindings');
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.addInitScript(() => {
		window.__screenBindingSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__screenBindingSockets.push(this);
			}
		};
	});
	await page.reload();
	const screens = async (payload) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	};
	const refs = [];
	for (const label of ['A', 'B']) {
		const { reference: source } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: `Binding source ${label}`
		});
		const { screen, surface } = await screens({
			kind: 'CreateScreen',
			label: `Binding screen ${label}`
		});
		await screens({ kind: 'RenameSurface', reference: surface, label: `Binding surface ${label}` });
		const definition = (await screens({ kind: 'InspectSurface', reference: surface })).snapshot
			.definition;
		await screens({
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...definition, source }
		});
		await screens({
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				target: { kind: 'Spout', sender: `Binding-${screen}`, extent: { width: 320, height: 180 } }
			}
		});
		refs.push({ source, screen, surface });
	}
	const panel = (id) => page.locator(`.workbench-editor[data-panel-id="${id}"]`);
	const activate = async (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`).click();
	async function select(label, manager = 'Screens') {
		await page
			.getByRole('tab')
			.filter({ hasText: new RegExp(`^${manager}$`) })
			.click();
		const tree = page.getByRole('region', { name: manager, exact: true });
		const search = tree.locator('input[type="search"]');
		await search.fill(label);
		await tree.getByRole('button', { name: label, exact: true }).click();
		await search.fill('');
	}
	async function target(id, screen, surface = null) {
		await activate(id);
		await page.waitForFunction(
			({ id, screen, surface }) => {
				const node = document.querySelector(`.workbench-editor[data-panel-id="${id}"]`);
				return (
					node?.getAttribute('data-screen-target') === screen &&
					(surface === null || node.getAttribute('data-surface-target') === surface) &&
					node.getAttribute('aria-busy') === 'false'
				);
			},
			{ id, screen, surface }
		);
	}
	const right = page
		.locator('.gc-dv-tab[data-panel-type="inspector"]')
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	async function peer(name, type) {
		const before = await page
			.locator(`.gc-dv-tab[data-panel-type="${type}"]`)
			.evaluateAll((nodes) => nodes.map((node) => node.dataset.contentPanelId));
		await right.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText(name, { exact: true }) })
			.click();
		await page.waitForFunction(
			({ type, before }) =>
				[...document.querySelectorAll(`.gc-dv-tab[data-panel-type="${type}"]`)].some(
					(node) => !before.includes(node.dataset.contentPanelId)
				),
			{ type, before }
		);
		return page
			.locator(`.gc-dv-tab[data-panel-type="${type}"]`)
			.evaluateAll(
				(nodes, before) =>
					nodes.find((node) => !before.includes(node.dataset.contentPanelId)).dataset
						.contentPanelId,
				before
			);
	}
	await select('Binding surface A');
	await target('screen-warp', refs[0].screen, refs[0].surface.surface);
	await (await contentLock(panel('screen-warp'))).press('Enter');
	await select('Binding surface B');
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	await target('screen-warp', refs[0].screen, refs[0].surface.surface);
	const warpPeer = await peer('Screen Warping', 'screenWarp');
	await target(warpPeer, refs[1].screen, refs[1].surface.surface);
	await (await contentLock(panel(warpPeer))).click();
	await select('Binding source A', 'Medias');
	await target('screen-warp', refs[0].screen);
	await target(warpPeer, refs[1].screen);
	const beforeDrop = (await screens({ kind: 'InspectSurface', reference: refs[1].surface }))
		.snapshot;
	// Trusted outliner drag exercises complete backend hit resolution, not a synthetic drop intent.
	const label = page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Binding source A', exact: true });
	await label.hover();
	await page.mouse.down();
	try {
		const canvas = panel(warpPeer).locator('svg[aria-label="Surface mapping controls"]');
		await canvas.hover();
		await canvas.hover();
	} finally {
		await page.mouse.up();
	}
	async function sourceSnapshot(media) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const snapshot = (await screens({ kind: 'InspectSurface', reference: refs[1].surface }))
				.snapshot;
			if (snapshot.definition.source?.media === media) return snapshot;
			assert(Date.now() < deadline, 'trusted Screen drop/history did not reach its actual Surface');
			await delay(25);
		}
	}
	const changed = await sourceSnapshot(refs[0].source.media);
	assert.deepEqual(
		{ ...changed.definition, source: beforeDrop.definition.source },
		beforeDrop.definition
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	assert.deepEqual(await sourceSnapshot(refs[1].source.media), beforeDrop);
	await writeFile(
		path.join(output, 'drop-source.json'),
		JSON.stringify({ before: beforeDrop, after: changed, undo: beforeDrop }, null, 2)
	);
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	await (await contentLock(panel('surface-source'))).click();
	await select('Binding surface A');
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	const sourcePeer = await peer('Surface Source', 'surfaceSource');
	await target(sourcePeer, refs[0].screen, refs[0].surface.surface);
	await (await contentLock(panel(sourcePeer))).click();
	const bindings = [
		['screen-warp', { kind: 'Screen', screen: refs[0].screen }, refs[0]],
		[warpPeer, { kind: 'Screen', screen: refs[1].screen }, refs[1]],
		['surface-source', { kind: 'Surface', reference: refs[1].surface }, refs[1]],
		[sourcePeer, { kind: 'Surface', reference: refs[0].surface }, refs[0]]
	];
	for (const [id, payload, renamed] of [
		[
			'screen-warp',
			{ kind: 'RenameScreen', screen: refs[0].screen, label: 'Renamed bound Screen' },
			'Screen Warping: Renamed bound Screen'
		],
		[
			'surface-source',
			{ kind: 'RenameSurface', reference: refs[1].surface, label: 'Renamed bound Surface' },
			'Surface Source: Renamed bound Surface'
		]
	]) {
		await screens(payload);
		await page.waitForFunction(
			({ id, renamed }) =>
				document.querySelector(`.gc-dv-tab[data-content-panel-id="${id}"] .gc-dv-tab-label`)
					?.textContent === renamed,
			{ id, renamed }
		);
	}
	for (const [id, binding] of bindings)
		await persistedPanelState(page, id, { locked: true, target: binding });
	await page.screenshot({ path: path.join(output, 'independent-sources.png') });
	const project = path.join(directory, 'screen-bindings.mapgyver');
	await post('/api/ui/project-save', { path: project });
	await page.reload();
	for (const [id, , ref] of bindings) {
		await target(id, ref.screen);
		await contentLock(panel(id), true);
	}
	await screens({ kind: 'RemoveSurface', reference: refs[1].surface });
	await activate('surface-source');
	await panel('surface-source')
		.getByText('Locked target unavailable in this project.', { exact: true })
		.waitFor();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	await screens({ kind: 'RemoveScreen', screen: refs[0].screen });
	await activate(sourcePeer);
	await panel(sourcePeer)
		.getByText('Locked target unavailable in this project.', { exact: true })
		.waitFor();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target(sourcePeer, refs[0].screen, refs[0].surface.surface);
	async function installed(id) {
		const view = panel(id).locator(
			'.native-preview[data-preview-state="available"][data-preview-installed="true"]'
		);
		await view.waitFor();
		const lease = await view.getAttribute('data-preview-view');
		assert(lease);
		return lease;
	}
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	const leases = [await installed('surface-source'), await installed(sourcePeer)];
	const sockets = await page.evaluate(() => window.__screenBindingSockets.length);
	await page.evaluate(() =>
		window.__screenBindingSockets
			.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			)
			.close(4001, 'Screen binding recovery')
	);
	await page.waitForFunction(
		(count) =>
			window.__screenBindingSockets.length > count &&
			window.__screenBindingSockets.some((socket) => socket.readyState === WebSocket.OPEN),
		sockets
	);
	await target('surface-source', refs[1].screen, refs[1].surface.surface);
	await target(sourcePeer, refs[0].screen, refs[0].surface.surface);
	assert.deepEqual([await installed('surface-source'), await installed(sourcePeer)], leases);
	for (const [id, , ref] of bindings) await target(id, ref.screen);
	await post('/api/ui/project-new', {});
	for (const [id] of bindings) {
		await activate(id);
		await panel(id)
			.getByText('Locked target unavailable in this project.', { exact: true })
			.waitFor();
	}
	await post('/api/ui/project-load', { path: project });
	for (const [id, , ref] of bindings) {
		await target(id, ref.screen);
		await contentLock(panel(id), true);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	const result = {
		result: 'pass',
		nativePid,
		bindings: bindings.map(([id, target]) => ({ id, target })),
		reconnect_leases: leases,
		page_errors: errors,
		receipts,
		scope:
			'two independent Screen and two independent Surface Source locks, keyboard, unrelated/relevant selection, real source drop, layout restore, deletion/Undo, reconnect and New/Load document fencing; DOM/native lease scope only'
	};
	await writeFile(path.join(output, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
}
