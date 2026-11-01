// Actual bundled Screen controls; backend requests below only observe authored state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const near = (actual, expected) =>
	Math.abs(actual.x - expected.x) < 0.005 && Math.abs(actual.y - expected.y) < 0.005;

export async function verifyScreenUi({ base, post, media, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	try {
		async function screens() {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.screens',
				operation: 'edit',
				payload: { kind: 'InspectManager' }
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value.snapshot;
		}
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Screen Manager$/ });
		await tab.click();
		const manager = page.locator('section[aria-label="Screen Manager"]');
		const editor = page.locator('section[aria-label="Screen Warping"]');
		const properties = page.locator('.properties');
		const warpTab = page.getByRole('tab').filter({ hasText: /^Screen Warping$/ });
		await manager.getByRole('button', { name: 'Add child item', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const id = snapshot.screens[0].id;
		const first = snapshot.screens[0].surfaces[0].reference;
		const screenName = properties.getByRole('textbox', { name: 'Screen name', exact: true });
		await screenName.fill('UI Screen');
		await screenName.press('Enter');
		await wait(screens, (value) => value.screens[0].label === 'UI Screen');
		await manager
			.getByRole('button', { name: snapshot.screens[0].surfaces[0].label, exact: true })
			.click();
		const surfaceName = properties.getByRole('textbox', { name: 'Surface name', exact: true });
		await surfaceName.fill('UI Surface');
		await surfaceName.press('Enter');
		await wait(screens, (value) => value.screens[0].surfaces[0].label === 'UI Surface');
		await manager.getByRole('button', { name: 'Add Surface', exact: true }).click();
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		const peer = snapshot.screens[0].surfaces[1];
		await manager.getByRole('button', { name: 'UI Surface', exact: true }).click();
		await wait(
			() => surfaceName.inputValue(),
			(value) => value === 'UI Surface'
		);
		assert.deepEqual(await screens(), snapshot, 'selection cannot author state');
		stage = 'source selection';
		const sourceMedia = media.media.find((entry) => entry.media_kind === 'SolidColor');
		assert(sourceMedia);
		const source = sourceMedia.channels[0];
		await properties.getByLabel('Find source', { exact: true }).fill(sourceMedia.label);
		await properties
			.getByRole('combobox', { name: /^Source channel/ })
			.selectOption({ label: `${sourceMedia.label} / ${source.label}` });
		assert.deepEqual(await screens(), snapshot, 'source picker edits a draft until Apply');
		await properties.getByRole('button', { name: 'Apply surface settings', exact: true }).click();
		await wait(
			screens,
			(value) =>
				value.screens[0].surfaces[0].definition.source?.channel === source.reference.channel
		);
		const before = await screens();
		assert.deepEqual(before.screens[0].surfaces[0].definition.source, source.reference);
		assert.deepEqual(before.screens[0].surfaces[1], peer);
		await warpTab.click();
		const outer = await editor.boundingBox(),
			canvas = await editor.locator('.editor-canvas').boundingBox();
		assert(
			canvas.width >= outer.width * 0.95 && canvas.height >= outer.height * 0.8,
			'Warping canvas fills its own panel'
		);
		assert.equal(
			await editor.locator('input,select,fieldset').count(),
			0,
			'Parameters belong in Inspector'
		);
		assert.equal(
			await manager.locator('input:not([type=search]),select,fieldset').count(),
			0,
			'Manager remains a filtered outliner'
		);
		const svg = editor.getByRole('group', { name: 'Surface mapping controls', exact: true });
		const ready = () =>
			wait(
				() => editor.getByRole('button', { name: 'Refresh preview', exact: true }).isEnabled(),
				Boolean
			);
		const corner = async (index = 0) =>
			(await screens()).screens[0].surfaces[0].definition.quad.corners[index];
		async function begin(index = 0) {
			await ready();
			await svg.scrollIntoViewIfNeeded();
			const rect = await svg.boundingBox();
			assert(
				rect && rect.width > 20 && rect.height > 20,
				'naturally rendered mapping controls need geometry'
			);
			const current = await corner(index);
			await page.mouse.move(
				rect.x + current.x * rect.width + (current.x >= 1 ? -0.5 : 0.5),
				rect.y + current.y * rect.height + (current.y >= 1 ? -0.5 : 0.5)
			);
			await page.mouse.down();
			const started = await svg.boundingBox();
			assert(
				started &&
					['x', 'y', 'width', 'height'].every((key) => Math.abs(started[key] - rect[key]) < 0.01),
				'Clearing an error on pointer-down cannot shift mapping geometry'
			);
			return rect;
		}
		async function move(rect, point, steps = 1) {
			await page.mouse.move(rect.x + point.x * rect.width, rect.y + point.y * rect.height, {
				steps
			});
		}
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		stage = 'grouped corner drag';
		let rect = await begin();
		await move(rect, { x: 0.18, y: 0.16 }, 16);
		await page.mouse.up();
		await ready();
		await wait(corner, (point) => near(point, { x: 0.18, y: 0.16 }));
		const dragged = await screens();
		assert.deepEqual(dragged.screens[0].surfaces[1], peer);
		assert.deepEqual(dragged.screens[0].surfaces[0].definition.source, source.reference);
		assert.deepEqual(
			dragged.screens[0].surfaces[0].definition.quad.corners.slice(1),
			before.screens[0].surfaces[0].definition.quad.corners.slice(1)
		);
		await undo();
		await wait(corner, (point) => near(point, { x: 0, y: 0 }));
		assert.deepEqual(await screens(), before, 'one Undo restores every sample in the drag');
		await redo();
		await wait(corner, (point) => near(point, { x: 0.18, y: 0.16 }));
		assert.deepEqual(await screens(), dragged);
		stage = 'invalid mapping';
		rect = await begin();
		await move(rect, { x: 1.1, y: 1.1 });
		await page.mouse.up();
		await editor.getByRole('alert').waitFor();
		const errorBounds = await svg.boundingBox();
		assert(
			['x', 'y', 'width', 'height'].every((key) => Math.abs(errorBounds[key] - rect[key]) < 0.01),
			'Showing a rejection cannot shift mapping geometry'
		);
		await ready();
		assert.deepEqual(
			await screens(),
			dragged,
			'an invalid corner cannot replace the last valid mapping'
		);
		await undo();
		await wait(corner, (point) => near(point, { x: 0, y: 0 }));
		assert.deepEqual(await screens(), before, 'rejection cannot consume an authored history step');
		await redo();
		await wait(corner, (point) => near(point, { x: 0.18, y: 0.16 }));
		stage = 'Escape ends accepted drag';
		rect = await begin();
		await move(rect, { x: 0.26, y: 0.2 }, 12);
		await wait(corner, (point) => near(point, { x: 0.26, y: 0.2 }));
		await page.keyboard.press('Escape');
		await move(rect, { x: 0.45, y: 0.4 });
		await page.mouse.up();
		await ready();
		assert(near(await corner(), { x: 0.26, y: 0.2 }), 'motion after Escape cannot author');
		const stopped = await screens();
		await undo();
		await wait(corner, (point) => near(point, { x: 0.18, y: 0.16 }));
		assert.deepEqual(await screens(), dragged);
		await redo();
		await wait(corner, (point) => near(point, { x: 0.26, y: 0.2 }));
		stage = 'remaining corner handles';
		for (const [index, point] of [
			[1, { x: 0.86, y: 0.1 }],
			[2, { x: 0.86, y: 0.87 }],
			[3, { x: 0.1, y: 0.88 }]
		]) {
			const previous = await screens();
			rect = await begin(index);
			await move(rect, point, 12);
			await page.mouse.up();
			await ready();
			await wait(
				() => corner(index),
				(current) => near(current, point)
			);
			const next = await screens();
			for (let other = 0; other < 4; other++) {
				if (other !== index)
					assert.deepEqual(
						next.screens[0].surfaces[0].definition.quad.corners[other],
						previous.screens[0].surfaces[0].definition.quad.corners[other]
					);
			}
			assert.deepEqual(next.screens[0].surfaces[1], peer);
			assert.deepEqual(next.screens[0].surfaces[0].definition.source, source.reference);
			await undo();
			await wait(
				() => corner(index),
				(current) => near(current, previous.screens[0].surfaces[0].definition.quad.corners[index])
			);
			assert.deepEqual(
				await screens(),
				previous,
				`corner ${index}: one Undo restores the whole gesture`
			);
			await redo();
			await wait(
				() => corner(index),
				(current) => near(current, point)
			);
			assert.deepEqual(await screens(), next);
		}
		const completed = await screens();
		stage = 'panel lifetime';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		await tab.click();
		await warpTab.click();
		await ready();
		assert.deepEqual(await screens(), completed);
		assert.deepEqual(
			await inspectMedia(),
			media,
			'Screen editing cannot mutate its shared source media'
		);
		assert.deepEqual(errors, []);
		assert(rejected.length > 0, 'Invalid geometry reaches backend rejection');
		assert(
			rejected.every(
				(item) =>
					item.request?.stage === 'invalid mapping' &&
					item.request.intent?.kind === 'invokeAppOperation' &&
					item.request.intent.namespace === 'mapgyver.screens' &&
					item.request.intent.payload?.kind === 'EditSurfaceGeometry'
			),
			'Only the explicit invalid geometry case may reject'
		);
		await svg.scrollIntoViewIfNeeded();
		await page.screenshot({ path: 'target/m05-screen-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual bundled Screen selection/source/drag/rejection/Escape/history input; physical preview/capture/DPI remain unverified',
				browser: browser.version(),
				screen: id,
				surface: first,
				peer: peer.reference,
				source: source.reference,
				final_quad: completed.screens[0].surfaces[0].definition.quad,
				page_errors: errors.length,
				expected_geometry_rejections: rejected.length,
				screenshot: 'target/m05-screen-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				page_errors: errors,
				rejected_requests: rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-3500)
			})
		);
		await page.screenshot({ path: 'target/m05-screen-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
