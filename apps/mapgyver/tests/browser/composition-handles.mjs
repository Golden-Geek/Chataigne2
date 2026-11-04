// Actual bundled/native pointer acceptance. Backend operations arrange/read the isolated scene;
// every geometry sample, selection, history and camera action below uses real DOM input.
import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { screenParameterControls } from './screen-parameters.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';

const near = (actual, expected, epsilon = 0.05) =>
	actual.every((value, index) => Math.abs(value - expected[index]) < epsilon);
const fraction = {
	TopLeft: [0, 0],
	Top: [0.5, 0],
	TopRight: [1, 0],
	Right: [1, 0.5],
	BottomRight: [1, 1],
	Bottom: [0.5, 1],
	BottomLeft: [0, 1],
	Left: [0, 0.5]
};
function projected(transform, point) {
	const angle = (transform.rotation_degrees * Math.PI) / 180;
	const x = (point[0] - transform.pivot[0]) * 20 * transform.scale[0];
	const y = (point[1] - transform.pivot[1]) * 16 * transform.scale[1];
	return [
		transform.position[0] + Math.cos(angle) * x - Math.sin(angle) * y,
		transform.position[1] + Math.sin(angle) * x + Math.cos(angle) * y
	];
}

export async function verifyCompositionHandles({ page, edit }) {
	assert(page, 'Resize acceptance requires the actual native renderer metadata');
	let stage = 'arrange';
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	async function wait(read, test) {
		const deadline = Date.now() + 6000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Handle source'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 20, height: 16 } }
	});
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Handle canvas'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const { instances } = await edit({
		kind: 'Composition',
		reference,
		edit: { kind: 'AddInstances', sources: [source, source] }
	});
	const operation = (value) => edit({ kind: 'Composition', reference, edit: value });
	const inspect = async () => (await operation({ kind: 'Inspect' })).snapshot;
	const primary = instances[0].instance;
	const initial = await inspect();
	await operation({
		kind: 'ConfigureInstance',
		instance: primary,
		definition: {
			...initial.instances[0].definition,
			transform: { position: [200, 150], scale: [-4, 3], rotation_degrees: 27, pivot: [0.3, 0.7] }
		}
	});
	await operation({
		kind: 'ConfigureInstance',
		instance: instances[1].instance,
		definition: {
			...initial.instances[1].definition,
			transform: { position: [320, 230], scale: [2, 2], rotation_degrees: 0, pivot: [0.5, 0.5] }
		}
	});
	const baseline = await inspect();
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	await tab('Medias').click();
	await tab('Composition').click();
	const manager = page.locator('section[aria-label="Medias"]');
	const parent = manager.getByRole('treeitem', { name: 'Handle canvas', exact: true });
	await parent.getByRole('button', { name: 'Handle canvas', exact: true }).click();
	if ((await parent.getAttribute('aria-expanded')) === 'false') {
		await parent.focus();
		await parent.press('ArrowRight');
	}
	const child = manager.getByRole('treeitem', { name: baseline.instances[0].label, exact: true });
	await child.getByRole('button', { name: baseline.instances[0].label, exact: true }).click();
	const panel = page.locator('section[aria-label="Composition"]');
	const view = panel.getByRole('application', { name: 'Composition view', exact: true });
	const svg = view.getByRole('group', { name: 'Composition transform controls', exact: true });
	const widgets = screenParameterControls(page);
	const ready = async () => {
		await wait(
			() => panel.getAttribute('data-composition-busy'),
			(value) => value === 'false'
		);
		await wait(
			() => panel.getAttribute('data-composition-loading'),
			(value) => value === 'false'
		);
	};
	await view
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	await wait(
		() => panel.locator('[data-instance-resize]').count(),
		(count) => count === 8
	);
	assert.equal(await panel.locator('[data-instance-rotation]').count(), 4);
	assert.equal(await panel.locator('[data-instance-handle="Scale"]').count(), 0);
	await widgets.number('Position').waitFor();
	assert.equal(Number(await widgets.number('Position').inputValue()), 200);
	assert.deepEqual(await inspect(), baseline, 'shared selection does not author');
	const current = async () =>
		(await inspect()).instances.find((item) => item.reference.instance === primary);
	const undo = async () => {
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await ready();
	};
	const redo = async () => {
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await ready();
	};
	async function history(before, after) {
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, before));
		await redo();
		await wait(inspect, (value) => isDeepStrictEqual(value, after));
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, before));
	}
	async function begin(selector, offset = [0, 0]) {
		await ready();
		// A receipt may precede the DOM update from the same commit.
		const expected = (await current()).definition.transform;
		await wait(
			async () => [
				Number(await widgets.number('Position', 0).inputValue()),
				Number(await widgets.number('Position', 1).inputValue())
			],
			(value) => near(value, expected.position)
		);
		const rect = await svg.boundingBox();
		const handle = panel.locator(selector);
		await wait(() => handle.isEnabled(), Boolean);
		const bounds = await handle.boundingBox();
		assert(rect && bounds);
		const x = bounds.x + bounds.width / 2 + offset[0],
			y = bounds.y + bounds.height / 2 + offset[1];
		await page.mouse.move(x, y);
		await page.mouse.down();
		return { rect, point: [((x - rect.x) * 400) / rect.width, ((y - rect.y) * 300) / rect.height] };
	}
	async function move(rect, point) {
		await page.mouse.move(
			rect.x + (point[0] * rect.width) / 400,
			rect.y + (point[1] * rect.height) / 300,
			{ steps: 12 }
		);
	}
	const reports = [];
	for (const [name, at] of Object.entries(fraction)) {
		stage = `resize ${name}`;
		const before = await inspect(),
			origin = (await current()).definition.transform;
		const opposite = at.map((value) => 1 - value),
			anchor = projected(origin, opposite);
		const { rect, point } = await begin(`[data-instance-resize="${name}"]`, [1, -1]);
		const angle = (origin.rotation_degrees * Math.PI) / 180;
		const delta = [
			Math.cos(angle) * 8 - Math.sin(angle) * 6,
			Math.sin(angle) * 8 + Math.cos(angle) * 6
		];
		await move(rect, [point[0] + delta[0], point[1] + delta[1]]);
		await page.mouse.up();
		await ready();
		const transform = (await current()).definition.transform;
		const expectedScale = origin.scale.map(
			(value, axis) => value + (at[axis] === 0.5 ? 0 : [8 / 20, 6 / 16][axis] * (2 * at[axis] - 1))
		);
		assert(
			near(transform.scale, expectedScale, 0.08),
			JSON.stringify({ name, transform, expectedScale })
		);
		assert(near(projected(transform, opposite), anchor), `${name} fixes opposite anchor`);
		for (let axis = 0; axis < 2; axis++)
			if (at[axis] === 0.5) assert.equal(transform.scale[axis], origin.scale[axis]);
		assert.equal(transform.rotation_degrees, origin.rotation_degrees);
		assert.deepEqual(transform.pivot, origin.pivot);
		const after = await inspect();
		assert.deepEqual(after.instances[1], before.instances[1]);
		await history(before, after);
		reports.push({ resize: name, fixedOppositeAnchor: true, groupedHistory: true });
	}
	for (const name of ['TopLeft', 'TopRight', 'BottomRight', 'BottomLeft']) {
		stage = `outside rotate ${name}`;
		const before = await inspect(),
			origin = (await current()).definition.transform;
		const control = panel.locator(`[data-instance-rotation="${name}"]`);
		await ready();
		await wait(() => control.isEnabled(), Boolean);
		const cursor = await control.evaluate((node) => getComputedStyle(node).cursor);
		assert(cursor.includes('url('), `outside zones use the rotation cursor: ${cursor}`);
		const { rect, point } = await begin(`[data-instance-rotation="${name}"]`);
		const corner = projected(origin, fraction[name]);
		assert(
			Math.hypot(
				((point[0] - corner[0]) * rect.width) / 400,
				((point[1] - corner[1]) * rect.height) / 300
			) > 10,
			'rotation hit is outside the actual corner'
		);
		const angle = Math.PI / 4,
			x = point[0] - origin.position[0],
			y = point[1] - origin.position[1];
		await move(rect, [
			origin.position[0] + Math.cos(angle) * x - Math.sin(angle) * y,
			origin.position[1] + Math.sin(angle) * x + Math.cos(angle) * y
		]);
		await page.mouse.up();
		await ready();
		const transform = (await current()).definition.transform;
		assert(
			Math.abs(transform.rotation_degrees - origin.rotation_degrees - 45) < (0.05 * 180) / Math.PI
		);
		assert.deepEqual(transform, { ...origin, rotation_degrees: transform.rotation_degrees });
		await history(before, await inspect());
		reports.push({ rotate: name, outsideCorner: true, rotationCursor: true, groupedHistory: true });
	}
	stage = 'smooth zoom then translated pointer';
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	const rect = await svg.boundingBox();
	const anchor = { x: rect.x + rect.width * 0.4, y: rect.y + rect.height * 0.45 };
	await view.evaluate((node) => {
		window.__compositionZoom = [];
		const until = performance.now() + 500;
		const sample = () => {
			const rect = node.querySelector('svg').getBoundingClientRect();
			window.__compositionZoom.push({
				zoom: Number(node.querySelector('output[data-zoom]').dataset.zoom),
				x: rect.x + rect.width * 0.4,
				y: rect.y + rect.height * 0.45
			});
			if (performance.now() < until) requestAnimationFrame(sample);
		};
		requestAnimationFrame(sample);
	});
	await page.mouse.move(anchor.x, anchor.y);
	await page.mouse.wheel(0, -100);
	await wait(
		() => view.locator('output[data-zoom]').getAttribute('data-zoom'),
		(value) => Math.abs(Number(value) - Math.exp(0.14)) < 1e-9
	);
	const samples = await page.evaluate(() => window.__compositionZoom);
	assert(
		new Set(
			samples
				.filter((sample) => sample.zoom > 1 && sample.zoom < Math.exp(0.14))
				.map((sample) => sample.zoom)
		).size >= 2,
		'zoom animates across frames'
	);
	for (const sample of samples)
		assert(
			Math.abs(sample.x - anchor.x) <= 1 && Math.abs(sample.y - anchor.y) <= 1,
			'smooth zoom retains pointer anchor'
		);
	assert.deepEqual(await inspect(), baseline, 'camera changes do not author');
	const drag = await begin('[data-instance-handle="Translate"]');
	await move(drag.rect, [drag.point[0] + 30, drag.point[1] + 20]);
	await page.mouse.up();
	await ready();
	assert(near((await current()).definition.transform.position, [230, 170], 2));
	await history(baseline, await inspect());
	stage = 'Escape and tab lifetime';
	const held = await begin('[data-instance-handle="Translate"]');
	await move(held.rect, [held.point[0] + 12, held.point[1] + 8]);
	await wait(current, (value) => near(value.definition.transform.position, [212, 158], 2));
	await page.keyboard.press('Escape');
	await ready();
	const stopped = await inspect();
	await move(held.rect, [held.point[0] + 40, held.point[1] + 40]);
	await page.mouse.up();
	assert.deepEqual(await inspect(), stopped, 'Escape discards later held samples');
	await history(baseline, stopped);
	await tab('Screens').click();
	await tab('Screen Warping').click();
	await tab('Composition').click();
	await ready();
	assert.deepEqual(await inspect(), baseline);
	await page.screenshot({ path: 'target/m06-composition-handles.png' });
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			native: true,
			scope:
				'actual pointer selection/anchored resize/outside rotation/cursor/smooth zoom/Escape/history/tab lifetime',
			reports,
			zoomSamples: samples.length,
			pageErrors: errors,
			rejected,
			screenshot: 'target/m06-composition-handles.png'
		})
	);
}
