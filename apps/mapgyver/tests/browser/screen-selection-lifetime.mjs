// Extend the original Screen contract without replacing its selection/history checks.
import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { contentLock, persistedPanelState } from './content-lock.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

export async function verifyScreenSelectionLifetime({ page, post, scene, directory }) {
	let stage = 'overlap selection agrees with authored front-to-back order',
		history;
	const checks = [];
	const main = page.locator(`[data-panel-id="${scene.mainId}"]`);
	const peer = page.locator(`[data-panel-id="${scene.peerId}"]`);
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
	const surface = async () => (await edit({ kind: 'InspectSurface', reference: scene.b })).snapshot;
	async function wait(read, ready) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (ready(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const selected = (label) =>
		page
			.getByRole('tab')
			.filter({ hasText: `Inspector: ${label}` })
			.waitFor();
	const corners = () =>
		main.getByRole('group', { name: 'Warp handles for Selection B', exact: true });
	const firstPointIs = (value, x, y) => {
		const point = value.split(' ')[0].split(',').map(Number);
		return Math.abs(point[0] - x) < 0.000001 && Math.abs(point[1] - y) < 0.000001;
	};
	async function clickPoint(x, y, label) {
		const first = (await surface()).definition.quad.corners[0];
		await wait(
			() =>
				main
					.locator(`polygon[data-surface-drop-target="${scene.b.surface}"]`)
					.getAttribute('points'),
			(value) => firstPointIs(value, first.x, first.y)
		);
		const box = await main.locator('.active-zone').boundingBox();
		assert(box);
		const point = [box.x + box.width * x, box.y + box.height * y];
		const expected = label === 'Selection A' ? scene.a.surface : scene.b.surface;
		await wait(
			() =>
				main.locator('.native-preview').evaluate(async (node, point) => {
					const reply = JSON.parse(
						await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
							request: JSON.stringify({
								kind: 'Pointer',
								session: node.dataset.previewSession,
								view: node.dataset.previewView,
								source: node.dataset.previewSource,
								point
							})
						})
					);
					return reply;
				}, point),
			(reply) => reply.kind === 'Pointer' && reply.sample?.hit === expected
		);
		await page.mouse.move(...point);
		await page.mouse.down();
		try {
			await selected(label);
		} finally {
			await page.mouse.up();
		}
	}
	async function undo(original) {
		await inspect();
		const previous = history.undo_len;
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).isEnabled(),
			Boolean
		);
		await wait(
			inspect,
			(value) =>
				isDeepStrictEqual(value, original) &&
				!history.active_edit_session &&
				history.undo_len === previous - 1
		);
	}
	const captureCount = () =>
		page.evaluate(
			() =>
				[...document.querySelectorAll('*')].filter((node) =>
					node.hasPointerCapture(window.__screenPointerId ?? -1)
				).length
		);
	async function begin() {
		await clickPoint(0.7, 0.7, 'Selection B');
		await wait(
			() => main.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		const expected = (await surface()).definition.quad.corners[0];
		await wait(
			() =>
				main
					.locator(`polygon[data-surface-drop-target="${scene.b.surface}"]`)
					.getAttribute('points'),
			(value) => {
				const [x, y] = value.split(' ')[0].split(',').map(Number);
				return x === expected.x && y === expected.y;
			}
		);
		const box = await corners()
			.getByRole('button', { name: 'Warp corner 1', exact: true })
			.boundingBox();
		assert(box);
		const before = await inspect();
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await wait(captureCount, (value) => value === 1);
		await page.mouse.move(box.x + box.width / 2 + 18, box.y + box.height / 2 + 9, { steps: 4 });
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
		assert.deepEqual(await inspect(), frozen, 'Retired input cannot publish another geometry edit');
	}
	try {
		const original = await inspect();
		const definition = (await surface()).definition;
		await edit({
			kind: 'ConfigureSurface',
			reference: scene.b,
			definition: {
				...definition,
				quad: {
					corners: [
						{ x: 0.2, y: 0.2 },
						{ x: 0.5, y: 0.2 },
						{ x: 0.5, y: 0.5 },
						{ x: 0.2, y: 0.5 }
					]
				}
			}
		});
		await wait(
			() =>
				main
					.locator(`polygon[data-surface-drop-target="${scene.b.surface}"]`)
					.getAttribute('points'),
			(value) => firstPointIs(value, 0.2, 0.2)
		);
		const overlap = await inspect(),
			overlapHistory = history.current_history_state_id;
		await clickPoint(0.3, 0.3, 'Selection B');
		assert.deepEqual(await inspect(), overlap);
		assert.equal(history.current_history_state_id, overlapHistory);
		await edit({ kind: 'ReorderSurface', reference: scene.a, after: scene.b.surface });
		await wait(
			() =>
				main
					.locator('polygon[data-surface-drop-target]')
					.last()
					.getAttribute('data-surface-drop-target'),
			(value) => value === scene.a.surface
		);
		const reordered = await inspect(),
			reorderedHistory = history.current_history_state_id;
		await clickPoint(0.3, 0.3, 'Selection A');
		assert.deepEqual(await inspect(), reordered);
		assert.equal(history.current_history_state_id, reorderedHistory);
		await undo(overlap);
		await undo(original);
		checks.push({
			stage,
			before_release: true,
			selection_history_unchanged: true,
			exact_undo: true,
			scope: 'actual WebView hit order; independent native pixel/hit alignment remains separate'
		});
		stage = 'offscreen selected Surface remains editable in the real Inspector';
		await clickPoint(0.7, 0.7, 'Selection B');
		await edit({
			kind: 'ConfigureSurface',
			reference: scene.b,
			definition: {
				...definition,
				quad: { corners: definition.quad.corners.map((point) => ({ x: point.x + 3, y: point.y })) }
			}
		});
		await wait(
			() =>
				main
					.locator(`polygon[data-surface-drop-target="${scene.b.surface}"]`)
					.getAttribute('points'),
			(value) => firstPointIs(value, 3.55, 0.55)
		);
		const offscreen = await inspect();
		await page.getByRole('tab').filter({ hasText: 'Inspector: Selection B' }).click();
		await screenParameterControls(page).setNumber('Opacity', 0.6);
		await wait(
			surface,
			(value) => Math.abs(value.definition.opacity - 0.6) < 0.0001 && !history.active_edit_session
		);
		await undo(offscreen);
		await undo(original);
		checks.push({
			stage,
			typed_inspector_edit: true,
			exact_undo: true,
			scope: 'geometry outside viewport; bounded off-page catalog selection remains separate'
		});
		stage = 'selected Surface beyond the bounded page remains independently editable';
		const paged = await edit({ kind: 'CreateScreen', label: 'Paged selection screen' });
		const createdPage = await inspect();
		const createdHistory = history.undo_len;
		await edit({
			kind: 'ConfigureScreen',
			screen: paged.screen,
			definition: { enabled: true, target: null }
		});
		const emptyPage = await inspect();
		const configuredHistory = history.undo_len;
		const document = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		const parent = document.nodes.find((node) => node.uuid === paged.screen);
		const initial = document.nodes.find((node) => node.uuid === paged.surface.surface);
		assert(parent && initial);
		const batch = await post('/api/ui/intent', {
			kind: 'duplicateNodes',
			created_items: Array.from({ length: 128 }, (_, index) => ({
				source: 900000 + index,
				parent: parent.node_id,
				node_type: initial.node_type,
				label: index === 127 ? 'Off-page selected Surface' : `Paged Surface ${index}`
			}))
		});
		assert(batch.success, JSON.stringify(batch));
		const catalog = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		const last = catalog.nodes.find((node) => node.meta.label === 'Off-page selected Surface');
		assert(last);
		const query = {
			editor: 'Screen',
			selected: last.uuid,
			document: null,
			locked: null,
			retained: null,
			surface: null
		};
		const { page: bounded } = await edit({ kind: 'InspectEditor', query });
		assert.equal(bounded.screen.surfaces.length, 128);
		assert.equal(bounded.has_more_surfaces, true);
		assert(!bounded.screen.surfaces.some((item) => item.reference.surface === last.uuid));
		assert.equal(bounded.surface.reference.surface, last.uuid);
		assert.equal(bounded.parameters.length, 129);
		assert(bounded.parameters.some((binding) => binding.surface === last.uuid));
		const pagedScene = await inspect(),
			pagedHistory = history.current_history_state_id;
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens(?::|$)/ })
			.click();
		const manager = page.locator('section[aria-label="Screens"]');
		const search = manager.locator('input[type="search"]');
		await search.fill('Off-page selected Surface');
		await manager.getByRole('button', { name: 'Off-page selected Surface', exact: true }).click();
		await search.fill('');
		await selected('Off-page selected Surface');
		await main
			.getByRole('group', { name: 'Warp handles for Off-page selected Surface', exact: true })
			.waitFor();
		assert.deepEqual(await inspect(), pagedScene);
		assert.equal(
			history.current_history_state_id,
			pagedHistory,
			'Off-page selection creates no history'
		);
		await page.getByRole('tab').filter({ hasText: 'Inspector: Off-page selected Surface' }).click();
		await screenParameterControls(page).setNumber('Opacity', 0.6);
		await wait(
			async () =>
				(await edit({ kind: 'InspectSurface', reference: bounded.surface.reference })).snapshot,
			(value) => Math.abs(value.definition.opacity - 0.6) < 0.0001 && !history.active_edit_session
		);
		await undo(pagedScene);
		await undo(emptyPage);
		if (configuredHistory > createdHistory) await undo(createdPage);
		await undo(original);
		await search.fill('Selection B');
		await manager.getByRole('button', { name: 'Selection B', exact: true }).click();
		await search.fill('');
		await selected('Selection B');
		checks.push({
			stage,
			page_surfaces: 128,
			independent_selected_bindings: 129,
			typed_inspector_edit: true,
			exact_undo: true,
			scope:
				'129-Surface metadata/selection; renderer admission remains 128, no larger render qualification'
		});
		await page.locator(`.gc-dv-tab[data-content-panel-id="${scene.peerId}"]`).click();
		for (const current of [main, peer]) {
			await wait(
				() => current.getAttribute('data-screen-target'),
				(value) => value === scene.screen
			);
			await wait(
				() => current.getAttribute('data-surface-target'),
				(value) => value === scene.b.surface
			);
			await (await contentLock(current)).click();
			await persistedPanelState(page, await current.getAttribute('data-panel-id'), {
				locked: true,
				target: { kind: 'Screen', screen: scene.screen }
			});
		}
		await page.addInitScript(() => {
			window.__screenSockets = [];
			const Original = window.WebSocket;
			window.WebSocket = class extends Original {
				constructor(...args) {
					super(...args);
					window.__screenSockets.push(this);
				}
			};
			window.addEventListener(
				'pointerdown',
				(event) => {
					window.__screenPointerId = event.pointerId;
				},
				true
			);
		});
		await page.reload();
		await main.getByRole('application', { name: 'Screen view', exact: true }).waitFor();
		await main
			.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
			.waitFor();
		for (const current of [main, peer])
			await wait(
				() => current.getAttribute('data-content-locked'),
				(value) => value === 'true'
			);
		stage = 'captured Escape retires input before owner deselection';
		const held = await begin();
		await page.keyboard.press('Escape');
		await stopped(held.box);
		await selected('Selection B');
		await page.keyboard.press('Escape');
		await selected('Selection screen');
		await undo(held.before);
		checks.push({ stage, no_late_writes: true, exact_undo: true });
		stage = 'held Surface input retires on actual transport reconnect';
		const reconnect = await begin();
		const sockets = await page.evaluate(() => window.__screenSockets.length);
		await page.evaluate(() => {
			const socket = window.__screenSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw Error('Missing actual application transport');
			socket.close(4001, 'Screen selection lifetime acceptance');
		});
		await page.waitForFunction(
			(count) =>
				window.__screenSockets.length > count &&
				window.__screenSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		await stopped(reconnect.box);
		await undo(reconnect.before);
		checks.push({ stage, replacement_socket: true, no_late_writes: true, exact_undo: true });
		assert.deepEqual(await inspect(), original);
		stage = 'New during held Surface geometry retires both locked owners';
		const file = path.join(directory, 'screen-selection.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
		const outgoing = await begin();
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		for (const current of [main, peer]) {
			await current
				.getByRole('application', { name: 'Screen view', exact: true })
				.waitFor({ state: 'detached' });
			assert.equal(await current.locator('svg[aria-label="Surface mapping controls"]').count(), 0);
		}
		await wait(captureCount, (value) => value === 0);
		await page.mouse.move(outgoing.box.x + 40, outgoing.box.y + 20);
		await page.mouse.up();
		const replacement = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		assert(
			!replacement.nodes.some((node) =>
				[scene.screen, scene.a.surface, scene.b.surface].includes(node.uuid)
			)
		);
		assert.equal(replacement.history.undo_len, 0);
		assert.equal(replacement.history.redo_len, 0);
		assert.equal(replacement.history.active_edit_session, false);
		checks.push({ stage, no_late_writes: true, replacement_history_clean: true });
		stage = 'Load restores exact Screen authoring and independent locked owners';
		const loaded = await post('/api/ui/project-load', { path: file });
		assert.equal(path.resolve(loaded.path), file);
		assert.equal(loaded.recovery ?? null, null);
		for (const current of [main, peer])
			await current.getByRole('application', { name: 'Screen view', exact: true }).waitFor();
		await wait(inspect, (value) => isDeepStrictEqual(value, original));
		await clickPoint(0.7, 0.7, 'Selection B');
		assert.deepEqual(await inspect(), original);
		assert.equal(history.undo_len, 0);
		assert.equal(history.redo_len, 0);
		assert.equal(history.active_edit_session, false);
		checks.push({ stage, exact_authoring: true, clean_history: true });
		return { checks, history };
	} catch (error) {
		throw Object.assign(new Error(`${stage}: ${error.message}`, { cause: error }), {
			proof: { stage, checks, history }
		});
	} finally {
		await page.mouse.up().catch(() => {});
	}
}
