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
	const inspect = async (reference) =>
		(await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } })).snapshot;
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
		const before = await inspect(owners[0]);
		await widgets.setNumber('Position', 123);
		const changed = await wait(
			() => inspect(owners[0]),
			(value) => value.instances.at(-1).definition.transform.position[0] === 123
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
		await main.getByRole('button', { name: 'Lock Composition', exact: true }).click();
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
		await peer.getByRole('button', { name: 'Lock Composition', exact: true }).click();
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
		await page.waitForFunction(
			({ id, channel }) =>
				Object.entries(localStorage).some(
					([key, value]) =>
						key.endsWith('layout.v1') && value.includes(id) && value.includes(channel)
				),
			{ id: secondId, channel: owners[0].channel }
		);
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
		await select('Panel B');
		await main.getByRole('button', { name: 'Unlock Composition', exact: true }).click();
		await target(main, owners[1]);
		const original = await inspect(owners[1]);
		await main.getByRole('button', { name: 'Copy instance', exact: true }).click();
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
		await wait(
			() => main.getAttribute('data-composition-selected'),
			(value) => value === copied.instances[1].reference.instance
		);
		await target(main, owners[1]);
		await main.getByRole('button', { name: 'Remove instance', exact: true }).click();
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
		await main.getByRole('button', { name: 'Lock Composition', exact: true }).click();
		await target(main, owners[1]);
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
				requests.every((query) => query.count === 128 && (!query.locked || query.document)),
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
