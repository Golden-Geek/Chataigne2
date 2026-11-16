import { contentLock, persistedPanelState } from './content-lock.mjs';
import { compositionNodeMenuAction } from './composition-menu.mjs';
// Current typed Composition panels: real selection, Inspector, follow/lock, history and lifetime.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyCompositionUi({
	base,
	edit,
	post,
	directory,
	page: nativePage = null
}) {
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Panel source'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 20, height: 16 } }
	});
	const owners = [];
	for (const [label, count] of [
		['Panel A', 130],
		['Panel B', 1]
	]) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Composition', label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
		});
		await edit({
			kind: 'Composition',
			reference,
			edit: { kind: 'AddInstances', sources: Array(count).fill(source) }
		});
		owners.push(reference);
	}
	let history;
	const inspect = async (reference) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: { kind: 'Composition', reference, edit: { kind: 'Inspect' } }
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value.snapshot;
	};
	const initial = await inspect(owners[0]),
		selected = initial.instances.at(-1).reference.instance;
	await edit({
		kind: 'Composition',
		reference: owners[0],
		edit: { kind: 'RenameInstance', instance: selected, label: 'Off-page child' }
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(7000);
	const errors = [],
		requests = [];
	let stage = 'startup';
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent' && frame.intent?.payload?.kind === 'InspectCompositionEditor')
					requests.push(frame.intent.payload.query);
			} catch {}
		})
	);
	await page.addInitScript(() => {
		window.__compositionSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__compositionSockets.push(this);
			}
		};
	});
	const tab = (name) =>
		page.getByRole('tab').filter({ hasText: new RegExp('^' + name + '(?::|$)') });
	const displayedNames = new Map(
		owners.map((owner, index) => [owner.media, `Panel ${index === 0 ? 'A' : 'B'}`])
	);
	const panel = (id) =>
		page.locator('section[data-composition-locked][data-panel-id="' + id + '"]');
	const main = panel('composition'),
		widgets = screenParameterControls(page);
	async function wait(read, test) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, stage + ': ' + JSON.stringify(value));
			await delay(20);
		}
	}
	async function target(panel, reference) {
		await wait(
			() => panel.getAttribute('data-composition-channel'),
			(value) => value === reference.channel
		);
		await wait(
			() => panel.getAttribute('data-composition-loading'),
			(value) => value === 'false'
		);
		await wait(
			() =>
				panel
					.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
					.locator('.gc-dv-tab[data-panel-type="composition"] .gc-dv-tab-label')
					.innerText(),
			(value) => value === `Composition: ${displayedNames.get(reference.media)}`
		);
	}
	async function select(label) {
		await tab('Medias').click();
		const manager = page.locator('section[aria-label="Medias"]');
		const search = manager.locator('input[type="search"]');
		await search.fill(label);
		await manager
			.getByRole('treeitem', { name: label, exact: true })
			.getByRole('button', { name: label, exact: true })
			.click();
		await search.fill('');
	}
	async function inspector(label) {
		await tab('Inspector')
			.filter({ has: page.getByText(`Inspector: ${label}`, { exact: true }) })
			.waitFor();
	}
	async function selectedChild(currentPanel, id) {
		const control = currentPanel.locator('[data-instance-handle="Translate"]');
		if (!id) {
			await control.waitFor({ state: 'detached' });
			return;
		}
		await control.waitFor();
		assert.equal(await control.getAttribute('data-instance-drop-target'), id);
	}
	async function pressItem(currentPanel, reference, item, held = async () => {}) {
		const hit = currentPanel.getByRole('button', { name: `Select ${item.label}`, exact: true });
		const rect = await hit.boundingBox();
		assert(rect);
		await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
		await page.mouse.down();
		try {
			await selectedChild(currentPanel, item.reference.instance);
			await inspector(item.label);
			assert.equal(await currentPanel.locator('[data-instance-handle="Translate"]').count(), 1);
			await held();
		} finally {
			await page.mouse.up();
		}
		await wait(
			() => inspect(reference),
			() => !history.active_edit_session
		);
		await wait(
			() => currentPanel.getAttribute('data-composition-busy'),
			(value) => value === 'false'
		);
		await wait(
			() => currentPanel.getAttribute('data-composition-selected'),
			(value) => value === item.reference.instance
		);
	}
	async function ownerCleared(currentPanel, label) {
		await selectedChild(currentPanel, '');
		await inspector(label);
		assert.equal(await currentPanel.locator('[data-instance-handle]').count(), 0);
	}
	async function settled(count) {
		if (!nativePage) return;
		await page.waitForFunction((count) => {
			const views = [
				...document.querySelectorAll('section[data-composition-locked] .native-preview')
			].filter((node) => node.getBoundingClientRect().width > 0);
			return (
				views.length === count &&
				views.every(
					(node) =>
						node.dataset.previewState === 'available' && node.dataset.previewInstalled === 'true'
				) &&
				!document.querySelector('[data-native-preview-retirement]')
			);
		}, count);
	}
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	try {
		if (nativePage) await page.reload();
		else await page.goto(base);
		await select('Panel A');
		await tab('Composition').click();
		await target(main, owners[0]);
		await ownerCleared(main, 'Panel A');
		assert.equal(
			await main.getByRole('button', { name: 'Add Composition', exact: true }).count(),
			0
		);
		assert.equal(
			await main.getByRole('button', { name: 'Refresh Composition', exact: true }).count(),
			0
		);
		assert.equal(
			await main.locator('.editor-toolbar').getByText('Channel 1', { exact: true }).count(),
			0
		);
		assert.equal(
			await main.locator('input:not([aria-label="Zoom"]),select,fieldset').count(),
			0,
			'ordinary settings stay in Inspector'
		);
		stage = 'off-page selected child and generic Inspector';
		await select('Off-page child');
		await target(main, owners[0]);
		await widgets.number('Position').waitFor();
		stage = 'backend canvas transform toolbar and ordinary node menu';
		assert.equal(await main.getByRole('button', { name: 'Copy instance', exact: true }).count(), 0);
		assert.equal(
			await main.getByRole('button', { name: 'Remove instance', exact: true }).count(),
			0
		);
		const beforeTransform = await inspect(owners[0]);
		for (const [label, scale] of [
			['Fill Composition', [20, 18.75]],
			['Fit to Composition', [18.75, 18.75]]
		]) {
			await main.getByRole('button', { name: label, exact: true }).click();
			await wait(
				() => inspect(owners[0]),
				(value) =>
					isDeepStrictEqual(value.instances.at(-1).definition.transform, {
						position: [200, 150],
						scale,
						rotation_degrees: 0,
						pivot: [0.5, 0.5]
					})
			);
			await undo();
			await wait(
				() => inspect(owners[0]),
				(value) => isDeepStrictEqual(value, beforeTransform)
			);
		}
		await main.getByRole('button', { name: 'Fill Composition', exact: true }).click();
		const beforeReset = await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value.instances.at(-1).definition.transform.scale, [20, 18.75])
		);
		await compositionNodeMenuAction(page, main, 'Reset', 'Transform');
		const resetTransform = await wait(
			() => inspect(owners[0]),
			(value) =>
				isDeepStrictEqual(value.instances.at(-1).definition.transform, {
					position: [200, 150],
					scale: [1, 1],
					rotation_degrees: 0,
					pivot: [0.5, 0.5]
				})
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, beforeReset)
		);
		await redo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, resetTransform)
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, beforeReset)
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, beforeTransform)
		);
		stage = 'media enabled metadata keeps disabled Composition children visible and editable';
		stage = 'front-first ordering through toolbar, ordinary menu and overlapping pointer targets';
		const authoredOrder = await inspect(owners[0]);
		await main.getByRole('button', { name: 'To Front', exact: true }).click();
		const broughtFront = await wait(
			() => inspect(owners[0]),
			(value) => value.instances[0].reference.instance === selected
		);
		await main.getByRole('button', { name: 'To Back', exact: true }).click();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, authoredOrder)
		);
		await compositionNodeMenuAction(page, main, 'To Front', 'Arrange');
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, broughtFront)
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, authoredOrder)
		);
		await redo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, broughtFront)
		);
		await compositionNodeMenuAction(page, main, 'To Back', 'Arrange');
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, authoredOrder)
		);
		await select('Panel A');
		await ownerCleared(main, 'Panel A');
		const behind = main.getByRole('button', {
			name: `Select ${authoredOrder.instances[1].label}`,
			exact: true
		});
		const overlap = await behind.boundingBox();
		assert(overlap);
		await page.mouse.move(overlap.x + overlap.width / 2, overlap.y + overlap.height / 2);
		await page.mouse.down();
		try {
			await wait(
				() => main.getAttribute('data-composition-selected'),
				(value) => value === authoredOrder.instances[0].reference.instance
			);
		} finally {
			await page.mouse.up();
		}
		await wait(
			() => inspect(owners[0]),
			() => !history.active_edit_session
		);
		await wait(
			() => main.getAttribute('data-composition-busy'),
			(value) => value === 'false'
		);
		await select('Off-page child');
		await inspector('Off-page child');
		stage = 'media enabled metadata keeps disabled Composition children visible and editable';
		assert.equal(
			await widgets.parameter('Enabled').count(),
			0,
			'no duplicate Placement Enabled Parameter'
		);
		const initialEnabled = await inspect(owners[0]);
		const header = page.locator('.inspector[data-panel-id="inspector"] .inspector-header');
		await header.getByTitle('Node enabled', { exact: true }).click();
		const disabled = await wait(
			() => inspect(owners[0]),
			(value) => !value.instances.at(-1).definition.enabled
		);
		await inspector('Off-page child');
		await selectedChild(main, selected);
		await widgets.setNumber('Position', 124);
		await wait(
			() => inspect(owners[0]),
			(value) =>
				!history.active_edit_session &&
				!value.instances.at(-1).definition.enabled &&
				value.instances.at(-1).definition.transform.position[0] === 124
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, disabled)
		);
		await header.getByTitle('Node disabled', { exact: true }).click();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, initialEnabled)
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, disabled)
		);
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, initialEnabled)
		);
		const enableFromMenu = async (label) => {
			const manager = page.locator('section[aria-label="Medias"]');
			const search = manager.locator('input[type="search"]');
			await search.fill('Off-page child');
			await manager
				.getByRole('treeitem', { name: 'Off-page child', exact: true })
				.getByRole('button', { name: 'Off-page child', exact: true })
				.click({ button: 'right' });
			await page.getByRole('menuitem', { name: label, exact: true }).click();
			await search.fill('');
		};
		await enableFromMenu('Disable');
		await wait(
			() => inspect(owners[0]),
			(value) => !value.instances.at(-1).definition.enabled
		);
		await inspector('Off-page child');
		await selectedChild(main, selected);
		await enableFromMenu('Enable');
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, initialEnabled)
		);
		await undo();
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, initialEnabled)
		);
		const before = await inspect(owners[0]);
		await widgets.setNumber('Position', 123);
		const changed = await wait(
			() => inspect(owners[0]),
			(value) =>
				!history.active_edit_session &&
				value.instances.at(-1).definition.transform.position[0] === 123
		);
		assert.deepEqual(changed.instances.slice(0, -1), before.instances.slice(0, -1));
		await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, before)
		);
		await redo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, changed)
		);
		stage = 'zero-scale child stays selected and editable through the generic Inspector';
		for (const component of [0, 1]) {
			await widgets.setNumber('Scale', 0, component);
			await wait(
				() => inspect(owners[0]),
				(value) => value.instances.at(-1).definition.transform.scale[component] === 0
			);
			await inspector('Off-page child');
			await selectedChild(main, selected);
			await undo();
			await wait(
				() => inspect(owners[0]),
				(value) => isDeepStrictEqual(value, changed)
			);
		}
		for (const [component, value, scale] of [
			[0, 0, [0, 1]],
			[1, 0, [0, 0]],
			[0, -1, [-1, 0]],
			[1, 1, [-1, 1]]
		]) {
			await widgets.setNumber('Scale', value, component);
			const current = await wait(
				() => inspect(owners[0]),
				(snapshot) => isDeepStrictEqual(snapshot.instances.at(-1).definition.transform.scale, scale)
			);
			assert.deepEqual(current.instances.slice(0, -1), changed.instances.slice(0, -1));
			await inspector('Off-page child');
			await selectedChild(main, selected);
		}
		for (let index = 0; index < 4; index++) await undo();
		await wait(
			() => inspect(owners[0]),
			(value) => isDeepStrictEqual(value, changed)
		);
		await main.getByRole('button', { name: 'Next instances', exact: true }).click();
		await target(main, owners[0]);
		await main.getByText('Page 2 / 2', { exact: true }).waitFor();
		await widgets.setNumber('Rotation', 90);
		await wait(
			() => inspect(owners[0]),
			(value) => value.instances.at(-1).definition.transform.rotation_degrees === 90
		);
		await main.getByRole('button', { name: 'Previous instances', exact: true }).click();
		await target(main, owners[0]);
		assert.equal(
			Number(await widgets.number('Position').inputValue()),
			123,
			'off-page Inspector remains editable'
		);
		assert(
			(await main.locator('svg polygon').count()) <= 129,
			'bounded page plus independent selected object'
		);
		await main.getByRole('button', { name: 'Next instances', exact: true }).click();
		await target(main, owners[0]);
		stage = 'unrelated selection retains unlocked content and layout identity';
		await settled(1);
		const previewBefore = nativePage
			? await main.locator('.native-preview').getAttribute('data-preview-view')
			: null;
		if (nativePage) assert(previewBefore);
		await select('Panel source');
		await target(main, owners[0]);
		if (nativePage)
			assert.equal(
				await main.locator('.native-preview').getAttribute('data-preview-view'),
				previewBefore
			);
		await persistedPanelState(page, 'composition', {
			page: 1,
			locked: null,
			retained: { kind: 'Composition', reference: owners[0] }
		});
		await page.reload();
		await target(main, owners[0]);
		await main.getByText('Page 2 / 2', { exact: true }).waitFor();
		assert.equal(await main.getAttribute('data-composition-locked'), 'false');
		const lock = await contentLock(main, false);
		await lock.focus();
		await lock.press('Enter');
		await contentLock(main, true);
		await select('Panel B');
		await target(main, owners[0]);
		stage = 'independent simultaneous panels';
		const inspectorGroup = page
			.getByRole('tab')
			.filter({ hasText: /^Inspector(?:$|:)/ })
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await inspectorGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Composition', { exact: true }) })
			.click();
		const secondId = await page
			.locator('section[data-composition-locked]')
			.evaluateAll(
				(nodes) => nodes.find((node) => node.dataset.panelId !== 'composition')?.dataset.panelId
			);
		assert(secondId);
		const peer = panel(secondId);
		await target(peer, owners[1]);
		await (await contentLock(peer, false)).click();
		for (const [owner, label, currentPanel] of [
			[owners[0], 'Renamed locked A', main],
			[owners[1], 'Renamed locked B', peer]
		]) {
			const originalName = displayedNames.get(owner.media);
			await edit({ kind: 'RenameMedia', media: owner.media, label });
			displayedNames.set(owner.media, label);
			await target(currentPanel, owner);
			await edit({ kind: 'RenameMedia', media: owner.media, label: originalName });
			displayedNames.set(owner.media, originalName);
			await target(currentPanel, owner);
		}
		await settled(2);
		const boxes = await Promise.all([main.boundingBox(), peer.boundingBox()]);
		assert(boxes.every((box) => box.width > 0 && box.height > 0));
		assert(
			boxes[0].x + boxes[0].width <= boxes[1].x + 1,
			'simultaneous side-by-side workbench panels'
		);
		await persistedPanelState(page, 'composition', {
			page: 1,
			locked: { kind: 'Composition', reference: owners[0] }
		});
		await persistedPanelState(page, secondId, {
			locked: { kind: 'Composition', reference: owners[1] }
		});
		stage = 'primary-down selection and independent locked owner clearing';
		const peerBefore = await inspect(owners[1]);
		const selectionHistory = history;
		const retained = await main.getAttribute('data-composition-selected');
		await pressItem(peer, owners[1], peerBefore.instances[0], async () => {
			await selectedChild(main, retained);
			assert.equal(await main.getAttribute('data-composition-selected'), retained);
			assert.equal(await main.getAttribute('data-composition-channel'), owners[0].channel);
		});
		const zone = await peer.locator('.active-zone').boundingBox();
		assert(zone);
		await page.mouse.move(zone.x + zone.width * 0.9, zone.y + zone.height * 0.9);
		await page.mouse.down();
		try {
			await ownerCleared(peer, 'Panel B');
			await selectedChild(main, retained);
			assert.equal(await main.getAttribute('data-composition-selected'), retained);
		} finally {
			await page.mouse.up();
		}
		await pressItem(peer, owners[1], peerBefore.instances[0]);
		await peer.getByRole('application', { name: 'Composition view', exact: true }).focus();
		await page.keyboard.press('Escape');
		await ownerCleared(peer, 'Panel B');
		await page.keyboard.press('Escape');
		await ownerCleared(peer, 'Panel B');
		await selectedChild(main, retained);
		assert.equal(await main.getAttribute('data-composition-selected'), retained);
		assert.deepEqual(await inspect(owners[1]), peerBefore, 'selection preserves authored geometry');
		assert.deepEqual(history, selectionHistory, 'selection and owner clearing create no history');
		const project = path.join(directory, 'composition-panels.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		stage = 'persisted layout and real reconnect';
		await page.reload();
		await target(main, owners[0]);
		await target(peer, owners[1]);
		await settled(2);
		assert.equal(await main.getAttribute('data-composition-locked'), 'true');
		assert.equal(await peer.getAttribute('data-composition-locked'), 'true');
		await main.getByText('Page 2 / 2', { exact: true }).waitFor();
		const sockets = await page.evaluate(() => window.__compositionSockets.length);
		await page.evaluate(() => {
			const socket = window.__compositionSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw new Error('missing real application transport');
			socket.close(4001, 'Composition recovery acceptance');
		});
		await page.waitForFunction(
			(count) =>
				window.__compositionSockets.length > count &&
				window.__compositionSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		await target(main, owners[0]);
		await target(peer, owners[1]);
		await settled(2);
		stage = 'peer close, unlock follow and structural history';
		await peer
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		await settled(1);
		stage = 'locked target delete and Undo';
		await edit({ kind: 'RemoveMedia', media: owners[0].media });
		await main
			.getByText('Locked Composition unavailable in this project.', { exact: true })
			.waitFor();
		await contentLock(main, true);
		await undo();
		await target(main, owners[0]);
		await settled(1);
		await select('Panel B');
		await (await contentLock(main, true)).click();
		await target(main, owners[1]);
		const original = await inspect(owners[1]);
		await ownerCleared(main, 'Panel B');
		await pressItem(main, owners[1], original.instances[0]);
		await compositionNodeMenuAction(page, main, 'Duplicate');
		const copied = await wait(
			() => inspect(owners[1]),
			(value) => value.instances.length === 2
		);
		assert.notEqual(
			copied.instances[1].reference.instance,
			original.instances[0].reference.instance
		);
		assert.deepEqual(
			{ ...copied.instances[1].definition, source: original.instances[0].definition.source },
			original.instances[0].definition
		);
		await main
			.getByRole('button', { name: `Select ${copied.instances[1].label}`, exact: true })
			.press('Enter');
		await main
			.locator(
				`[data-instance-handle="Translate"][data-instance-drop-target="${copied.instances[1].reference.instance}"]`
			)
			.waitFor();
		await wait(
			() => main.getAttribute('data-composition-selected'),
			(value) => value === copied.instances[1].reference.instance
		);
		await target(main, owners[1]);
		await compositionNodeMenuAction(page, main, 'Delete');
		const removed = await wait(
			() => inspect(owners[1]),
			(value) => value.instances.length === 1
		);
		assert.deepEqual(removed, original, 'remove acts on the newly selected independent copy');
		await undo();
		await wait(
			() => inspect(owners[1]),
			(value) => isDeepStrictEqual(value, copied)
		);
		await redo();
		await wait(
			() => inspect(owners[1]),
			(value) => isDeepStrictEqual(value, removed)
		);
		await (await contentLock(main, false)).click();
		await target(main, owners[1]);
		await pressItem(main, owners[1], removed.instances[0]);
		stage = 'New during held pointer and document-scoped reload';
		const svg = main.getByRole('group', { name: 'Composition transform controls', exact: true });
		const handle = main.locator('[data-instance-handle="Translate"]');
		const bounds = await handle.boundingBox(),
			canvas = await svg.boundingBox();
		assert(bounds && canvas);
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
		await page.mouse.down();
		await page.mouse.move(
			bounds.x + bounds.width / 2 + (canvas.width * 20) / 400,
			bounds.y + bounds.height / 2 + (canvas.height * 10) / 300,
			{ steps: 12 }
		);
		await wait(
			() => inspect(owners[1]),
			(value) =>
				Math.abs(
					value.instances[0].definition.transform.position[0] -
						removed.instances[0].definition.transform.position[0] -
						20
				) < 2
		);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await main
			.getByText('Locked Composition unavailable in this project.', { exact: true })
			.waitFor();
		await wait(
			() =>
				main
					.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
					.locator('.gc-dv-tab[data-panel-type="composition"] .gc-dv-tab-label')
					.innerText(),
			(value) => value === 'Composition'
		);
		await page.mouse.move(canvas.x + canvas.width * 0.8, canvas.y + canvas.height * 0.8, {
			steps: 8
		});
		await page.mouse.up();
		assert.deepEqual((await edit({ kind: 'InspectManager' })).snapshot, { assets: [], media: [] });
		await settled(0);
		await post('/api/ui/project-load', { path: project });
		await target(main, owners[1]);
		await settled(1);
		assert.deepEqual(
			await inspect(owners[1]),
			original,
			'saved authoring restored, discarded held input cannot cross generations'
		);
		stage = 'tab lifetime';
		await tab('Screen Warping').click();
		await settled(0);
		await tab('Composition').click();
		await target(main, owners[1]);
		await settled(1);
		await main
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		await settled(0);
		assert(
			requests.length > 0 &&
				requests.every(
					(query) =>
						(query.count === 128 ||
							(query.count === 1 &&
								query.selected === selected &&
								query.locked === null &&
								query.retained === null)) &&
						(!query.locked || query.document)
				),
			'all current Composition reads are bounded and locks are document-scoped'
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				native: !!nativePage,
				scope:
					'bounded selected child/Inspector/history, simultaneous independent follow/lock/page state, layout reload, actual reconnect, copy/removal history, peer/tab retirement, New during held pointer and same-document Load',
				instances: 130,
				pageLimit: 128,
				ownerHasNoImplicitChild: true,
				primaryDownSelectionAndOwnerBeforeRelease: true,
				lockedPeerSelectionIndependent: true,
				idleEscapeOwnerAndIdempotence: true,
				selectionGeometryAndHistoryUnchanged: true,
				mediaEnabledMetadataAndDisabledEditing: true,
				independentSelected: selected,
				panelQueries: requests.length,
				pageErrors: errors,
				rejected
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-3500)
			})
		);
		await page.screenshot({ path: 'target/m06-panels-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser?.close();
	}
}
