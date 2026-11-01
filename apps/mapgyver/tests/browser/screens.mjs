// Actual bundled Screen controls; backend requests below only observe authored state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';

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
		const panel = page.getByRole('region', { name: 'Screen Manager', exact: true });
		await panel.getByRole('button', { name: 'Add screen', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const id = snapshot.screens[0].id;
		const first = snapshot.screens[0].surfaces[0].reference;
		await panel
			.getByRole('navigation', { name: 'Screens', exact: true })
			.getByRole('textbox')
			.fill('UI Screen');
		await panel
			.getByRole('navigation', { name: 'Screens', exact: true })
			.getByRole('button', { name: 'Rename', exact: true })
			.click();
		await wait(screens, (value) => value.screens[0].label === 'UI Screen');
		await panel.getByLabel('Surface name', { exact: true }).fill('UI Surface');
		await panel.getByRole('button', { name: 'Rename surface', exact: true }).click();
		await wait(screens, (value) => value.screens[0].surfaces[0].label === 'UI Surface');
		await panel.getByRole('button', { name: 'Add surface', exact: true }).click();
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		const peer = snapshot.screens[0].surfaces[1];
		const ordered = panel.getByRole('region', { name: 'Ordered surfaces', exact: true });
		await ordered.getByRole('button', { name: 'UI Surface', exact: true }).click();
		assert.deepEqual(await screens(), snapshot, 'selection cannot author state');
		stage = 'source selection';
		const sourceMedia = media.media.find((entry) => entry.media_kind === 'SolidColor');
		assert(sourceMedia);
		const source = sourceMedia.channels[0];
		await panel.getByLabel('Find source', { exact: true }).fill(sourceMedia.label);
		await panel
			.getByLabel('Source channel')
			.selectOption({ label: `${sourceMedia.label} / ${source.label}` });
		assert.deepEqual(await screens(), snapshot, 'source picker edits a draft until Apply');
		await panel.getByRole('button', { name: 'Apply surface settings', exact: true }).click();
		await wait(
			screens,
			(value) =>
				value.screens[0].surfaces[0].definition.source?.channel === source.reference.channel
		);
		const before = await screens();
		assert.deepEqual(before.screens[0].surfaces[0].definition.source, source.reference);
		assert.deepEqual(before.screens[0].surfaces[1], peer);
		const svg = panel.getByRole('group', { name: 'Surface mapping controls', exact: true });
		const ready = () =>
			wait(
				() => panel.getByRole('button', { name: 'Add surface', exact: true }).isEnabled(),
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
				rect.x + current.x * rect.width + 0.5,
				rect.y + current.y * rect.height + 0.5
			);
			await page.mouse.down();
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
		await panel.locator('p.error[role="status"]').waitFor();
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
		await tab.click();
		await ready();
		assert.deepEqual(await screens(), completed);
		assert.deepEqual(
			await inspectMedia(),
			media,
			'Screen editing cannot mutate its shared source media'
		);
		assert.deepEqual(errors, []);
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
				screenshot: 'target/m05-screen-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
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
