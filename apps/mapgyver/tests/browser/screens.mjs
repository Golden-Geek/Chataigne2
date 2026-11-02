// Actual bundled Screen controls; backend requests below only observe authored state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls, surfaceParameterLabels } from './screen-parameters.mjs';

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
		const controls = screenParameterControls(page);
		const warpTab = page.getByRole('tab').filter({ hasText: /^Screen Warping$/ });
		await manager.getByRole('button', { name: 'Add child item', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const id = snapshot.screens[0].id;
		const first = snapshot.screens[0].surfaces[0].reference;
		await controls.fields(['Target', 'Enabled'], wait);
		await warpTab.click();
		await editor.locator('.native-preview').waitFor();
		const backgrounds = await page.evaluate(() => {
			const app = document.querySelector('.app');
			const previous = app.style.getPropertyValue('--gc-workbench-background');
			// Exercise the native underlay's transparent root without claiming native pixels.
			app.style.setProperty('--gc-workbench-background', 'transparent');
			const theme = document.createElement('div');
			theme.style.backgroundColor = 'var(--gc-color-panel)';
			app.append(theme);
			try {
				const background = (element) => getComputedStyle(element).backgroundColor;
				const manager = document.querySelector('section[aria-label="Screen Manager"]');
				const editor = document.querySelector('section[aria-label="Screen Warping"]');
				return {
					theme: background(theme),
					root: background(app.querySelector('.gc-main')),
					manager: background(manager.closest('.dv-groupview')),
					inspector: background(app.querySelector('.inspector').closest('.dv-groupview')),
					preview: background(editor.closest('.dv-groupview')),
					toolbar: background(editor.querySelector('.editor-toolbar'))
				};
			} finally {
				theme.remove();
				if (previous) app.style.setProperty('--gc-workbench-background', previous);
				else app.style.removeProperty('--gc-workbench-background');
			}
		});
		assert.equal(backgrounds.root, 'rgba(0, 0, 0, 0)');
		assert.equal(
			backgrounds.preview,
			'rgba(0, 0, 0, 0)',
			'Native aperture ancestors stay transparent'
		);
		for (const area of ['manager', 'inspector', 'toolbar'])
			assert.equal(backgrounds[area], backgrounds.theme, `${area} retains the opaque panel theme`);
		await controls.rename(snapshot.screens[0].label, 'UI Screen');
		await wait(screens, (value) => value.screens[0].label === 'UI Screen');
		stage = 'generic Screen target Parameters';
		await controls.choice('Target').selectOption('Fullscreen');
		await controls.fields(['Target', 'Display identity', 'Enabled'], wait);
		await controls.setText('Display identity', 'Parameter UI unavailable display');
		await wait(
			screens,
			(value) => value.screens[0].definition.target?.display === 'Parameter UI unavailable display'
		);
		await controls.choice('Target').selectOption('Rectangle');
		await controls.fields(['Target', 'Left', 'Top', 'Width', 'Height', 'Enabled'], wait);
		const widthParameter = await controls.parameter('Width').getAttribute('data-node-id');
		for (const [label, value, read] of [
			['Left', -100, (target) => target.bounds.left],
			['Top', 50, (target) => target.bounds.top],
			['Width', 800, (target) => target.bounds.extent.width],
			['Height', 600, (target) => target.bounds.extent.height]
		]) {
			await controls.setNumber(label, value);
			await wait(screens, (snapshot) => read(snapshot.screens[0].definition.target) === value);
		}
		await controls.choice('Target').selectOption('Spout');
		await controls.fields(['Target', 'Width', 'Height', 'Sender name', 'Enabled'], wait);
		await controls.setText('Sender name', 'Parameter UI output');
		await wait(
			screens,
			(value) => value.screens[0].definition.target?.sender === 'Parameter UI output'
		);
		assert.equal(await controls.parameter('Width').getAttribute('data-node-id'), widthParameter);
		assert.deepEqual((await screens()).screens[0].definition.target.extent, {
			width: 800,
			height: 600
		});
		await controls.parameter('Enabled').locator('input.editor-checkbox').check();
		await wait(screens, (value) => value.screens[0].definition.enabled);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(screens, (value) => !value.screens[0].definition.enabled);
		await manager
			.getByRole('button', { name: snapshot.screens[0].surfaces[0].label, exact: true })
			.click();
		await controls.fields(surfaceParameterLabels, wait);
		await controls.rename(snapshot.screens[0].surfaces[0].label, 'UI Surface');
		await wait(screens, (value) => value.screens[0].surfaces[0].label === 'UI Surface');
		await manager.getByRole('button', { name: 'Add Surface', exact: true }).click();
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		const peer = snapshot.screens[0].surfaces[1];
		await manager.getByRole('button', { name: 'UI Surface', exact: true }).click();
		await wait(
			() => page.locator('.inspector-header .title-text').first().innerText(),
			(value) => value === 'UI Surface'
		);
		assert.deepEqual(await screens(), snapshot, 'selection cannot author state');
		stage = 'source selection';
		const sourceMedia = media.media.find((entry) => entry.media_kind === 'SolidColor');
		assert(sourceMedia);
		const source = sourceMedia.channels[0];
		await controls.reference('Source media', sourceMedia.label, sourceMedia.label);
		await controls.reference('Source channel', sourceMedia.label, source.label);
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
		await wait(corner, (point) => near(point, { x: 1.1, y: 1.1 }));
		await svg.locator('polygon.invalid').waitFor();
		assert.equal(await editor.getByRole('alert').count(), 0);
		await page.mouse.up();
		const errorBounds = await svg.boundingBox();
		assert(
			['x', 'y', 'width', 'height'].every((key) => Math.abs(errorBounds[key] - rect[key]) < 0.01),
			'Invalid-shape feedback cannot shift mapping geometry'
		);
		await ready();
		const invalid = await screens();
		assert.equal(invalid.screens[0].surfaces[0].mapping_valid, false);
		await undo();
		await wait(corner, (point) => near(point, { x: 0.18, y: 0.16 }));
		assert.deepEqual(
			await screens(),
			dragged,
			'invalid shapes are ordinary undoable authored edits'
		);
		await redo();
		await wait(corner, (point) => near(point, { x: 1.1, y: 1.1 }));
		assert.deepEqual(await screens(), invalid);
		await undo();
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
		assert.deepEqual(rejected, [], 'Finite intermediate warp shapes never reject');
		await svg.scrollIntoViewIfNeeded();
		await page.screenshot({ path: 'target/m05-screen-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual bundled Screen selection/source/free warp/Escape/history input; physical preview/capture/DPI remain unverified',
				browser: browser.version(),
				screen: id,
				surface: first,
				peer: peer.reference,
				source: source.reference,
				final_quad: completed.screens[0].surfaces[0].definition.quad,
				page_errors: errors.length,
				geometry_rejections: rejected.length,
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
