import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyScreenTranslation({ base, post, page: nativePage = null }) {
	let history,
		stage = 'setup';
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
		label: 'Translation screen'
	});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: {
				kind: 'Spout',
				sender: `Translation-${screen}`,
				extent: { width: 640, height: 360 }
			}
		}
	});
	const { snapshot: initial } = await screens({ kind: 'InspectSurface', reference });
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: {
			...initial.definition,
			crop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 },
			opacity: 0.65,
			calibration: 'ColorBars',
			quad: {
				corners: [
					{ x: 0.2, y: 0.2 },
					{ x: 0.75, y: 0.15 },
					{ x: 0.8, y: 0.75 },
					{ x: 0.15, y: 0.8 }
				]
			}
		}
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	const main = page.locator('[data-panel-id="screen-warp"]');
	const view = main.getByRole('application', { name: 'Screen view', exact: true });
	const canvas = main.getByRole('group', { name: 'Surface mapping controls', exact: true });
	const corner = main.getByRole('button', { name: 'Warp corner 1', exact: true });
	const inspect = async () => (await screens({ kind: 'InspectSurface', reference })).snapshot;
	async function wait(test) {
		const deadline = Date.now() + 6000;
		for (;;) {
			const snapshot = await inspect();
			if (test(snapshot, history)) return snapshot;
			assert(Date.now() < deadline, JSON.stringify({ stage, snapshot, history }));
			await delay(20);
		}
	}
	async function ready() {
		await page.waitForFunction(
			() =>
				document.querySelector('[data-panel-id="screen-warp"] button[aria-label="Refresh preview"]')
					?.disabled === false
		);
	}
	async function position(x, y) {
		const rect = await canvas.boundingBox();
		assert(rect);
		return { x: rect.x + x * rect.width, y: rect.y + y * rect.height };
	}
	function translated(snapshot, before, dx, dy) {
		return snapshot.definition.quad.corners.every(
			(p, i) =>
				Math.abs(p.x - before.definition.quad.corners[i].x - dx) < 0.004 &&
				Math.abs(p.y - before.definition.quad.corners[i].y - dy) < 0.004
		);
	}
	function unchangedFields(after, before) {
		assert.deepEqual({ ...after.definition, quad: null }, { ...before.definition, quad: null });
		const delta = {
			x: after.definition.quad.corners[0].x - before.definition.quad.corners[0].x,
			y: after.definition.quad.corners[0].y - before.definition.quad.corners[0].y
		};
		for (const [i, p] of after.definition.quad.corners.entries())
			assert(
				Math.abs(p.x - before.definition.quad.corners[i].x - delta.x) < 1e-6 &&
					Math.abs(p.y - before.definition.quad.corners[i].y - delta.y) < 1e-6
			);
	}
	async function startAt(point, alt = true) {
		await page.mouse.move(point.x, point.y);
		if (alt) await page.keyboard.down('Alt');
		await page.mouse.down();
	}
	async function end(alt = true) {
		await page.mouse.up();
		if (alt) await page.keyboard.up('Alt');
		await wait((_s, h) => !h.active_edit_session);
		await ready();
	}
	async function undo(before) {
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			(s, h) =>
				!h.active_edit_session && JSON.stringify(s.definition) === JSON.stringify(before.definition)
		);
		await ready();
		// Backend history completion precedes the WebSocket projection and its visible geometry.
		await page.waitForFunction(
			({ id, corners }) => {
				const points = document
					.querySelector(`polygon[data-surface-drop-target="${id}"]`)
					?.getAttribute('points')
					?.split(' ')
					.map((pair) => pair.split(',').map(Number));
				return (
					points?.length === 4 &&
					points.every(
						([x, y], i) => Math.abs(x - corners[i].x) < 1e-6 && Math.abs(y - corners[i].y) < 1e-6
					)
				);
			},
			{ id: reference.surface, corners: before.definition.quad.corners }
		);
	}
	try {
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		await page
			.locator('section[aria-label="Screen Manager"]')
			.getByRole('button', { name: 'Translation screen', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping$/ })
			.click();
		if (nativePage)
			await main
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
		await ready();
		const body = await position(0.45, 0.45);
		await page.mouse.click(body.x, body.y);
		const before = await inspect();
		const beforeUndo = history.undo_len;
		stage = 'ordinary body drag and Alt jitter';
		await startAt(body, false);
		await page.mouse.move(body.x + 15, body.y + 10, { steps: 4 });
		await end(false);
		assert.deepEqual(await inspect(), before);
		await startAt(body);
		await page.mouse.move(body.x + 1, body.y + 1);
		await end();
		assert.deepEqual(await inspect(), before);
		assert.equal(history.undo_len, beforeUndo);
		stage = 'Alt body translation';
		const background = await position(0.95, 0.05);
		await page.mouse.click(background.x, background.y);
		// Native clicks resolve against the accepted image before dispatching selection.
		await page.waitForFunction(
			() => !document.querySelector('[data-panel-id="screen-warp"] .viewport-handle:not(.ghost)')
		);
		await startAt(body);
		await page.keyboard.up('Alt');
		const destination = await position(0.53, 0.51);
		await page.mouse.move(destination.x, destination.y, { steps: 20 });
		await wait((s, h) => translated(s, before, 0.08, 0.06) && h.active_edit_session);
		await end(false);
		const moved = await inspect();
		unchangedFields(moved, before);
		assert.equal(history.undo_len, beforeUndo + 1);
		await undo(before);
		stage = 'Alt corner translation at zoom';
		await main.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await page.waitForFunction(
			() =>
				Number(
					document
						.querySelector('[data-panel-id="screen-warp"] [data-zoom]')
						?.getAttribute('data-zoom')
				) > 1.19
		);
		const center = await position(0.45, 0.45);
		await page.mouse.click(center.x, center.y);
		await corner.click();
		const rect = await canvas.boundingBox(),
			handle = await corner.boundingBox();
		assert(rect && handle);
		await startAt({ x: handle.x + handle.width / 2, y: handle.y + handle.height / 2 });
		await page.mouse.move(
			handle.x + handle.width / 2 + rect.width * 0.06,
			handle.y + handle.height / 2 - rect.height * 0.04,
			{ steps: 20 }
		);
		await wait((s, h) => translated(s, before, 0.06, -0.04) && h.active_edit_session);
		await end();
		const cornerMoved = await inspect();
		unchangedFields(cornerMoved, before);
		assert.equal(history.undo_len, beforeUndo + 1);
		const overlay = await corner.boundingBox(),
			transformed = await canvas.boundingBox();
		assert(overlay && transformed);
		assert(
			Math.abs(
				overlay.x +
					overlay.width / 2 -
					transformed.x -
					cornerMoved.definition.quad.corners[0].x * transformed.width
			) < 1
		);
		assert(
			Math.abs(
				overlay.y +
					overlay.height / 2 -
					transformed.y -
					cornerMoved.definition.quad.corners[0].y * transformed.height
			) < 1
		);
		await undo(before);
		stage = 'Space Alt pan isolation';
		await view.focus();
		await page.keyboard.down('Space');
		const pan = await position(0.45, 0.45);
		await startAt(pan);
		await page.mouse.move(pan.x + 20, pan.y + 10, { steps: 6 });
		await end();
		await page.keyboard.up('Space');
		assert.deepEqual(await inspect(), before);
		assert.equal(history.undo_len, beforeUndo);
		await main.getByRole('button', { name: 'Home view', exact: true }).click();
		stage = 'Escape stops a held translation';
		const cancelStart = await position(0.45, 0.45);
		await startAt(cancelStart);
		const cancelEnd = await position(0.49, 0.47);
		await page.mouse.move(cancelEnd.x, cancelEnd.y, { steps: 8 });
		await wait((s, h) => translated(s, before, 0.04, 0.02) && h.active_edit_session);
		await page.keyboard.press('Escape');
		const cancelled = await wait((_s, h) => !h.active_edit_session);
		await page.mouse.move(cancelEnd.x + 50, cancelEnd.y + 30, { steps: 8 });
		await end();
		assert.deepEqual(await inspect(), cancelled);
		assert.equal(history.undo_len, beforeUndo + 1);
		await undo(before);
		stage = 'New while translation held';
		const newStart = await position(0.45, 0.45);
		await startAt(newStart);
		const newEnd = await position(0.49, 0.47);
		await page.mouse.move(newEnd.x, newEnd.y, { steps: 8 });
		await wait((s, h) => translated(s, before, 0.04, 0.02) && h.active_edit_session);
		await post('/api/ui/project-new', {});
		await view.waitFor({ state: 'detached' });
		await page.mouse.move(newEnd.x + 50, newEnd.y + 30);
		await page.mouse.up();
		await page.keyboard.up('Alt');
		assert.equal((await screens({ kind: 'InspectManager' })).snapshot.screens.length, 0);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				screenTranslation:
					'Alt body/corner at zoom, all-corner displacement and shape preservation, one Undo, Alt release mid-drag, ordinary body/jitter/pan isolation, Escape and New while held',
				native: !!nativePage,
				pageErrors: []
			})
		);
	} catch (error) {
		console.error(JSON.stringify({ stage, errors, rejected }));
		throw error;
	} finally {
		await browser?.close();
	}
}
