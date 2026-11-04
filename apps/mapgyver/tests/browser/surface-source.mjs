// Real Surface Source input and history; backend requests below only observe state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls, surfaceParameterLabels } from './screen-parameters.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const near = (actual, expected) =>
	Object.entries(expected).every(([key, value]) => Math.abs(actual[key] - value) < 0.005);
const corners = (crop) => [
	{ x: crop.x, y: crop.y },
	{ x: crop.x + crop.width, y: crop.y },
	{ x: crop.x + crop.width, y: crop.y + crop.height },
	{ x: crop.x, y: crop.y + crop.height }
];

export async function verifySurfaceSourceUi({
	base,
	post,
	media,
	inspectMedia,
	page: nativePage = null
}) {
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1440, height: 1000 } }));
	if (nativePage) await page.setViewportSize({ width: 1440, height: 1000 });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	let history;
	try {
		async function screens() {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.screens',
				operation: 'edit',
				payload: { kind: 'InspectManager' }
			});
			assert(ack.success, JSON.stringify(ack));
			history = ack.history;
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
		if (!nativePage) await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Surface Source$/ });
		const screenTab = page.getByRole('tab').filter({ hasText: /^Screen Manager$/ });
		const panel = page.locator('section[aria-label="Surface Source"]');
		const tree = page.locator('section[aria-label="Screen Manager"]');
		const inspector = page.locator('.inspector');
		const controls = screenParameterControls(page);
		await screenTab.click();
		await tree.getByRole('button', { name: 'Add child item', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const first = snapshot.screens[0].surfaces[0];
		await tree
			.locator('[data-node-id]')
			.filter({ has: page.getByRole('button', { name: snapshot.screens[0].label, exact: true }) })
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		const peer = snapshot.screens[0].surfaces[1];
		await tree.getByRole('button', { name: first.label, exact: true }).click();
		await wait(
			() => page.locator('.inspector-header .title-text').first().innerText(),
			(value) => value === first.label
		);
		await tab.click();
		const outer = await panel.boundingBox(),
			canvas = await panel.locator('.editor-canvas').boundingBox();
		assert(
			canvas.width >= outer.width * 0.95 && canvas.height >= outer.height * 0.8,
			'Source canvas fills its own panel'
		);
		assert.equal(
			await panel.locator('input:not([type=range]),select,fieldset').count(),
			0,
			'Source parameters belong in Inspector'
		);
		assert.equal(
			await tree.locator('input:not([type=search]),select,fieldset').count(),
			0,
			'Screen Manager stays a filtered outliner'
		);
		const sourceMedia = media.media.find((value) => value.media_kind === 'SolidColor');
		assert(sourceMedia);
		const source = sourceMedia.channels[0];
		await controls.fields(surfaceParameterLabels, wait);
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		const definition = async () => (await screens()).screens[0].surfaces[0].definition;
		const crop = async () => (await definition()).crop;
		const ready = () =>
			wait(
				() => panel.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		const parameterHistory = new Map();
		async function remember() {
			const value = await wait(screens, () => !history.active_edit_session);
			parameterHistory.set(history.undo_len, value);
			return value;
		}
		await remember();
		const initialDepth = history.undo_len;
		stage = 'generic source references and ordinary Surface Parameters';
		await controls.reference('Source media', sourceMedia.label, sourceMedia.label);
		await wait(
			() => controls.parameter('Source media').locator('.reference-editor.link-linked').count(),
			(count) => count === 1
		);
		await remember();
		await controls.reference('Source channel', sourceMedia.label, source.label);
		await wait(definition, (value) => value.source?.channel === source.reference.channel);
		if (nativePage)
			await panel
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
		await remember();
		await controls.parameter('Enabled').locator('input.editor-checkbox').uncheck();
		await wait(definition, (value) => !value.enabled);
		await remember();
		await controls.setNumber('Opacity', 0.65);
		await wait(definition, (value) => Math.abs(value.opacity - 0.65) < 0.000001);
		await remember();
		// Each ordinary editor commits its real Parameter immediately. Shrink the
		// crop before moving its origin so every intermediate mapping remains valid.
		for (const [label, component, value, read] of [
			['Crop size', 0, 0.6, (value) => value.crop.width],
			['Crop size', 1, 0.5, (value) => value.crop.height],
			['Crop origin', 0, 0.1, (value) => value.crop.x],
			['Crop origin', 1, 0.1, (value) => value.crop.y],
			['Top left', 0, 0.05, (value) => value.quad.corners[0].x],
			['Top left', 1, 0.04, (value) => value.quad.corners[0].y],
			['Top right', 0, 0.95, (value) => value.quad.corners[1].x],
			['Top right', 1, 0.02, (value) => value.quad.corners[1].y],
			['Bottom right', 0, 0.9, (value) => value.quad.corners[2].x],
			['Bottom right', 1, 0.96, (value) => value.quad.corners[2].y],
			['Bottom left', 0, 0.02, (value) => value.quad.corners[3].x],
			['Bottom left', 1, 0.9, (value) => value.quad.corners[3].y]
		]) {
			await controls.setNumber(label, value, component);
			await wait(definition, (current) => Math.abs(read(current) - value) < 0.000001);
			await remember();
		}
		await controls.choice('Calibration').selectOption('Grid');
		await wait(definition, (value) => value.calibration === 'Grid');
		const configuredSnapshot = await remember();
		const configured = configuredSnapshot.screens[0].surfaces[0].definition;
		const configuredDepth = history.undo_len;
		assert(configuredDepth - initialDepth >= 10, 'Ordinary edits use Parameter history entries');
		assert.deepEqual(configured.source, source.reference);
		assert.equal(configured.enabled, false);
		assert(Math.abs(configured.opacity - 0.65) < 0.000001);
		assert(near(configured.crop, { x: 0.1, y: 0.1, width: 0.6, height: 0.5 }));
		for (const [index, point] of [
			{ x: 0.05, y: 0.04 },
			{ x: 0.95, y: 0.02 },
			{ x: 0.9, y: 0.96 },
			{ x: 0.02, y: 0.9 }
		].entries())
			assert(
				near(configured.quad.corners[index], point),
				`corner ${index}: actual=${JSON.stringify(configured.quad.corners[index])} expected=${JSON.stringify(point)} complete=${JSON.stringify(configured)}`
			);
		stage = 'generic Surface Parameter history';
		for (let depth = configuredDepth; depth > initialDepth; depth--) {
			await undo();
			const state = await wait(
				screens,
				() => history.undo_len === depth - 1 && !history.active_edit_session
			);
			assert.deepEqual(state, parameterHistory.get(depth - 1));
		}
		assert.deepEqual(await screens(), snapshot);
		await wait(
			() =>
				controls
					.parameter('Source media')
					.locator(
						first.definition.source
							? '.reference-editor.link-linked'
							: '.reference-editor.link-empty'
					)
					.count(),
			(count) => count === 1
		);
		for (let depth = initialDepth; depth < configuredDepth; depth++) {
			await redo();
			const state = await wait(
				screens,
				() => history.undo_len === depth + 1 && !history.active_edit_session
			);
			assert.deepEqual(state, parameterHistory.get(depth + 1));
		}
		assert.deepEqual(await screens(), configuredSnapshot);
		for (const calibration of ['ColorBars', 'Off']) {
			await controls.choice('Calibration').selectOption(calibration);
			await wait(definition, (value) => value.calibration === calibration);
			assert.deepEqual(
				{ ...(await definition()), calibration: 'Grid' },
				configured,
				'calibration cannot replace source/crop/mapping'
			);
		}
		const svg = panel.getByRole('group', { name: 'Source crop controls', exact: true });
		async function begin(index) {
			const value = await crop();
			await ready();
			await wait(
				async () =>
					Object.fromEntries(
						await Promise.all(
							['x', 'y', 'width', 'height'].map(async (name) => [
								name,
								Number(await svg.locator('rect').getAttribute(name))
							])
						)
					),
				(displayed) => near(displayed, value)
			);
			await wait(
				() =>
					panel
						.getByRole('button', {
							name: `Crop corner ${index === null ? 1 : index + 1}`,
							exact: true
						})
						.isEnabled(),
				Boolean
			);
			await svg.scrollIntoViewIfNeeded();
			const box = await svg.boundingBox();
			assert(box && box.width > 20 && box.height > 20);
			if (index !== null)
				await wait(
					() =>
						panel
							.getByRole('button', { name: `Crop corner ${index + 1}`, exact: true })
							.isEnabled(),
					Boolean
				);
			const point =
				index === null
					? { x: value.x + value.width / 2, y: value.y + value.height / 2 }
					: corners(value)[index];
			const handle =
				index === null
					? null
					: await panel
							.getByRole('button', { name: `Crop corner ${index + 1}`, exact: true })
							.boundingBox();
			if (handle) {
				const target = await page.evaluate(
					({ x, y }) => document.elementFromPoint(x, y)?.outerHTML,
					{ x: handle.x + handle.width / 2, y: handle.y + handle.height / 2 }
				);
				assert(
					target?.includes(`aria-label="Crop corner ${index + 1}"`) &&
						!target.includes(' disabled'),
					`Crop handle must receive pointer input: ${target}`
				);
			}
			await page.mouse.move(
				handle ? handle.x + handle.width / 2 : box.x + point.x * box.width,
				handle ? handle.y + handle.height / 2 : box.y + point.y * box.height
			);
			await page.mouse.down();
			await wait(screens, () => history.active_edit_session);
			const started = await svg.boundingBox();
			assert(
				started &&
					['x', 'y', 'width', 'height'].every((key) => Math.abs(started[key] - box[key]) < 0.01),
				'Clearing an error cannot shift source coordinates'
			);
			return started;
		}
		async function move(box, point, steps = 1) {
			assert(box);
			await page.mouse.move(box.x + point.x * box.width, box.y + point.y * box.height, { steps });
		}
		async function roundtrip(before, after, expected) {
			assert.deepEqual(after.screens[0].surfaces[1], peer);
			assert.deepEqual(
				{
					...after.screens[0].surfaces[0].definition,
					crop: before.screens[0].surfaces[0].definition.crop
				},
				before.screens[0].surfaces[0].definition
			);
			await undo();
			await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
			assert.deepEqual(await screens(), before);
			await redo();
			await wait(crop, (value) => near(value, expected));
			assert.deepEqual(await screens(), after);
		}
		stage = 'all four crop handles';
		for (const [index, point, expected] of [
			[0, { x: 0.15, y: 0.15 }, { x: 0.15, y: 0.15, width: 0.55, height: 0.45 }],
			[1, { x: 0.65, y: 0.18 }, { x: 0.15, y: 0.18, width: 0.5, height: 0.42 }],
			[2, { x: 0.6, y: 0.55 }, { x: 0.15, y: 0.18, width: 0.45, height: 0.37 }],
			[3, { x: 0.2, y: 0.5 }, { x: 0.2, y: 0.18, width: 0.4, height: 0.32 }]
		]) {
			const before = await screens();
			const box = await begin(index);
			await move(box, point, 12);
			await page.mouse.up();
			await ready();
			await wait(crop, (value) => near(value, expected));
			await roundtrip(before, await screens(), expected);
		}
		stage = 'origin drag';
		let before = await screens();
		let box = await begin(null);
		await move(box, { x: 0.48, y: 0.4 }, 16);
		await page.mouse.up();
		await ready();
		await wait(crop, (value) => near(value, { x: 0.28, y: 0.24 }));
		let after = await screens();
		assert.equal(
			after.screens[0].surfaces[0].definition.crop.width,
			before.screens[0].surfaces[0].definition.crop.width
		);
		assert.equal(
			after.screens[0].surfaces[0].definition.crop.height,
			before.screens[0].surfaces[0].definition.crop.height
		);
		await roundtrip(before, after, after.screens[0].surfaces[0].definition.crop);
		stage = 'crop stops at source edges';
		before = await screens();
		box = await begin(0);
		await move(box, { x: -0.2, y: -0.2 });
		await page.mouse.up();
		await ready();
		const previousCrop = before.screens[0].surfaces[0].definition.crop;
		const bounded = {
			x: 0,
			y: 0,
			width: previousCrop.x + previousCrop.width,
			height: previousCrop.y + previousCrop.height
		};
		await wait(crop, (value) => near(value, bounded));
		const errorBounds = await svg.boundingBox();
		assert(
			errorBounds &&
				['x', 'y', 'width', 'height'].every((key) => Math.abs(errorBounds[key] - box[key]) < 0.01),
			'Clamping at the source edge cannot shift source coordinates'
		);
		assert.equal(await panel.getByRole('alert').count(), 0);
		await roundtrip(before, await screens(), bounded);
		await undo();
		await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
		assert.deepEqual(await screens(), before);
		for (const [index, point, expected] of [
			[
				2,
				{ x: 1.2, y: 1.2 },
				{ ...previousCrop, width: 1 - previousCrop.x, height: 1 - previousCrop.y }
			],
			[
				null,
				{ x: 1.2, y: 1.2 },
				{ ...previousCrop, x: 1 - previousCrop.width, y: 1 - previousCrop.height }
			],
			[null, { x: -0.2, y: -0.2 }, { ...previousCrop, x: 0, y: 0 }]
		]) {
			before = await screens();
			box = await begin(index);
			await move(box, point, 12);
			await page.mouse.up();
			await ready();
			await wait(crop, (value) => near(value, expected));
			assert.equal(await panel.getByRole('alert').count(), 0);
			await roundtrip(before, await screens(), expected);
			await undo();
			await wait(crop, (value) => near(value, before.screens[0].surfaces[0].definition.crop));
			assert.deepEqual(await screens(), before);
		}
		stage = 'Escape stops accepted origin drag';
		before = await screens();
		box = await begin(null);
		const current = await crop();
		await move(
			box,
			{ x: current.x + current.width / 2 + 0.04, y: current.y + current.height / 2 + 0.02 },
			12
		);
		await wait(crop, (value) => near(value, { x: current.x + 0.04, y: current.y + 0.02 }));
		await page.keyboard.press('Escape');
		await wait(screens, () => !history.active_edit_session);
		const accepted = await crop();
		await move(box, { x: 0.8, y: 0.8 });
		await page.mouse.up();
		await ready();
		assert.deepEqual(await crop(), accepted, 'held-pointer motion after Escape cannot author');
		after = await screens();
		await roundtrip(before, after, accepted);
		stage = 'finite crop Parameters remain contained';
		const beforeClamp = await screens();
		await controls.setNumber('Crop origin', 0.9, 0);
		const maximumOrigin = 1 - beforeClamp.screens[0].surfaces[0].definition.crop.width;
		await wait(crop, (value) => near(value, { x: maximumOrigin }));
		await wait(
			() => controls.number('Crop origin').inputValue(),
			(value) => Math.abs(Number(value) - maximumOrigin) < 0.005
		);
		const clamped = await screens();
		await roundtrip(beforeClamp, clamped, clamped.screens[0].surfaces[0].definition.crop);
		const clampedDepth = history.undo_len;
		await controls.setNumber('Crop origin', 1.5, 0);
		await wait(
			() => controls.number('Crop origin').inputValue(),
			(value) => Math.abs(Number(value) - maximumOrigin) < 0.005
		);
		assert.deepEqual(await screens(), clamped);
		assert.equal(history.undo_len, clampedDepth, 'a clamped no-op cannot consume history');
		await undo();
		await wait(crop, (value) => near(value, beforeClamp.screens[0].surfaces[0].definition.crop));
		assert.deepEqual(await screens(), beforeClamp);
		await redo();
		await wait(crop, (value) => near(value, clamped.screens[0].surfaces[0].definition.crop));
		stage = 'Inspector crop feedback before pointer release';
		before = await screens();
		const cropBeforeSlider = before.screens[0].surfaces[0].definition.crop;
		await panel.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await wait(
			() => panel.locator('output[aria-label="View zoom"]').getAttribute('data-zoom'),
			(value) => Math.abs(Number(value) - 1.2) < 1e-9
		);
		const zoomBeforeSlider = await panel.locator('output[aria-label="View zoom"]').innerText();
		assert.notEqual(zoomBeforeSlider, '100%', 'exercise a camera different from Home');
		const canvasBeforeSlider = await svg.boundingBox();
		const slider = controls.parameter('Crop size').getByRole('slider').nth(0);
		await slider.scrollIntoViewIfNeeded();
		const sliderBounds = await slider.boundingBox();
		assert(
			sliderBounds && sliderBounds.width > 10 && sliderBounds.height > 0,
			`bounded vector slider must provide a usable drag area: ${JSON.stringify(sliderBounds)}`
		);
		await page.mouse.move(
			sliderBounds.x + sliderBounds.width / 2,
			sliderBounds.y + sliderBounds.height / 2
		);
		await page.mouse.down();
		await wait(screens, () => history.active_edit_session);
		await page.mouse.move(
			sliderBounds.x + sliderBounds.width * 0.3,
			sliderBounds.y + sliderBounds.height / 2,
			{ steps: 12 }
		);
		await wait(
			async () => {
				const authored = await crop();
				const displayed = Number(await svg.locator('rect').getAttribute('width'));
				const handle = await panel
					.getByRole('button', { name: 'Crop corner 2', exact: true })
					.boundingBox();
				const canvas = await svg.boundingBox();
				return { authored, displayed, handle, canvas, active: history.active_edit_session };
			},
			(value) => {
				if (!value.active || !value.handle || !value.canvas) return false;
				const handleX = value.handle.x + value.handle.width / 2;
				const expectedX =
					value.canvas.x + (value.authored.x + value.authored.width) * value.canvas.width;
				return (
					value.authored.width < cropBeforeSlider.width - 0.04 &&
					Math.abs(value.displayed - value.authored.width) < 0.00001 &&
					Math.abs(handleX - expectedX) < 1
				);
			}
		);
		assert.equal(
			await panel.locator('output[aria-label="View zoom"]').innerText(),
			zoomBeforeSlider
		);
		const canvasHeld = await svg.boundingBox();
		assert(
			canvasBeforeSlider &&
				canvasHeld &&
				['x', 'y', 'width', 'height'].every(
					(key) => Math.abs(canvasBeforeSlider[key] - canvasHeld[key]) < 0.01
				),
			'Inspector samples preserve the current view camera'
		);
		await page.mouse.up();
		await ready();
		await wait(screens, () => !history.active_edit_session);
		after = await screens();
		await roundtrip(before, after, after.screens[0].surfaces[0].definition.crop);
		stage = 'source loss and Undo';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		const manager = page.locator('section[aria-label="Media Manager"]');
		await manager.getByRole('button', { name: sourceMedia.label, exact: true }).click();
		await wait(
			() => inspector.locator('.inspector-header .title-text').innerText(),
			(value) => value === sourceMedia.label
		);
		await manager
			.getByRole('button', { name: sourceMedia.label, exact: true })
			.click({ button: 'right' });
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Delete', { exact: true }) })
			.click();
		await wait(inspectMedia, (value) => !value.media.some((entry) => entry.id === sourceMedia.id));
		await screenTab.click();
		await tree.getByRole('button', { name: first.label, exact: true }).click();
		await tab.click();
		await wait(
			() => controls.parameter('Source channel').locator('.reference-editor.link-missing').count(),
			(value) => value === 1
		);
		assert.deepEqual(await screens(), after);
		await undo();
		await wait(inspectMedia, (value) => value.media.some((entry) => entry.id === sourceMedia.id));
		assert.deepEqual(await inspectMedia(), media);
		assert.deepEqual(await screens(), after);
		stage = 'shared selection and panel lifetime';
		await tree.getByRole('button', { name: peer.label, exact: true }).click();
		await tree.getByRole('button', { name: first.label, exact: true }).click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping$/ })
			.click();
		await tab.click();
		await ready();
		assert.deepEqual(await screens(), after);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, [], 'finite crop controls and Inspector drags never reject');
		await svg.scrollIntoViewIfNeeded();
		if (!nativePage) await page.screenshot({ path: 'target/m05-surface-source-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual generic Surface Parameters/calibration/crop/origin/clamping/live-before-release/Escape/source-loss/history input',
				native: !!nativePage,
				browser: nativePage ? 'bundled WebView2' : browser.version(),
				surface: first.reference,
				source: source.reference,
				final_definition: await definition(),
				page_errors: errors.length,
				expected_crop_rejections: rejected.length,
				screenshot: nativePage ? null : 'target/m05-surface-source-ui.png',
				native_pixel_alignment: 'not established by DOM input or installation assertions'
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
		if (!nativePage)
			await page.screenshot({ path: 'target/m05-surface-source-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser?.close();
	}
}
