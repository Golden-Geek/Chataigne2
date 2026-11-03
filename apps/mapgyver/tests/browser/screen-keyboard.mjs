import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyScreenKeyboard({ base, post, page: nativePage = null }) {
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
	const { screen, surface: reference } = await screens({
		kind: 'CreateScreen',
		label: 'Keyboard screen'
	});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Keyboard-${screen}`, extent: { width: 640, height: 360 } }
		}
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Screen keyboard');
	const main = page.locator('[data-panel-id="screen-warp"]');
	const view = main.getByRole('application', { name: 'Screen view', exact: true });
	const corner = main.getByRole('button', { name: 'Warp corner 1', exact: true });
	const inspect = async () => (await screens({ kind: 'InspectSurface', reference })).snapshot;
	async function waitFor(test) {
		const deadline = Date.now() + 6000;
		for (;;) {
			const snapshot = await inspect();
			if (test(snapshot, history)) return snapshot;
			assert(Date.now() < deadline, JSON.stringify({ snapshot, history }));
			await delay(20);
		}
	}
	const close = (a, b) => Math.abs(a - b) < 1e-6;
	try {
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		await page
			.locator('section[aria-label="Screen Manager"]')
			.getByRole('button', { name: 'Keyboard screen', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping$/ })
			.click();
		if (nativePage)
			await main
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
		const rect = await main.locator('.active-zone').boundingBox();
		assert(rect);
		await page.mouse.click(rect.x + rect.width / 2, rect.y + rect.height / 2);
		await corner.click();
		await main.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await delay(250);
		await view.focus();
		const before = await inspect();
		const undo = history.undo_len;
		const original = before.definition.quad.corners[0];
		for (let i = 0; i < 100; i++) await page.keyboard.down('ArrowRight');
		await page.keyboard.down('Shift');
		await page.keyboard.down('ArrowDown');
		await page.keyboard.up('Shift');
		await waitFor(
			(s, h) =>
				close(s.definition.quad.corners[0].x, original.x + 100 / 640) &&
				close(s.definition.quad.corners[0].y, original.y + 10 / 360) &&
				!!h.active_edit_session
		);
		await page.keyboard.up('ArrowRight');
		await inspect();
		assert(history.active_edit_session, 'second held arrow retains the group');
		await page.keyboard.up('ArrowDown');
		const after = await waitFor((_s, h) => !h.active_edit_session);
		assert.equal(history.undo_len, undo + 1);
		assert.deepEqual(
			after.definition.quad.corners.slice(1),
			before.definition.quad.corners.slice(1)
		);
		assert.deepEqual(after.definition.crop, before.definition.crop);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await waitFor(
			(s, h) => !h.active_edit_session && close(s.definition.quad.corners[0].x, original.x)
		);
		assert.deepEqual(await inspect(), before, 'Undo restores exact geometry');
		// History replacement rebuilds the projection; start the next interaction only once ready.
		await page.waitForFunction(
			() =>
				document.querySelector('[data-panel-id="screen-warp"] button[aria-label="Refresh preview"]')
					?.disabled === false
		);
		await main.getByRole('button', { name: 'Home view', exact: true }).click();
		const restored = await main.locator('.active-zone').boundingBox();
		assert(restored);
		await page.mouse.click(restored.x + restored.width / 2, restored.y + restored.height / 2);
		await corner.click();
		await view.focus();
		await page.keyboard.down('ArrowLeft');
		await waitFor((s) => close(s.definition.quad.corners[0].x, original.x - 1 / 640));
		await main.getByRole('button', { name: 'Zoom out', exact: true }).focus();
		await waitFor((_s, h) => !h.active_edit_session);
		await page.keyboard.up('ArrowLeft');
		const beforeCtrl = await inspect();
		await page.keyboard.press('ArrowRight');
		assert.deepEqual(await inspect(), beforeCtrl, 'navigation toolbar keeps its own keys');
		await view.focus();
		await page.keyboard.press('Control+ArrowRight');
		await delay(80);
		assert.deepEqual(await inspect(), beforeCtrl);
		// New while held fences the old collector; repeated input cannot target a new document.
		await page.keyboard.down('ArrowUp');
		await waitFor(
			(s, h) =>
				!!h.active_edit_session &&
				close(s.definition.quad.corners[0].y, beforeCtrl.definition.quad.corners[0].y - 1 / 360)
		);
		await post('/api/ui/project-new', {});
		await view.waitFor({ state: 'detached' });
		await page.keyboard.down('ArrowUp');
		await page.keyboard.up('ArrowUp');
		assert.equal((await screens({ kind: 'InspectManager' })).snapshot.screens.length, 0);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				screenKeyboard:
					'100 repeats + Shift 10 pixels at zoom, chord release, exact one-step Undo, blur, modifier isolation and New while held',
				native: !!nativePage,
				pageErrors: []
			})
		);
	} finally {
		await browser?.close();
	}
}
