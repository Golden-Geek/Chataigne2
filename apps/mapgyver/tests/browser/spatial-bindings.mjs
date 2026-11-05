import assert from 'node:assert/strict';
import path from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifySpatialBindings({ page, edit, post, directory, nativePid }) {
	const output = path.resolve(process.env.MAPGYVER_SPATIAL_BINDING_DIRECTORY);
	await mkdir(output, { recursive: true });
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => 'shared spatial bindings');
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.addInitScript(() => {
		window.__spatialSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__spatialSockets.push(this);
			}
		};
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.waitFor();
	await page.reload();
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Spatial unrelated'
	});
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Spatial Composition A'
	});
	await edit({
		kind: 'Composition',
		reference: composition,
		edit: { kind: 'AddInstances', sources: [source] }
	});
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Spatial Matrix B'
	});
	await edit({
		kind: 'Matrix',
		media: matrix.media,
		edit: { kind: 'AddLayers', parent: null, count: 129 }
	});
	const initial = (await edit({ kind: 'Matrix', media: matrix.media, edit: { kind: 'Inspect' } }))
		.snapshot;
	const last = initial.items.at(-1);
	await edit({
		kind: 'Matrix',
		media: matrix.media,
		edit: {
			kind: 'CreateCellMedia',
			layer: initial.items[0].id.id,
			column: initial.columns[0].id,
			media_kind: 'SolidColor'
		}
	});
	await edit({
		kind: 'Matrix',
		media: matrix.media,
		edit: {
			kind: 'ConfigureLayer',
			layer: last.id.id,
			settings: { position: [220, 120], size: { width: 64, height: 48 }, duration_ms: 250 }
		}
	});
	const compTarget = { kind: 'Composition', reference: composition },
		matrixTarget = { kind: 'Matrix', media: matrix.media };
	const panel = (id) => page.locator(`.workbench-editor[data-panel-id="${id}"]`);
	const activate = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`).click();
	async function select(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		const tree = page.getByRole('region', { name: 'Medias', exact: true });
		await tree.locator('input[type="search"]').fill(label);
		await tree.getByRole('button', { name: label, exact: true }).click();
		await tree.locator('input[type="search"]').fill('');
	}
	async function target(id, kind, media) {
		await activate(id);
		await page.waitForFunction(
			({ id, kind, media }) => {
				const node = document.querySelector(`.workbench-editor[data-panel-id="${id}"]`);
				return (
					node?.dataset.spatialKind === kind &&
					node.dataset.spatialMedia === media &&
					!!node.querySelector('.viewport-2d')
				);
			},
			{ id, kind, media }
		);
	}
	async function installed(id) {
		const view = panel(id).locator(
			'.native-preview[data-preview-installed="true"][data-preview-state="available"]'
		);
		await view.waitFor();
		const lease = await view.getAttribute('data-preview-view');
		assert(lease);
		return lease;
	}
	await select('Spatial Composition A');
	await target('composition', 'Composition', composition.media);
	await (await contentLock(panel('composition'))).press('Enter');
	await select('Spatial Matrix B');
	await target('composition', 'Composition', composition.media);
	const right = page
		.locator('.gc-dv-tab[data-panel-type="inspector"]')
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	await right.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Composition', { exact: true }) })
		.click();
	await page.waitForFunction(
		() => document.querySelectorAll('.gc-dv-tab[data-panel-type="composition"]').length === 2
	);
	const peer = await page
		.locator('.gc-dv-tab[data-panel-type="composition"]')
		.evaluateAll(
			(nodes) =>
				nodes.find((node) => node.dataset.contentPanelId !== 'composition').dataset.contentPanelId
		);
	await target(peer, 'Matrix', matrix.media);
	await (await contentLock(panel(peer))).click();
	await select('Spatial unrelated');
	await target('composition', 'Composition', composition.media);
	await target(peer, 'Matrix', matrix.media);
	// Both domains use the same viewport/navigation surface and actual native rendering contract.
	for (const id of ['composition', peer]) {
		await activate(id);
		await panel(id).getByRole('button', { name: 'Home view', exact: true }).click();
		assert.equal(await panel(id).locator('.viewport-2d').count(), 1);
	}
	await (await contentLock(panel('composition'), true)).click();
	await select('Spatial Matrix B');
	await target('composition', 'Matrix', matrix.media);
	const retainedMatrixLease = await installed('composition');
	await select('Spatial unrelated');
	await target('composition', 'Matrix', matrix.media);
	assert.equal(
		await installed('composition'),
		retainedMatrixLease,
		'unlocked unrelated selection retains the native Matrix view'
	);
	await select('Spatial Composition A');
	await target('composition', 'Composition', composition.media);
	await (await contentLock(panel('composition'))).click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.locator('[data-panel-id="matrix"]').getByRole('grid');
	await page.waitForFunction(
		(count) =>
			document
				.querySelector('[data-panel-id="matrix"] [role="grid"]')
				?.getAttribute('aria-rowcount') === String(count + 2),
		initial.items.length
	);
	await grid
		.getByTitle(`${initial.items[0].label} / ${initial.columns[0].label}`, { exact: true })
		.click({ position: { x: 5, y: 5 } });
	await target(peer, 'Matrix', matrix.media);
	await page.waitForFunction(
		({ peer, layer }) =>
			document.querySelector(`[data-panel-id="${peer}"]`)?.dataset.spatialSelected === layer,
		{ peer, layer: initial.items[0].id.id }
	);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
	});
	await grid.getByTitle(`Select ${last.label}`, { exact: true }).click();
	await target(peer, 'Matrix', matrix.media);
	await page.waitForFunction(
		({ peer, last }) =>
			document.querySelector(`[data-panel-id="${peer}"]`)?.dataset.spatialSelected === last,
		{ peer, last: last.id.id }
	);
	assert(
		(await panel(peer).locator('[data-layer-id]').count()) <= 129,
		'128 visible definitions plus independent selection'
	);
	await panel(peer).getByRole('button', { name: 'Frame selection', exact: true }).click();
	assert(
		Number(await panel(peer).locator('[data-zoom]').getAttribute('data-zoom')) > 1,
		'off-page Layer uses shared Frame navigation'
	);
	await panel(peer).getByRole('button', { name: 'Home view', exact: true }).click();
	await panel(peer).getByRole('button', { name: 'Next Layers', exact: true }).click();
	await panel(peer).getByText('Page 2 / 2', { exact: true }).waitFor();
	await edit({ kind: 'RenameMedia', media: matrix.media, label: 'Spatial renamed Matrix' });
	await page.waitForFunction(
		(peer) =>
			document.querySelector(`.gc-dv-tab[data-content-panel-id="${peer}"] .gc-dv-tab-label`)
				?.textContent === 'Composition: Spatial renamed Matrix / Matrix Layers',
		peer
	);
	await persistedPanelState(page, 'composition', { locked: compTarget, retained: compTarget });
	await persistedPanelState(page, peer, { locked: matrixTarget, retained: matrixTarget, page: 1 });
	const project = path.join(directory, 'spatial-bindings.mapgyver');
	await post('/api/ui/project-save', { path: project });
	await page.reload();
	await target('composition', 'Composition', composition.media);
	await target(peer, 'Matrix', matrix.media);
	await panel(peer).getByText('Page 2 / 2', { exact: true }).waitFor();
	await contentLock(panel('composition'), true);
	await contentLock(panel(peer), true);
	await edit({ kind: 'RemoveMedia', media: matrix.media });
	await activate(peer);
	await panel(peer)
		.getByText('Locked Matrix unavailable in this project.', { exact: true })
		.waitFor();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target(peer, 'Matrix', matrix.media);
	await page.waitForFunction(
		({ peer, last }) =>
			document.querySelector(`[data-panel-id="${peer}"]`)?.dataset.spatialSelected === last,
		{ peer, last: last.id.id }
	);
	await edit({ kind: 'RemoveMedia', media: composition.media });
	await activate('composition');
	await panel('composition')
		.getByText('Locked Composition unavailable in this project.', { exact: true })
		.waitFor()
		.catch(async (error) => {
			console.log(
				JSON.stringify({
					failed: 'missing Composition',
					text: await panel('composition').innerText(),
					errors,
					receipts
				})
			);
			throw error;
		});
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target('composition', 'Composition', composition.media);
	await target(peer, 'Matrix', matrix.media);
	const leases = [await installed('composition'), await installed(peer)],
		sockets = await page.evaluate(() => window.__spatialSockets.length);
	await page.evaluate(() =>
		window.__spatialSockets
			.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			)
			.close(4001, 'Spatial binding recovery')
	);
	await page.waitForFunction(
		(count) =>
			window.__spatialSockets.length > count &&
			window.__spatialSockets.some((socket) => socket.readyState === WebSocket.OPEN),
		sockets
	);
	await target('composition', 'Composition', composition.media);
	await target(peer, 'Matrix', matrix.media);
	assert.deepEqual([await installed('composition'), await installed(peer)], leases);
	assert.equal(
		(await edit({ kind: 'Matrix', media: matrix.media, edit: { kind: 'InspectPlayback' } }))
			.snapshot.desired_column,
		null
	);
	await page.screenshot({ path: path.join(output, 'mixed-layouts.png') });
	await post('/api/ui/project-new', {});
	for (const [id, kind] of [
		['composition', 'Composition'],
		[peer, 'Matrix']
	]) {
		await activate(id);
		await panel(id)
			.getByText(`Locked ${kind} unavailable in this project.`, { exact: true })
			.waitFor();
		assert.equal(await panel(id).locator('.native-preview').count(), 0);
	}
	const { reference: foreign } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Foreign spatial Matrix'
	});
	await select('Foreign spatial Matrix');
	for (const id of ['composition', peer])
		assert.equal(await panel(id).getAttribute('data-spatial-media'), '');
	await (await contentLock(panel('composition'), true)).click();
	await target('composition', 'Matrix', foreign.media);
	await (await contentLock(panel('composition'))).click();
	await post('/api/ui/project-load', { path: project });
	await activate('composition');
	await panel('composition')
		.getByText('Locked Matrix unavailable in this project.', { exact: true })
		.waitFor();
	await target(peer, 'Matrix', matrix.media);
	await contentLock(panel(peer), true);
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	const result = {
		result: 'pass',
		nativePid,
		bindings: [
			{ id: 'composition', initial: compTarget },
			{ id: peer, target: matrixTarget }
		],
		reconnect_leases: leases,
		page_errors: errors,
		receipts,
		scope:
			'one Composition registration with focused Composition/Matrix adapters and shared native viewport; mixed independent locks, eligible/unrelated/Cell/Layer selection, unlocked Matrix native-view retention, bounded independent off-page selection, Frame/Home, pagination/rename/layout, deletion/Undo with selected Layer retention, stable leases through reconnect, New/Load/cross-project fences and no implicit Matrix launch; M12/7 placement/Cell editing and native pixels remain separate'
	};
	await writeFile(path.join(output, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
}
