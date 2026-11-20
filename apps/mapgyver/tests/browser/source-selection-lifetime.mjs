// Held crop controls across real transport and project changes in two locked editors.
import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import {
	persistedPanelState,
	reopenWorkspace,
	saveLockedWorkspace,
	seedProjectReopen
} from './content-lock.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

export async function verifySourceSelectionLifetime({ page, post, scene, directory }) {
	let stage = 'reload independent locked Source owners',
		history,
		focusBeforeEscape;
	const checks = [];
	const file = path.join(directory, 'source-selection.mapgyver');
	const panel = (id) => page.locator(`section[data-panel-id="${id}"]`);
	const main = panel(scene.mainId),
		peer = panel(scene.peerId);
	async function edit(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value;
	}
	const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	const surface = async (item) =>
		(await edit({ kind: 'InspectSurface', reference: item.reference })).snapshot;
	async function wait(read, ready) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (ready(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const inspector = (label) =>
		page
			.locator('.inspector-header .title-text')
			.filter({ hasText: new RegExp(`^${RegExp.escape(label)}$`) })
			.waitFor();
	const captureCount = () =>
		page.evaluate(
			() =>
				[...document.querySelectorAll('*')].filter((node) =>
					node.hasPointerCapture(window.__sourcePointerId ?? -1)
				).length
		);
	async function settled() {
		for (const [current, item] of [
			[main, scene.first],
			[peer, scene.peer]
		]) {
			await wait(
				() => current.getAttribute('data-surface-target'),
				(value) => value === item.reference.surface
			);
			await wait(
				() => current.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
			assert.equal(await current.getAttribute('data-content-locked'), 'true');
		}
	}
	async function begin(current, item, corner = false) {
		await settled();
		await current.getByRole('button', { name: 'Home view', exact: true }).click();
		await wait(
			() =>
				current.locator('.active-zone').evaluate((node) => {
					const canvas = node.getBoundingClientRect();
					const stage = node.parentElement.getBoundingClientRect();
					return (
						Math.abs(canvas.x + canvas.width / 2 - stage.x - stage.width / 2) < 1 &&
						Math.abs(canvas.y + canvas.height / 2 - stage.y - stage.height / 2) < 1
					);
				}),
			Boolean
		);
		const crop = (await surface(item)).definition.crop;
		const rect = current.getByRole('button', { name: 'Select source crop', exact: true });
		await wait(
			() =>
				rect.evaluate((node) => [Number(node.getAttribute('x')), Number(node.getAttribute('y'))]),
			(value) => Math.abs(value[0] - crop.x) < 0.000001 && Math.abs(value[1] - crop.y) < 0.000001
		);
		if (corner) {
			await rect.focus();
			await page.keyboard.press('Enter');
			await wait(
				() => current.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		}
		const control = corner
			? current.getByRole('button', { name: 'Crop corner 3', exact: true })
			: rect;
		const box = await wait(async () => {
			const bounds = await control.boundingBox(),
				viewport = await current.getByRole('application').boundingBox();
			if (!bounds || !viewport) return null;
			const point = [bounds.x + bounds.width / 2, bounds.y + bounds.height / 2];
			if (
				point[0] < viewport.x ||
				point[0] > viewport.x + viewport.width ||
				point[1] < viewport.y ||
				point[1] > viewport.y + viewport.height
			)
				return null;
			if (await current.locator('.native-preview').count()) {
				const reply = await current.locator('.native-preview').evaluate(
					async (node, point) =>
						JSON.parse(
							await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
								request: JSON.stringify({
									kind: 'Pointer',
									session: node.dataset.previewSession,
									view: node.dataset.previewView,
									source: node.dataset.previewSource,
									point
								})
							})
						),
					point
				);
				if (reply.sample?.hit !== (corner ? 'crop:2' : 'crop')) return null;
			}
			return bounds;
		}, Boolean);
		const before = await inspect();
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await inspector(corner ? 'Crop size' : 'Crop origin');
		await wait(captureCount, (value) => value === 1);
		const direction = corner || crop.x > 0.2 ? -1 : 1;
		await page.mouse.move(
			box.x + box.width / 2 + direction * 18,
			box.y + box.height / 2 + direction * 9,
			{ steps: 4 }
		);
		await wait(
			inspect,
			(value) => !isDeepStrictEqual(value, before) && history.active_edit_session
		);
		return { before, box };
	}
	async function stopped(box) {
		await wait(captureCount, (value) => value === 0);
		const frozen = await wait(inspect, () => !history.active_edit_session);
		await page.mouse.move(box.x + box.width / 2 + 40, box.y + box.height / 2 + 20, { steps: 4 });
		await page.mouse.up();
		await delay(80);
		assert.deepEqual(await inspect(), frozen, 'Retired crop input cannot publish another edit');
	}
	async function undo(original) {
		await inspect();
		const previous = history.undo_len;
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			inspect,
			(value) =>
				isDeepStrictEqual(value, original) &&
				history.undo_len === previous - 1 &&
				!history.active_edit_session
		);
	}
	try {
		for (const [current, item] of [
			[main, scene.first],
			[peer, scene.peer]
		])
			await persistedPanelState(page, await current.getAttribute('data-panel-id'), {
				locked: true,
				target: { kind: 'Surface', reference: item.reference }
			});
		await seedProjectReopen(page, file);
		await page.addInitScript(() => {
			window.__sourceSockets = [];
			const Original = window.WebSocket;
			window.WebSocket = class extends Original {
				constructor(...args) {
					super(...args);
					window.__sourceSockets.push(this);
				}
			};
			window.addEventListener(
				'pointerdown',
				(event) => {
					window.__sourcePointerId = event.pointerId;
				},
				true
			);
		});
		await page.reload();
		await settled();
		await main
			.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
			.waitFor();
		const original = await inspect();
		checks.push({ stage, independent_owners: true, authoring: 'unchanged' });
		stage = 'native crop Escape retires input before idle owner selection';
		const escape = await begin(main, scene.first);
		focusBeforeEscape = await page.evaluate(() => ({
			focused: document.hasFocus(),
			element: document.activeElement?.outerHTML.slice(0, 350),
			panel: document.activeElement?.closest('[data-panel-id]')?.getAttribute('data-panel-id')
		}));
		await page.keyboard.press('Escape');
		await stopped(escape.box);
		await inspector('Crop origin');
		await page.keyboard.press('Escape');
		await inspector(scene.first.label);
		await undo(escape.before);
		checks.push({ stage, no_late_writes: true, exact_undo: true });
		stage = 'native crop capture retires on actual transport reconnect';
		const reconnect = await begin(main, scene.first);
		const sockets = await page.evaluate(() => window.__sourceSockets.length);
		await page.evaluate(() => {
			const socket = window.__sourceSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw Error('Missing actual application transport');
			socket.close(4001, 'Source lifetime acceptance');
		});
		await page.waitForFunction(
			(count) =>
				window.__sourceSockets.length > count &&
				window.__sourceSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		// Capture can retire before an already dispatched authored sample finishes.
		// Wait for the real editor gesture/read owner before freezing accepted state.
		await settled();
		await stopped(reconnect.box);
		await undo(reconnect.before);
		checks.push({ stage, replacement_socket: true, no_late_writes: true, exact_undo: true });
		stage = 'unavailable-source DOM crop Escape retires independent capture';
		const unavailable = await begin(peer, scene.peer, true);
		await page.keyboard.press('Escape');
		await stopped(unavailable.box);
		await inspector('Crop size');
		await page.keyboard.press('Escape');
		await inspector(scene.peer.label);
		await undo(unavailable.before);
		checks.push({ stage, unavailable_source: true, no_late_writes: true, exact_undo: true });
		stage = 'offscreen crop retains typed Inspector editing';
		await main.getByRole('button', { name: 'Select source crop', exact: true }).click();
		await inspector('Crop origin');
		const area = await main.getByRole('application').boundingBox();
		assert(area);
		await main.getByRole('application').focus();
		await page.keyboard.down('Space');
		await page.mouse.move(area.x + area.width / 2, area.y + area.height / 2);
		await page.mouse.down();
		await page.mouse.move(area.x + area.width * 1.8, area.y + area.height * 1.8, { steps: 8 });
		await page.mouse.up();
		await page.keyboard.up('Space');
		await wait(
			() =>
				main.getByRole('button', { name: 'Select source crop', exact: true }).evaluate((node) => {
					const crop = node.getBoundingClientRect(),
						viewport = node.closest('.editor-canvas').getBoundingClientRect();
					return (
						crop.right <= viewport.left ||
						crop.left >= viewport.right ||
						crop.bottom <= viewport.top ||
						crop.top >= viewport.bottom
					);
				}),
			Boolean
		);
		const before = await inspect();
		const previousX = (await surface(scene.first)).definition.crop.x;
		const nextX = Math.abs(previousX - 0.15) < 0.000001 ? 0.2 : 0.15;
		await screenParameterControls(page).setNumber('Crop origin', nextX);
		await wait(
			() => surface(scene.first),
			(value) =>
				Math.abs(value.definition.crop.x - nextX) < 0.000001 && !history.active_edit_session
		);
		await undo(before);
		checks.push({ stage, typed_inspector_edit: true, exact_undo: true });
		stage = 'New during held native crop retires both locked owners';
		assert.deepEqual(await inspect(), original);
		await saveLockedWorkspace(page, post, file, [scene.mainId, scene.peerId]);
		const outgoing = await begin(main, scene.first);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		const fresh = await inspect();
		assert.equal(fresh.screens.length, 1);
		assert.equal(fresh.screens[0].surfaces.length, 1);
		const initialSurface = fresh.screens[0].surfaces[0].reference.surface;
		assert(![scene.first.reference.surface, scene.peer.reference.surface].includes(initialSurface));
		for (const current of [main, peer])
			await wait(
				() => current.getAttribute('data-surface-target'),
				(value) => value === '' || value === initialSurface
			);
		await wait(captureCount, (value) => value === 0);
		await page.mouse.move(outgoing.box.x + 40, outgoing.box.y + 20);
		await page.mouse.up();
		const replacement = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		assert.equal(replacement.history.undo_len, 0);
		assert.equal(replacement.history.redo_len, 0);
		assert.equal(replacement.history.active_edit_session, false);
		assert.deepEqual(await inspect(), fresh);
		checks.push({ stage, no_late_writes: true, clean_replacement_history: true });
		stage = 'Load restores exact Source owners and typed authoring';
		await reopenWorkspace(page, post, file);
		await settled();
		assert.deepEqual(await inspect(), original);
		assert.equal(history.undo_len, 0);
		assert.equal(history.redo_len, 0);
		assert.equal(history.active_edit_session, false);
		checks.push({ stage, exact_authoring: true, independent_owners: true, clean_history: true });
		return { checks, history };
	} catch (error) {
		throw Object.assign(new Error(`${stage}: ${error.message}`, { cause: error }), {
			proof: { stage, checks, history, focusBeforeEscape }
		});
	} finally {
		await page.mouse.up().catch(() => {});
	}
}
