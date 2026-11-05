import assert from 'node:assert/strict';
import path from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixBindings({ page, edit, post, directory, nativePid }) {
	const output = path.resolve(process.env.MAPGYVER_MATRIX_BINDING_DIRECTORY);
	await mkdir(output, { recursive: true });
	const errors = [],
		receipts = await observeNativeRejectedRequests(page, () => 'Matrix bindings');
	const reads = [];
	let readCount = 0;
	const trace = await page.context().newCDPSession(page);
	trace.on('Network.webSocketFrameSent', ({ response }) => {
		try {
			const frame = JSON.parse(response.payloadData),
				intent = frame.intent;
			if (intent?.payload?.kind === 'InspectMatrixEditor') {
				readCount++;
				reads.push(intent.payload.query);
				if (reads.length > 8) reads.shift();
			}
		} catch {}
	});
	await trace.send('Network.enable');
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.addInitScript(() => {
		window.__matrixBindingSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__matrixBindingSockets.push(this);
			}
		};
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.waitFor();
	await page.reload();
	const matrices = [];
	for (const name of ['A', 'B']) {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: `Bound Matrix ${name}`
		});
		await edit({
			kind: 'Matrix',
			media: reference.media,
			edit: { kind: 'AddLayers', parent: null, count: 61 }
		});
		await edit({
			kind: 'Matrix',
			media: reference.media,
			edit: { kind: 'AddColumns', parent: null, count: 61 }
		});
		const snapshot = (
			await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } })
		).snapshot;
		await edit({
			kind: 'Matrix',
			media: reference.media,
			edit: {
				kind: 'CreateCellMedia',
				layer: snapshot.items[0].id.id,
				column: snapshot.columns[0].id,
				media_kind: 'SolidColor'
			}
		});
		matrices.push({ reference, snapshot });
	}
	await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Unrelated binding source' });
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
	async function target(id, media) {
		await activate(id);
		await page
			.waitForFunction(
				({ id, media }) => {
					const node = document.querySelector(`.workbench-editor[data-panel-id="${id}"]`);
					return (
						node?.getAttribute('data-matrix-target') === media &&
						node.getAttribute('aria-busy') === 'false'
					);
				},
				{ id, media }
			)
			.catch(async (error) => {
				console.error(
					JSON.stringify({
						id,
						media,
						errors,
						receipts,
						reads,
						view: await panel(id).evaluate((node) => ({
							html: node.outerHTML.slice(0, 1800),
							text: node.innerText.slice(0, 1200)
						})),
						layout: await page.evaluate(() =>
							Object.fromEntries(
								Object.entries(localStorage).filter(([key]) => key.endsWith('layout.v1'))
							)
						)
					})
				);
				await page.screenshot({ path: path.join(output, 'failed-target.png') });
				throw error;
			});
	}
	async function installed(id) {
		const native = panel(id).locator(
			'.native-preview[data-preview-installed="true"][data-preview-state="available"]'
		);
		await native.waitFor();
		const lease = await native.getAttribute('data-preview-view');
		assert(lease);
		return lease;
	}
	await select('Bound Matrix A');
	await target('matrix', matrices[0].reference.media);
	const settledReads = readCount;
	await delay(250);
	const idleReads = readCount - settledReads;
	assert(idleReads <= 2, `settled viewport must not create a read loop: ${idleReads}`);
	await (await contentLock(panel('matrix'))).press('Enter');
	await select('Bound Matrix B');
	await target('matrix', matrices[0].reference.media);
	const right = page
		.locator('.gc-dv-tab[data-panel-type="inspector"]')
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	await right.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Matrix', { exact: true }) })
		.click();
	await page.waitForFunction(
		() => document.querySelectorAll('.gc-dv-tab[data-panel-type="matrix"]').length === 2
	);
	const peer = await page
		.locator('.gc-dv-tab[data-panel-type="matrix"]')
		.evaluateAll(
			(nodes) =>
				nodes.find((node) => node.dataset.contentPanelId !== 'matrix').dataset.contentPanelId
		);
	await target(peer, matrices[1].reference.media);
	// Make room for a Cell beyond the fixed row-label column in both side-by-side grids.
	const peerBox = await panel(peer).boundingBox();
	const sash = await page.locator('.dv-sash').evaluateAll(
		(nodes, x) =>
			nodes
				.map((node) => {
					const rect = node.getBoundingClientRect();
					return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
				})
				.find((rect) => rect.height > 100 && rect.width < 20 && Math.abs(rect.x - x) < 12),
		peerBox.x
	);
	assert(sash, 'right group must expose its ordinary resize sash');
	await page.mouse.move(sash.x + sash.width / 2, sash.y + sash.height / 2);
	await page.mouse.down();
	try {
		await page.mouse.move(sash.x - 240, sash.y + sash.height / 2, { steps: 15 });
	} finally {
		await page.mouse.up();
	}
	assert((await panel(peer).boundingBox()).width > 400, 'both grids expose visible Cell imagery');
	await select('Unrelated binding source');
	await target(peer, matrices[1].reference.media);
	await target('matrix', matrices[0].reference.media);
	await select('Bound Matrix A');
	await target(peer, matrices[0].reference.media);
	await select('Bound Matrix B');
	await target(peer, matrices[1].reference.media);
	await (await contentLock(panel(peer))).click();
	await edit({
		kind: 'RenameMedia',
		media: matrices[0].reference.media,
		label: 'Renamed bound Matrix'
	});
	await page.waitForFunction(
		() =>
			document.querySelector('.gc-dv-tab[data-content-panel-id="matrix"] .gc-dv-tab-label')
				?.textContent === 'Matrix: Renamed bound Matrix'
	);
	// Each panel retains its own presentation and selection while another domain object is inspected.
	await target('matrix', matrices[0].reference.media);
	await panel('matrix').getByTitle('Select Layer 1', { exact: true }).click();
	const scroll = panel('matrix').getByRole('grid');
	await scroll.evaluate((node) => {
		node.scrollTop = 900;
		node.scrollLeft = 750;
	});
	await page.waitForFunction(() => {
		const node = document.querySelector('[data-panel-id="matrix"] [role="grid"]');
		return node.scrollTop > 800 && node.scrollLeft > 600;
	});
	const offsets = await scroll.evaluate((node) => ({
		top: node.scrollTop / parseFloat(getComputedStyle(document.documentElement).fontSize),
		left: node.scrollLeft / parseFloat(getComputedStyle(document.documentElement).fontSize)
	}));
	await persistedPanelState(page, 'matrix', {
		media: matrices[0].reference.media,
		locked: true,
		scroll: offsets,
		selection: { item: matrices[0].snapshot.items[0].id, column: null }
	}).catch(async (error) => {
		console.log(
			JSON.stringify({
				expectedOffsets: offsets,
				expectedItem: matrices[0].snapshot.items[0].id,
				persisted: await page.evaluate(() =>
					Object.entries(localStorage)
						.filter(([key]) => key.endsWith('layout.v1'))
						.map(([, value]) => JSON.parse(value).payload?.panels?.matrix?.params)
				)
			})
		);
		throw error;
	});
	await target(peer, matrices[1].reference.media);
	assert.equal(
		await panel(peer)
			.getByRole('grid')
			.evaluate((node) => node.scrollTop),
		0
	);
	await persistedPanelState(page, peer, { media: matrices[1].reference.media, locked: true });
	const project = path.join(directory, 'matrix-bindings.mapgyver');
	await post('/api/ui/project-save', { path: project });
	await page.reload();
	await target('matrix', matrices[0].reference.media);
	await page.waitForFunction(({ top, left }) => {
		const node = document.querySelector('[data-panel-id="matrix"] [role="grid"]'),
			rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
		return (
			Math.abs(node.scrollTop / rem - top) < 0.1 && Math.abs(node.scrollLeft / rem - left) < 0.1
		);
	}, offsets);
	await contentLock(panel('matrix'), true);
	await target(peer, matrices[1].reference.media);
	await contentLock(panel(peer), true);
	await edit({ kind: 'RemoveMedia', media: matrices[0].reference.media });
	await activate('matrix');
	await panel('matrix')
		.getByText('Locked Matrix unavailable in this project.', { exact: true })
		.waitFor();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target('matrix', matrices[0].reference.media);
	// Return both views to visible media for actual native lease/reconnect assertions.
	await panel('matrix')
		.getByRole('grid')
		.evaluate((node) => {
			node.scrollTop = 0;
			node.scrollLeft = 0;
		});
	const leases = [await installed('matrix')];
	await target(peer, matrices[1].reference.media);
	leases.push(await installed(peer));
	const sockets = await page.evaluate(() => window.__matrixBindingSockets.length);
	await page.evaluate(() =>
		window.__matrixBindingSockets
			.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			)
			.close(4001, 'Matrix binding recovery')
	);
	await page.waitForFunction(
		(count) =>
			window.__matrixBindingSockets.length > count &&
			window.__matrixBindingSockets.some((socket) => socket.readyState === WebSocket.OPEN),
		sockets
	);
	await target('matrix', matrices[0].reference.media);
	await target(peer, matrices[1].reference.media);
	assert.deepEqual([await installed('matrix'), await installed(peer)], leases);
	for (const { reference } of matrices)
		assert.equal(
			(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'InspectPlayback' } }))
				.snapshot.desired_column,
			null
		);
	await page.screenshot({ path: path.join(output, 'independent-matrices.png') });
	await post('/api/ui/project-new', {});
	for (const id of ['matrix', peer]) {
		await activate(id);
		await panel(id)
			.getByText('Locked Matrix unavailable in this project.', { exact: true })
			.waitFor();
	}
	await post('/api/ui/project-load', { path: project });
	for (const [i, id] of ['matrix', peer].entries()) {
		await target(id, matrices[i].reference.media);
		await contentLock(panel(id), true);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	const result = {
		result: 'pass',
		nativePid,
		bindings: ['matrix', peer].map((id, i) => ({ id, media: matrices[i].reference.media })),
		scroll: offsets,
		reconnect_leases: leases,
		idle_reads_over_250ms: idleReads,
		page_errors: errors,
		receipts,
		scope:
			'two retained Matrix grids, shared tab locks/keyboard, relevant/unrelated selection, independent scroll/selection/layout, rename, deletion/Undo, reconnect with stable native leases, New/Load, no implicit launch; DOM and native ownership, no physical pixel claim'
	};
	await writeFile(path.join(output, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
}
