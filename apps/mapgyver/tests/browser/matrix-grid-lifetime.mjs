import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { contentLock, persistedPanelState } from './content-lock.mjs';

export async function verifyMatrixGridLifetime({ page, post, edit, directory, mainId, mainMedia }) {
	let stage = 'independent grids';
	let history;
	async function inspect(media) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: { kind: 'Matrix', media, edit: { kind: 'Inspect' } }
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value.snapshot;
	}
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const panel = (id) => page.locator(`section[data-panel-id="${id}"]`);
	const main = panel(mainId);
	const grid = (current) =>
		current.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const inspector = page.locator('.inspector');
	async function target(id, media) {
		await page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`).click();
		const current = panel(id);
		await wait(
			() => current.getAttribute('data-matrix-target'),
			(value) => value === media
		);
		await wait(
			() => current.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
	}
	const inspected = (id) =>
		wait(
			() => inspector.getAttribute('data-inspected-uuid'),
			(value) => value === id
		);
	const header = (current, id) => grid(current).locator(`[role="rowheader"][data-row-id="${id}"]`);
	async function row(current, id) {
		const label = header(current, id).locator('.row-title-label button');
		await label.scrollIntoViewIfNeeded();
		const box = await label.boundingBox();
		assert(box);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		try {
			await inspected(id);
			await wait(
				() => header(current, id).getAttribute('aria-selected'),
				(value) => value === 'true'
			);
		} finally {
			await page.mouse.up();
		}
	}
	try {
		await target(mainId, mainMedia);
		await (await contentLock(main)).click();
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Independent selection Matrix'
		});
		const mediasTab = page.getByRole('tab').filter({ hasText: /^Medias$/ });
		await mediasTab.click();
		const manager = page.getByRole('region', { name: 'Medias', exact: true });
		await manager.locator('input[type="search"]').fill('Independent selection Matrix');
		await manager
			.getByRole('button', { name: 'Independent selection Matrix', exact: true })
			.click();
		await manager.locator('input[type="search"]').fill('');
		const group = mediasTab.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Matrix', { exact: true }) })
			.click();
		const tabs = page.locator('.gc-dv-tab[data-panel-type="matrix"]');
		await wait(
			() => tabs.count(),
			(count) => count === 2
		);
		const peerId = await tabs.evaluateAll(
			(nodes, main) =>
				nodes.find((node) => node.dataset.contentPanelId !== main)?.dataset.contentPanelId,
			mainId
		);
		assert(peerId);
		const peer = panel(peerId);
		await target(peerId, reference.media);
		await (await contentLock(peer)).click();
		await grid(main).evaluate((element) => {
			element.scrollTop = 0;
			element.scrollLeft = 0;
			element.dispatchEvent(new Event('scroll'));
		});
		const beforeMain = await inspect(mainMedia);
		const beforePeer = await inspect(reference.media);
		const beforeHistory = history;
		const mainRow = beforeMain.items[0].id.id;
		const peerRow = beforePeer.items[1].id.id;
		await target(mainId, mainMedia);
		await row(main, mainRow);
		await target(peerId, reference.media);
		await row(peer, peerRow);
		assert.equal(await header(main, mainRow).getAttribute('aria-selected'), 'true');
		await grid(peer).focus();
		await page.keyboard.press('Escape');
		await inspected(reference.media);
		assert.equal(await header(main, mainRow).getAttribute('aria-selected'), 'true');
		await wait(
			() => grid(peer).locator('[aria-selected="true"]').count(),
			(count) => count === 0
		);
		await row(peer, peerRow);
		assert.deepEqual(await inspect(mainMedia), beforeMain);
		assert.deepEqual(await inspect(reference.media), beforePeer);
		assert.deepEqual(history, beforeHistory, 'independent-grid selection cannot author history');
		stage = 'layout reload retains independent targets and selections';
		await persistedPanelState(page, mainId, {
			media: mainMedia,
			locked: true,
			selection: { item: beforeMain.items[0].id, column: beforeMain.columns[0].id }
		});
		await persistedPanelState(page, peerId, {
			media: reference.media,
			locked: true,
			selection: { item: beforePeer.items[1].id, column: null }
		});
		const project = path.join(directory, 'm14-independent-grid.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		await page.reload();
		await target(mainId, mainMedia);
		await target(peerId, reference.media);
		await contentLock(main, true);
		await contentLock(peer, true);
		await wait(
			() => header(main, mainRow).getAttribute('aria-selected'),
			(value) => value === 'true'
		);
		await wait(
			() => header(peer, peerRow).getAttribute('aria-selected'),
			(value) => value === 'true'
		);
		assert.deepEqual(await inspect(mainMedia), beforeMain);
		assert.deepEqual(await inspect(reference.media), beforePeer);
		assert.deepEqual(history, beforeHistory);
		stage = 'New during held input withdraws both locked targets';
		const box = await header(peer, peerRow).locator('.row-title-label button').boundingBox();
		assert(box);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		try {
			assert.equal((await post('/api/ui/project-new', {})).ok, true);
			for (const current of [main, peer])
				await current
					.getByText('Locked Matrix unavailable in this project.', { exact: true })
					.waitFor();
			await inspected('');
		} finally {
			await page.mouse.up();
		}
		await inspected('');
		stage = 'Load recovers exact authoring and targets with clean history';
		const loaded = await post('/api/ui/project-load', { path: project });
		assert.equal(path.resolve(loaded.path), project);
		assert.equal(loaded.recovery ?? null, null);
		await target(mainId, mainMedia);
		await target(peerId, reference.media);
		assert.deepEqual(await inspect(mainMedia), beforeMain);
		assert.deepEqual(await inspect(reference.media), beforePeer);
		assert.equal(history.undo_len, 0);
		assert.equal(history.redo_len, 0);
		assert.equal(history.active_edit_session, false);
		for (const media of [mainMedia, reference.media])
			assert.equal(
				(await edit({ kind: 'Matrix', media, edit: { kind: 'InspectPlayback' } })).snapshot
					.desired_column,
				null
			);
		await page
			.locator(`.gc-dv-tab[data-content-panel-id="${peerId}"]`)
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		await (await contentLock(main, true)).click();
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'two real locked grids and visible Inspector, independent before-release selection, exact history, layout reload, New during held input and exact Load recovery',
				native: false
			})
		);
	} catch (error) {
		throw new Error(`${stage}: ${error.message}`, { cause: error });
	}
}
