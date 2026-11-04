import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

/** Real bundled editor drop handlers consume payloads authored by real outliner dragstart.
 * Backend calls observe state only; creation, source edits and history use UI interactions. */
export async function verifySourceDropUi({ base, post, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [],
		intents = [],
		payloads = [];
	let stage = 'startup';
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent') intents.push(frame.intent);
			} catch {}
		})
	);
	const rejected = observeRejectedRequests(page, () => stage);
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	const panel = (name) => page.locator(`section[aria-label="${name}"]`);
	const parameters = screenParameterControls(page);
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	const screens = async () => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload: { kind: 'InspectManager' }
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value.snapshot;
	};
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	async function history(read, before, after) {
		await undo();
		await wait(read, (value) => isDeepStrictEqual(value, before));
		await redo();
		await wait(read, (value) => isDeepStrictEqual(value, after));
	}
	async function mediaLabel(media, channel = null) {
		await tab('Medias').click();
		const manager = panel('Medias');
		const parent = manager.getByRole('treeitem', { name: media.label, exact: true });
		const parentLabel = parent.getByRole('button', { name: media.label, exact: true });
		if (!channel) return parentLabel;
		if ((await parent.getAttribute('aria-expanded')) === 'false') {
			await parent.focus();
			await parent.press('ArrowRight');
		}
		const index = media.channels.findIndex((item) => item.reference.channel === channel);
		assert(index >= 0);
		const rows = manager.getByRole('treeitem');
		const labels = await rows.evaluateAll((elements) =>
			elements.map((element) => element.getAttribute('aria-label'))
		);
		const start = labels.indexOf(media.label);
		assert(start >= 0);
		const row = rows.nth(start + index + 1);
		assert.equal(
			Number(await row.getAttribute('aria-level')),
			Number(await parent.getAttribute('aria-level')) + 1
		);
		return row.getByRole('button', { name: media.channels[index].label, exact: true });
	}
	async function drop(source, target, point = null, accepted = true) {
		await source.waitFor({ state: 'visible' });
		await target.waitFor({ state: 'visible' });
		assert.equal(await source.getAttribute('draggable'), 'true');
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		const delivered = await target.evaluateHandle((element) => {
			const observed = { point: null };
			element.addEventListener(
				'drop',
				(event) => {
					observed.point = { x: event.clientX, y: event.clientY };
				},
				{ once: true, capture: true }
			);
			return observed;
		});
		try {
			await source.dispatchEvent('dragstart', { dataTransfer: transfer });
			const observed = await transfer.evaluate((value) =>
				[...value.types].map((type) => [type, value.getData(type)])
			);
			assert(
				observed.some(([, value]) => value.includes('nodeId')),
				'Real outliner authored a node payload'
			);
			payloads.push(observed);
			const bounds = await target.boundingBox();
			assert(bounds);
			const position = point ?? { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
			const event = { dataTransfer: transfer, clientX: position.x, clientY: position.y };
			await target.dispatchEvent('dragenter', event);
			if (accepted) {
				const object = await target.getAttribute('data-source-drop-object');
				const hint = object
					? 'Replace source'
					: (await target.getAttribute('data-screen-drop-target')) !== null
						? 'Add Surface'
						: 'Add instance';
				// Native dragging repeats dragover while asynchronous snapshots settle.
				await wait(
					async () => {
						await target.dispatchEvent('dragover', event);
						return page
							.getByRole('status')
							.filter({ hasText: new RegExp(`^${hint}$`) })
							.count();
					},
					(count) => count === 1
				);
			} else await target.dispatchEvent('dragover', event);
			await target.dispatchEvent('drop', event);
			await source.dispatchEvent('dragend', { dataTransfer: transfer });
			return await delivered.evaluate((observed) => observed.point);
		} finally {
			await delivered.dispose();
			await transfer.dispose();
		}
	}
	function sourceOnly(before, after, source) {
		assert.deepEqual(after, { ...before, definition: { ...before.definition, source } });
	}
	try {
		await page.goto(base);
		const catalog = await inspectMedia();
		const solid = catalog.media.find((item) => item.media_kind === 'SolidColor');
		assert(solid?.channels.length >= 2);
		const first = solid.channels[0].reference,
			second = solid.channels[1].reference;
		stage = 'Screen Media background drop';
		await tab('Screens').click();
		await panel('Screens')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await page.getByRole('menuitem', { name: 'Screen', exact: true }).click();
		let snapshot = await wait(screens, (value) => value.screens.length === 1);
		const screenId = snapshot.screens[0].id;
		const baseline = snapshot;
		await tab('Screen Warping').click();
		const screenDrop = panel('Screen Warping').locator('[data-screen-drop-target]');
		async function screenBackground() {
			const mapping = panel('Screen Warping').getByLabel('Surface mapping controls', {
				exact: true
			});
			await mapping.waitFor();
			const bounds = await screenDrop.boundingBox();
			const active = await mapping.boundingBox();
			assert(bounds && active && active.x > bounds.x + 1, 'Screen has an empty themed surround');
			// Default Surfaces overlap the complete active area; its center is a replacement hit.
			// Drop inside the visible left surround to exercise an actual empty background point.
			return { x: (bounds.x + active.x) / 2, y: bounds.y + bounds.height / 2 };
		}
		await drop(await mediaLabel(solid), screenDrop, await screenBackground());
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 2);
		assert.deepEqual(snapshot.screens[0].surfaces[0], baseline.screens[0].surfaces[0]);
		assert.deepEqual(snapshot.screens[0].definition, baseline.screens[0].definition);
		const mediaSurface = snapshot.screens[0].surfaces[1];
		assert.deepEqual(
			mediaSurface.definition.source,
			first,
			'Media chooses the first backend-ordered channel'
		);
		await history(screens, baseline, snapshot);
		stage = 'Screen exact Channel background drop';
		const beforeChannel = snapshot;
		await drop(await mediaLabel(solid, second.channel), screenDrop, await screenBackground());
		snapshot = await wait(screens, (value) => value.screens[0].surfaces.length === 3);
		assert.deepEqual(snapshot.screens[0].surfaces.slice(0, 2), beforeChannel.screens[0].surfaces);
		assert.deepEqual(snapshot.screens[0].surfaces[2].definition.source, second);
		await history(screens, beforeChannel, snapshot);
		stage = 'Surface source replacement preserves Parameters and peers';
		await tab('Screens').click();
		await panel('Screens')
			.getByRole('button', { name: mediaSurface.label, exact: true })
			.click();
		await parameters.parameter('Opacity').waitFor();
		await parameters.setNumber('Opacity', 0.37);
		const beforeReplace = await wait(
			screens,
			(value) => Math.abs(value.screens[0].surfaces[1].definition.opacity - 0.37) < 0.00001
		);
		const surfaceTarget = panel('Screen Warping')
			.locator(`button[data-surface-drop-target="${mediaSurface.reference.surface}"]`)
			.first();
		await drop(await mediaLabel(solid, second.channel), surfaceTarget);
		snapshot = await wait(screens, (value) =>
			isDeepStrictEqual(value.screens[0].surfaces[1].definition.source, second)
		);
		sourceOnly(beforeReplace.screens[0].surfaces[1], snapshot.screens[0].surfaces[1], second);
		assert.deepEqual(
			snapshot.screens[0].surfaces.filter((_, index) => index !== 1),
			beforeReplace.screens[0].surfaces.filter((_, index) => index !== 1)
		);
		assert.deepEqual(snapshot.screens[0].definition, beforeReplace.screens[0].definition);
		await history(screens, beforeReplace, snapshot);
		stage = 'Composition Media background drop under pan and zoom';
		await tab('Composition').click();
		await panel('Composition')
			.getByRole('button', { name: 'Add Composition', exact: true })
			.click();
		const withComposition = await wait(inspectMedia, (value) =>
			value.media.some((item) => item.media_kind === 'Composition')
		);
		const composition = withComposition.media.find((item) => item.media_kind === 'Composition');
		const reference = composition.channels[0].reference;
		const inspect = async () =>
			(await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } })).snapshot;
		const empty = await inspect();
		assert.equal(empty.instances.length, 0);
		const compositionPanel = panel('Composition');
		const compositionDrop = compositionPanel.locator('[data-composition-drop-target]');
		const transform = compositionPanel.getByLabel('Composition transform controls', {
			exact: true
		});
		const view = compositionPanel.getByLabel('Composition view', { exact: true });
		await transform.waitFor();
		const originalBounds = await transform.boundingBox();
		assert(originalBounds);
		await page.mouse.move(
			originalBounds.x + originalBounds.width / 2,
			originalBounds.y + originalBounds.height / 2
		);
		await page.mouse.wheel(0, -100);
		await view.getByLabel('View zoom').filter({ hasText: '115%' }).waitFor();
		await view.focus();
		await page.keyboard.down('Space');
		await page.mouse.down();
		await page.mouse.move(
			originalBounds.x + originalBounds.width / 2 + 35,
			originalBounds.y + originalBounds.height / 2 - 20
		);
		await page.mouse.up();
		await page.keyboard.up('Space');
		async function canvasPoint(x, y) {
			const bounds = await transform.boundingBox();
			assert(bounds);
			return { x: bounds.x + bounds.width * x, y: bounds.y + bounds.height * y };
		}
		const placementBounds = await transform.boundingBox();
		assert(placementBounds);
		const intendedPoint = {
			x: placementBounds.x + placementBounds.width * 0.31,
			y: placementBounds.y + placementBounds.height * 0.44
		};
		const deliveredPoint = await drop(await mediaLabel(solid), compositionDrop, intendedPoint);
		assert(deliveredPoint);
		for (const axis of ['x', 'y'])
			assert(
				Math.abs(deliveredPoint[axis] - intendedPoint[axis]) < 1,
				'Drag stays at the intended CSS point'
			);
		let scene = await wait(inspect, (value) => value.instances.length === 1);
		assert.deepEqual(scene.instances[0].definition.source, first);
		// DragEvent quantizes CSS coordinates. Compare canvas placement to the actual
		// delivered event through independently measured, transformed SVG bounds.
		for (const [index, axis, length] of [
			[0, 'x', 'width'],
			[1, 'y', 'height']
		])
			assert(
				Math.abs(
					scene.instances[0].definition.transform.position[index] -
						((deliveredPoint[axis] - placementBounds[axis]) / placementBounds[length]) *
							scene.canvas[length]
				) < 0.5,
				'Drop uses transformed canvas coordinates'
			);
		await history(inspect, empty, scene);
		stage = 'Composition exact Channel background drop';
		const beforeSecond = scene;
		await drop(
			await mediaLabel(solid, second.channel),
			compositionDrop,
			await canvasPoint(0.68, 0.62)
		);
		scene = await wait(inspect, (value) => value.instances.length === 2);
		assert.deepEqual(scene.instances[0], beforeSecond.instances[0]);
		assert.deepEqual(scene.instances[1].definition.source, second);
		await history(inspect, beforeSecond, scene);
		stage = 'Instance source replacement preserves Parameters and peers';
		await (await mediaLabel(composition, reference.channel)).click();
		await page
			.getByRole('navigation', { name: 'Ordered instances', exact: true })
			.getByRole('button', { name: scene.instances[1].label, exact: true })
			.click();
		await parameters.parameter('Opacity').waitFor();
		await parameters.setNumber('Opacity', 0.29);
		const beforeInstance = await wait(
			inspect,
			(value) => Math.abs(value.instances[1].definition.opacity - 0.29) < 0.00001
		);
		const instance = beforeInstance.instances[1].reference.instance;
		const instanceTarget = compositionPanel.locator(
			`button[data-instance-drop-target="${instance}"][data-instance-handle="Translate"]`
		);
		await drop(await mediaLabel(solid, first.channel), instanceTarget);
		scene = await wait(inspect, (value) =>
			isDeepStrictEqual(value.instances[1].definition.source, first)
		);
		sourceOnly(beforeInstance.instances[1], scene.instances[1], first);
		assert.deepEqual(scene.instances[0], beforeInstance.instances[0]);
		assert.deepEqual(scene.canvas, beforeInstance.canvas);
		await history(inspect, beforeInstance, scene);
		stage = 'Wrong parameter drag ignored coherently';
		const wrong = parameters.parameter('Opacity').locator('.parameter-label');
		const dropCount = () =>
			intents.filter(
				(intent) =>
					intent.payload?.kind === 'DropSource' || intent.payload?.edit?.kind === 'DropSource'
			).length;
		const beforeWrong = dropCount();
		await drop(wrong, compositionDrop, await canvasPoint(0.5, 0.5), false);
		assert.deepEqual(await inspect(), scene);
		assert.equal(dropCount(), beforeWrong, 'An Inspector Parameter is not a playable source node');
		stage = 'Composition cycle drop is atomically rejected';
		await drop(await mediaLabel(composition), compositionDrop, await canvasPoint(0.5, 0.5));
		await wait(
			() => rejected.length,
			(value) => value === 1
		);
		assert.deepEqual(await inspect(), scene);
		await history(inspect, beforeInstance, scene);
		assert.deepEqual(await screens(), snapshot);
		assert.deepEqual(
			await inspectMedia(),
			withComposition,
			'Dropping never mutates source Media or Channel Parameters'
		);
		assert.equal(dropCount(), 7);
		assert.deepEqual(errors, []);
		await page.screenshot({ path: 'target/mapgyver-source-drop-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Observed outliner Media/Channel drags to Screen and Composition backgrounds or existing objects',
				successful_drops: 6,
				expected_cycle_rejections: 1,
				wrong_node_ignored: true,
				single_step_history: true,
				source_only_replacement: true,
				pan_zoom_coordinates: true,
				screen: screenId,
				observed_payloads: payloads.length,
				page_errors: errors.length,
				screenshot: 'target/mapgyver-source-drop-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				intents: intents.slice(-8),
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		await page.screenshot({ path: 'target/mapgyver-source-drop-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
