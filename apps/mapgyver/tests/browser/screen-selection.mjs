import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyScreenSelection({ base, post, page: nativePage = null }) {
	let history;
	async function screens(payload) {
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
	const { screen, surface: a } = await screens({ kind: 'CreateScreen', label: 'Selection screen' });
	await screens({ kind: 'RenameSurface', reference: a, label: 'Selection A' });
	const { reference: b } = await screens({ kind: 'AddSurface', screen, label: 'Selection B' });
	const quad = (x, y, w, h) => ({
		corners: [
			{ x, y },
			{ x: x + w, y },
			{ x: x + w, y: y + h },
			{ x, y: y + h }
		]
	});
	for (const [reference, shape] of [
		[a, quad(0.1, 0.1, 0.3, 0.3)],
		[b, quad(0.55, 0.55, 0.35, 0.35)]
	])
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: {
				source: null,
				crop: { x: 0, y: 0, width: 1, height: 1 },
				quad: shape,
				opacity: 1,
				enabled: true,
				calibration: 'ColorBars'
			}
		});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Selection-${screen}`, extent: { width: 640, height: 360 } }
		}
	});
	const before = (await screens({ kind: 'InspectManager' })).snapshot;
	const beforeHistory = history.current_history_state_id;
	const beforeUndo = history.undo_len;
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Screen selection');
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	const main = page.locator('[data-panel-id="screen-warp"]');
	const groups = (target) => target.getByRole('group', { name: /^Warp handles for / });
	const handles = (target, label) =>
		target.getByRole('group', { name: `Warp handles for Selection ${label}`, exact: true });
	async function point(target, x, y) {
		const rect = await target.locator('.active-zone').boundingBox();
		assert(rect);
		return { x: rect.x + rect.width * x, y: rect.y + rect.height * y };
	}
	async function mouse(target, x, y, click = false) {
		const p = await point(target, x, y);
		await page.mouse.move(p.x, p.y);
		if (click) await page.mouse.click(p.x, p.y);
	}
	async function count(target, total) {
		await page.waitForFunction(
			({ id, total }) =>
				document.querySelectorAll(`[data-panel-id="${id}"] .viewport-handle:not(.ghost)`).length ===
				total,
			{ id: await target.getAttribute('data-panel-id'), total }
		);
	}
	async function inspector(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: new RegExp(`^Inspector: ${label}$`) })
			.waitFor();
	}
	async function installed() {
		if (nativePage) {
			await main
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
			await main.locator('.native-preview .unavailable').waitFor({ state: 'detached' });
		}
	}
	try {
		if (!nativePage) await page.goto(base);
		await tab('Screen Manager').click();
		await page
			.locator('section[aria-label="Screen Manager"]')
			.getByRole('button', { name: 'Selection screen', exact: true })
			.click();
		await tab('Screen Warping').click();
		await installed();
		await mouse(main, 0.48, 0.15);
		await count(main, 0);
		await mouse(main, 0.25, 0.25);
		await count(main, 0);
		await mouse(main, 0.25, 0.25, true);
		await handles(main, 'A').waitFor();
		await inspector('Selection A');
		await mouse(main, 0.7, 0.7);
		await count(main, 4);
		await mouse(main, 0.48, 0.15);
		await count(main, 4);
		const corner = handles(main, 'A').getByRole('button', { name: 'Warp corner 2', exact: true });
		await corner.click();
		const jitter = await corner.boundingBox();
		assert(jitter);
		await page.mouse.move(jitter.x + jitter.width / 2, jitter.y + jitter.height / 2);
		await page.mouse.down();
		await page.mouse.move(jitter.x + jitter.width / 2 + 1, jitter.y + jitter.height / 2 + 1);
		await page.mouse.up();
		await page.waitForFunction(
			(id) =>
				document
					.querySelector(`[data-panel-id="${id}"] [aria-label="Warp corner 2"]`)
					?.getAttribute('aria-pressed') === 'true',
			'screen-warp'
		);
		await inspector('Selection A');
		await mouse(main, 0.48, 0.15, true);
		await count(main, 0);
		await inspector('Selection screen');
		assert.equal(
			await main.getByRole('button', { name: 'Refresh preview', exact: true }).count(),
			0
		);
		await main.getByRole('button', { name: 'Home view', exact: true }).click();
		await count(main, 0);
		await mouse(main, 0.25, 0.25, true);
		await corner.click();
		const application = main.getByRole('application', { name: 'Screen view', exact: true });
		await application.focus();
		await page.keyboard.press('Escape');
		assert.equal(await corner.getAttribute('aria-pressed'), 'false');
		await inspector('Selection A');
		await application.focus();
		await page.keyboard.down('Space');
		await mouse(main, 0.48, 0.15, true);
		await page.keyboard.up('Space');
		await inspector('Selection A');
		await count(main, 4);
		// Two live editors share Surface selection, but never corner selection.
		const inspectorGroup = page
			.getByRole('tab')
			.filter({ hasText: /^Inspector:/ })
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await inspectorGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Screen Warping', { exact: true }) })
			.click();
		const secondId = await page
			.locator('.workbench-editor[data-panel-type="screenWarp"]')
			.evaluateAll(
				(elements) =>
					elements.find((element) => element.dataset.panelId !== 'screen-warp')?.dataset.panelId
			);
		assert(secondId);
		const peer = page.locator(`[data-panel-id="${secondId}"]`);
		const toolbar = peer.locator('.canvas-toolbar');
		const panelBox = await peer.boundingBox(),
			toolbarBox = await toolbar.boundingBox();
		assert(
			panelBox &&
				toolbarBox &&
				toolbarBox.width <= panelBox.width &&
				Math.abs(toolbarBox.x + toolbarBox.width / 2 - panelBox.x - panelBox.width / 2) < 1,
			'narrow peer toolbar remains inside its canvas and centered'
		);
		await handles(peer, 'A').waitFor();
		if (nativePage) {
			await peer
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
			await peer.locator('.native-preview .unavailable').waitFor({ state: 'detached' });
		}
		await corner.click();
		const peerCorner = handles(peer, 'A').getByRole('button', {
			name: 'Warp corner 3',
			exact: true
		});
		await peerCorner.click();
		await page.waitForFunction(
			(id) =>
				document
					.querySelector(`[data-panel-id="${id}"] [aria-label="Warp corner 3"]`)
					?.getAttribute('aria-pressed') === 'true',
			secondId
		);
		assert.equal(await corner.getAttribute('aria-pressed'), 'true');
		assert.equal(await peerCorner.getAttribute('aria-pressed'), 'true');
		assert.equal(
			await handles(peer, 'A')
				.getByRole('button', { name: 'Warp corner 2', exact: true })
				.getAttribute('aria-pressed'),
			'false'
		);
		assert.equal(
			await handles(main, 'A')
				.getByRole('button', { name: 'Warp corner 3', exact: true })
				.getAttribute('aria-pressed'),
			'false'
		);
		await mouse(main, 0.7, 0.7, true);
		await handles(peer, 'B').waitFor();
		await mouse(peer, 0.48, 0.15);
		await count(peer, 4);
		await count(main, 4);
		assert.equal(await main.locator('.viewport-handle[aria-pressed="true"]').count(), 0);
		assert.equal(await peer.locator('.viewport-handle[aria-pressed="true"]').count(), 0);
		assert.deepEqual(
			(await screens({ kind: 'InspectManager' })).snapshot,
			before,
			'selection/corner clicks/pan do not edit authored geometry'
		);
		assert.equal(
			history.current_history_state_id,
			beforeHistory,
			'selection creates no Undo steps'
		);
		const drag = handles(main, 'B').getByRole('button', { name: 'Warp corner 1', exact: true });
		const origin = await drag.boundingBox(),
			destination = await point(main, 0.58, 0.57);
		assert(origin);
		await page.mouse.move(origin.x + origin.width / 2, origin.y + origin.height / 2);
		await page.mouse.down();
		await page.mouse.move(destination.x, destination.y, { steps: 8 });
		await page.mouse.up();
		async function waitSurface(test) {
			const deadline = Date.now() + 6000;
			for (;;) {
				const { snapshot } = await screens({ kind: 'InspectSurface', reference: b });
				if (test(snapshot) && !history.active_edit_session) return snapshot;
				assert(Date.now() < deadline, JSON.stringify({ snapshot, history }));
				await delay(20);
			}
		}
		await waitSurface(
			(snapshot) =>
				Math.abs(snapshot.definition.quad.corners[0].x - 0.58) < 0.005 &&
				Math.abs(snapshot.definition.quad.corners[0].y - 0.57) < 0.005
		);
		assert.equal(history.undo_len, beforeUndo + 1, 'one pointer drag creates one Undo gesture');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await waitSurface(
			(snapshot) =>
				Math.abs(snapshot.definition.quad.corners[0].x - 0.55) < 0.005 &&
				Math.abs(snapshot.definition.quad.corners[0].y - 0.55) < 0.005
		);
		assert.deepEqual(
			(await screens({ kind: 'InspectManager' })).snapshot,
			before,
			'Undo restores the complete authored scene'
		);
		await post('/api/ui/project-new', {});
		await main
			.getByRole('application', { name: 'Screen view', exact: true })
			.waitFor({ state: 'detached' });
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				screenSelection:
					'Surface/Inspector and background, selected-only handles, persistent deselection, local per-editor corners, Escape and pan isolation',
				selectionGeometryAndHistoryUnchanged: true,
				pointerDragOneUndo: true,
				native: !!nativePage,
				pageErrors: []
			})
		);
	} finally {
		await browser?.close();
	}
}
