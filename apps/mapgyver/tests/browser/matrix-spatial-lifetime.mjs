// Real held geometry across cancellation, independent owners, reconnect and project replacement.
import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyMatrixSpatialLifetime({ page, post, edit, directory, scene }) {
	let stage = 'reload locked Layer and Cell scopes',
		history;
	const checks = [];
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const receipts = await observeNativeRejectedRequests(page, () => stage);
	const panel = (id) =>
		page.locator(`.workbench-editor[data-panel-id="${id}"][data-spatial-kind="Matrix"]`);
	const main = panel(scene.mainId),
		peer = panel(scene.peerId);
	const inspect = async () => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: { kind: 'Matrix', media: scene.media, edit: { kind: 'Inspect' } }
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value.snapshot;
	};
	async function wait(read, ready) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (ready(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const selected = (current, id) =>
		wait(
			() => current.getAttribute('data-spatial-selected'),
			(value) => value === id
		);
	const captureCount = () =>
		page.evaluate(
			() =>
				[...document.querySelectorAll('svg')].filter((node) =>
					node.hasPointerCapture(window.__matrixSpatialPointerId ?? -1)
				).length
		);
	async function settled() {
		for (const [current, level] of [
			[main, 'Cells'],
			[peer, 'Layers']
		]) {
			await wait(
				() => current.getAttribute('data-spatial-level'),
				(value) => value === level
			);
			await wait(
				() => current.getAttribute('data-spatial-media'),
				(value) => value === scene.media
			);
			await wait(
				() => current.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		}
	}
	const itemState = (snapshot, item) =>
		item.level === 'Layers'
			? snapshot.items.find((row) => row.id.id === item.id).layer
			: snapshot.cells.find((cell) => cell.id === item.id).definition.settings;
	async function begin(current, item) {
		const field = item.level === 'Layers' ? 'data-layer-id' : 'data-layout-cell';
		const rect = current.locator(`rect[${field}="${item.id}"]`);
		const before = await inspect();
		const expected = itemState(before, item);
		await wait(
			async () => ({
				busy: await current.getAttribute('aria-busy'),
				position: await rect.evaluate((node) => [
					Number(node.getAttribute('x')),
					Number(node.getAttribute('y'))
				])
			}),
			(value) => value.busy === 'false' && isDeepStrictEqual(value.position, expected.position)
		);
		const box = await rect.boundingBox();
		assert(box);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await selected(current, item.id);
		await wait(captureCount, (count) => count === 1);
		await page.mouse.move(box.x + box.width / 2 + 18, box.y + box.height / 2 + 9, { steps: 4 });
		await wait(
			inspect,
			(value) => !isDeepStrictEqual(itemState(value, item), itemState(before, item))
		);
		assert.equal(history.active_edit_session, true);
		return { before, box };
	}
	async function stopped(current, item, box) {
		await wait(captureCount, (count) => count === 0);
		const frozen = await wait(inspect, () => !history.active_edit_session);
		await page.mouse.move(box.x + box.width / 2 + 40, box.y + box.height / 2 + 20, { steps: 4 });
		await page.mouse.up();
		await delay(80);
		assert.deepEqual(
			await inspect(),
			frozen,
			'Retired pointer cannot publish geometry after cancellation'
		);
		return frozen;
	}
	async function restore(original) {
		if (!isDeepStrictEqual(await inspect(), original)) {
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(
				() => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).isEnabled(),
				Boolean
			);
		}
		await wait(inspect, (value) => isDeepStrictEqual(value, original));
		assert.equal(history.active_edit_session, false);
	}
	await page.addInitScript(() => {
		window.__matrixSpatialSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__matrixSpatialSockets.push(this);
			}
		};
		window.addEventListener(
			'pointerdown',
			(event) => {
				window.__matrixSpatialPointerId = event.pointerId;
			},
			true
		);
	});
	try {
		await page.reload();
		await settled();
		const original = await inspect();
		checks.push({ stage, main_level: 'Cells', peer_level: 'Layers', authoring: 'unchanged' });
		for (const [current, item] of [
			[peer, scene.layers[0]],
			[main, scene.cells[0]]
		]) {
			stage = `captured ${item.level} Escape retires input before owner deselection`;
			const { before, box } = await begin(current, item);
			await page.keyboard.press('Escape');
			await stopped(current, item, box);
			await selected(current, item.id);
			await page.keyboard.press('Escape');
			await selected(current, '');
			await restore(before);
			checks.push({ stage, no_late_writes: true, exact_undo: true });
		}
		stage = 'held Layer input retires on real transport reconnect';
		const { before, box } = await begin(peer, scene.layers[0]);
		const sockets = await page.evaluate(() => window.__matrixSpatialSockets.length);
		await page.evaluate(() => {
			const socket = window.__matrixSpatialSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw Error('Missing actual application transport');
			socket.close(4001, 'Matrix spatial lifetime acceptance');
		});
		await page.waitForFunction(
			(count) =>
				window.__matrixSpatialSockets.length > count &&
				window.__matrixSpatialSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		await settled();
		await stopped(peer, scene.layers[0], box);
		await restore(before);
		checks.push({ stage, replacement_socket: true, no_late_writes: true, exact_undo: true });
		stage = 'New during held Cell geometry retires both locked scopes';
		const file = path.join(directory, 'matrix-spatial.mapgyver');
		assert.deepEqual(await inspect(), original);
		assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
		const { box: heldBox } = await begin(main, scene.cells[0]);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		for (const current of [main, peer]) {
			await current
				.getByText('Locked Matrix unavailable in this project.', { exact: true })
				.waitFor();
			assert.equal(
				await current.locator('[data-matrix-layer-resize], [data-matrix-cell-resize]').count(),
				0
			);
		}
		await wait(captureCount, (count) => count === 0);
		await page.mouse.move(heldBox.x + heldBox.width / 2 + 45, heldBox.y + heldBox.height / 2 + 20);
		await page.mouse.up();
		const replacement = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		assert.equal(replacement.history.undo_len, 0);
		assert.equal(replacement.history.redo_len, 0);
		assert.equal(replacement.history.active_edit_session, false);
		checks.push({
			stage,
			captured_pointer_retired: true,
			both_owners_unavailable: true,
			replacement_history: replacement.history
		});
		stage = 'Load restores exact typed authoring and independent locked targets';
		const loaded = await post('/api/ui/project-load', { path: file });
		assert.equal(path.resolve(loaded.path), file);
		assert.equal(loaded.recovery ?? null, null);
		await settled();
		assert.deepEqual(await inspect(), original);
		await main.locator(`rect[data-layout-cell="${scene.cells[0].id}"]`).click();
		await selected(main, scene.cells[0].id);
		await wait(inspect, () => !history.active_edit_session);
		assert.deepEqual(await inspect(), original);
		assert.equal(history.undo_len, 0);
		assert.equal(history.redo_len, 0);
		assert.equal(history.active_edit_session, false);
		assert.equal(
			(await edit({ kind: 'Matrix', media: scene.media, edit: { kind: 'InspectPlayback' } }))
				.snapshot.desired_column,
			null
		);
		checks.push({ stage, exact_authoring: true, clean_history: true, unchanged_playback: true });
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		return {
			result: 'pass',
			checks,
			observed: receipts.observed,
			errors,
			rejected: receipts.rejected
		};
	} catch (error) {
		throw Object.assign(new Error(`${stage}: ${error.message}`, { cause: error }), {
			proof: { stage, checks, history }
		});
	} finally {
		await page.mouse.up().catch(() => {});
	}
}
