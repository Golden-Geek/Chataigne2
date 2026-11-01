// Actual bundled Composition input. Public backend operations below only inspect committed state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const near = (actual, expected, epsilon = 0.05) =>
	actual.every((value, index) => Math.abs(value - expected[index]) < epsilon);

export async function verifyCompositionUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	try {
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
		const tab = page.getByRole('tab').filter({ hasText: /^Composition$/ });
		await tab.click();
		const panel = page.getByRole('region', { name: 'Composition', exact: true });
		const addComposition = panel.getByRole('button', { name: 'Add Composition', exact: true });
		const ready = () => wait(() => addComposition.isEnabled(), Boolean);
		const media = await inspectMedia();
		await addComposition.click();
		const catalog = await wait(inspectMedia, (value) =>
			value.media.some((entry) => entry.media_kind === 'Composition')
		);
		const reference = catalog.media.find((entry) => entry.media_kind === 'Composition').channels[0]
			.reference;
		const inspect = async () =>
			(await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } })).snapshot;
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		async function restore(before, after) {
			await undo();
			await wait(inspect, (value) => isDeepStrictEqual(value, before));
			await redo();
			await wait(inspect, (value) => isDeepStrictEqual(value, after));
			await ready();
		}
		stage = 'canvas and cycle rejection';
		const initial = await inspect();
		await panel.getByLabel('Canvas width', { exact: true }).fill('400');
		await panel.getByLabel('Canvas height', { exact: true }).fill('300');
		await panel.getByRole('button', { name: 'Apply channel settings', exact: true }).click();
		const resized = await wait(
			inspect,
			(value) => value.canvas.width === 400 && value.canvas.height === 300
		);
		const add = panel
			.locator('form')
			.filter({ has: page.getByRole('button', { name: 'Add instance', exact: true }) });
		const root = catalog.media.find((entry) => entry.media_kind === 'Composition');
		await add
			.getByLabel('Source channel', { exact: true })
			.selectOption({ label: `${root.label} / ${root.channels[0].label}` });
		await add.getByRole('button', { name: 'Add instance', exact: true }).click();
		await panel.getByRole('alert').waitFor();
		assert.deepEqual(await inspect(), resized);
		await restore(initial, resized);
		const solid = media.media.find((entry) => entry.media_kind === 'SolidColor');
		const picture = media.media.find((entry) => entry.media_kind === 'Picture');
		for (const source of [solid, picture]) {
			await add.getByLabel('Find source channel', { exact: true }).fill(source.label);
			await add
				.getByLabel('Source channel', { exact: true })
				.selectOption({ label: `${source.label} / ${source.channels[0].label}` });
			await add.getByRole('button', { name: 'Add instance', exact: true }).click();
			await ready();
		}
		let snapshot = await wait(inspect, (value) => value.instances.length === 2);
		const primary = snapshot.instances[0].reference.instance;
		const peer = snapshot.instances[1];
		const navigation = panel.getByRole('navigation', { name: 'Ordered instances', exact: true });
		await navigation
			.getByRole('button', { name: snapshot.instances[0].label, exact: true })
			.click();
		assert.deepEqual(await inspect(), snapshot, 'selection does not author');
		const form = panel.getByRole('region', { name: 'Selected instance', exact: true });
		stage = 'complete instance form';
		await form.getByLabel('Instance name', { exact: true }).fill('UI Instance');
		await form.getByRole('button', { name: 'Rename', exact: true }).click();
		await wait(inspect, (value) => value.instances[0].label === 'UI Instance');
		snapshot = await inspect();
		await form
			.getByLabel('Source channel', { exact: true })
			.selectOption({ label: `${solid.label} / ${solid.channels[1].label}` });
		for (const [label, value] of Object.entries({
			'Position X': 200,
			'Position Y': 150,
			'Scale X': -1.2,
			'Scale Y': 0.8,
			'Pivot X': 0.3,
			'Pivot Y': 0.7,
			'Rotation (radians, clockwise)': 0.25,
			Opacity: 0.6
		}))
			await form.getByLabel(label, { exact: true }).fill(String(value));
		await form.getByLabel('Blend').selectOption('Screen');
		await form.getByLabel('Enabled', { exact: true }).uncheck();
		assert.deepEqual(await inspect(), snapshot, 'form edits remain drafts');
		await form.getByRole('button', { name: 'Apply instance', exact: true }).click();
		let after = await wait(inspect, (value) => value.instances[0].definition.blend === 'Screen');
		assert.deepEqual(after.instances[0].definition, {
			...snapshot.instances[0].definition,
			source: solid.channels[1].reference,
			transform: {
				position: [200, 150],
				scale: [-1.2, 0.8].map(Math.fround),
				pivot: [0.3, 0.7].map(Math.fround),
				rotation: 0.25
			},
			opacity: Math.fround(0.6),
			blend: 'Screen',
			enabled: false
		});
		assert.deepEqual(after.instances[1], peer);
		await restore(snapshot, after);
		stage = 'order copy removal';
		snapshot = after;
		await navigation.getByRole('button', { name: 'Move UI Instance later', exact: true }).click();
		after = await wait(inspect, (value) => value.instances[1].reference.instance === primary);
		await restore(snapshot, after);
		snapshot = after;
		await panel.getByRole('button', { name: 'Duplicate', exact: true }).click();
		after = await wait(inspect, (value) => value.instances.length === 3);
		const copy = after.instances.find(
			(entry) =>
				!snapshot.instances.some((before) => before.reference.instance === entry.reference.instance)
		);
		assert(copy);
		assert.deepEqual(copy.definition, snapshot.instances[1].definition);
		await panel.getByRole('button', { name: 'Remove', exact: true }).click();
		await wait(inspect, (value) => value.instances.length === 2);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, after));
		await redo();
		await wait(inspect, (value) => value.instances.length === 2);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, after));
		await ready();
		await navigation.getByRole('button', { name: 'UI Instance', exact: true }).click();
		const current = async () =>
			(await inspect()).instances.find((entry) => entry.reference.instance === primary);
		const svg = panel.getByRole('group', { name: 'Composition transform controls', exact: true });
		async function begin(selector) {
			await ready();
			// History commits can reach the public read before the panel has rendered their response.
			const expected = (await current()).definition.transform;
			for (const [label, value] of Object.entries({
				'Position X': expected.position[0],
				'Position Y': expected.position[1],
				'Scale X': expected.scale[0],
				'Scale Y': expected.scale[1],
				'Rotation (radians, clockwise)': expected.rotation
			}))
				await wait(
					async () => Number(await form.getByLabel(label, { exact: true }).inputValue()),
					(actual) => Math.fround(actual) === value
				);
			await svg.scrollIntoViewIfNeeded();
			const rect = await svg.boundingBox();
			const handle = await svg.locator(selector).boundingBox();
			assert(rect && handle, 'naturally rendered handles need geometry');
			const point = {
				x: ((handle.x + handle.width / 2 - rect.x) * 400) / rect.width,
				y: ((handle.y + handle.height / 2 - rect.y) * 300) / rect.height
			};
			await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
			await page.mouse.down();
			return { rect: await svg.boundingBox(), point };
		}
		const move = async (rect, point, steps = 12) =>
			page.mouse.move(
				rect.x + (point.x * rect.width) / 400,
				rect.y + (point.y * rect.height) / 300,
				{ steps }
			);
		for (const [mode, selector] of [
			['Translate', 'circle.selected-pivot'],
			['Rotate', 'circle.rotate'],
			['Scale', 'rect.scale']
		]) {
			stage = `${mode} grouped gesture`;
			snapshot = await inspect();
			const before = (await current()).definition.transform;
			const { rect, point } = await begin(selector);
			const [x, y] = before.position;
			let target;
			if (mode === 'Translate') target = { x: point.x + 30, y: point.y + 20 };
			if (mode === 'Rotate') {
				const angle = Math.PI / 4;
				target = {
					x: x + Math.cos(angle) * (point.x - x) - Math.sin(angle) * (point.y - y),
					y: y + Math.sin(angle) * (point.x - x) + Math.cos(angle) * (point.y - y)
				};
			}
			if (mode === 'Scale') target = { x: x + 2 * (point.x - x), y: y + 2 * (point.y - y) };
			await move(rect, target);
			await page.mouse.up();
			await ready();
			const transform = (await current()).definition.transform;
			if (mode === 'Translate') assert(near(transform.position, [x + 30, y + 20], 2));
			if (mode === 'Rotate')
				assert(Math.abs(transform.rotation - before.rotation - Math.PI / 4) < 0.05);
			if (mode === 'Scale')
				assert(
					near(
						transform.scale,
						before.scale.map((value) => value * 2),
						0.08
					),
					JSON.stringify({ mode, before, transform, point, target, rect })
				);
			const field = { Translate: 'position', Rotate: 'rotation', Scale: 'scale' }[mode];
			assert.deepEqual(transform, { ...before, [field]: transform[field] });
			after = await inspect();
			assert.deepEqual(after, {
				...snapshot,
				instances: snapshot.instances.map((entry) =>
					entry.reference.instance === primary
						? { ...entry, definition: { ...entry.definition, transform } }
						: entry
				)
			});
			await restore(snapshot, after);
		}
		stage = 'Escape and invalid form';
		snapshot = await inspect();
		const { rect, point } = await begin('circle.selected-pivot');
		await move(rect, { x: point.x + 12, y: point.y + 8 });
		await wait(current, (value) =>
			near(value.definition.transform.position, [point.x + 12, point.y + 8], 2)
		);
		const accepted = (await current()).definition.transform;
		await page.keyboard.press('Escape');
		await ready();
		await move(rect, { x: point.x + 40, y: point.y + 40 });
		await page.mouse.up();
		assert.deepEqual(
			(await current()).definition.transform,
			accepted,
			'Escape stops later held-pointer samples'
		);
		after = await inspect();
		await restore(snapshot, after);
		await form.getByLabel('Scale X', { exact: true }).fill('0');
		await form.getByRole('button', { name: 'Apply instance', exact: true }).click();
		await panel.getByRole('alert').waitFor();
		assert.deepEqual(await inspect(), after);
		await restore(snapshot, after);
		stage = 'source loss and panel lifetime';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		const article = page.locator('section.manager article').filter({
			has: page.getByRole('textbox', { name: `Media name: ${solid.label}`, exact: true })
		});
		await article.getByRole('button', { name: 'Delete media', exact: true }).click();
		await wait(inspectMedia, (value) => !value.media.some((entry) => entry.id === solid.id));
		await tab.click();
		await wait(
			() =>
				form
					.getByLabel('Source channel', { exact: true })
					.locator('option')
					.filter({ hasText: 'Missing channel' })
					.count(),
			(value) => value === 1
		);
		assert.deepEqual(await inspect(), after);
		await undo();
		await wait(inspectMedia, (value) => value.media.some((entry) => entry.id === solid.id));
		assert.deepEqual(await inspect(), after);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		await tab.click();
		await ready();
		assert.deepEqual(await inspect(), after);
		const finalCatalog = await inspectMedia();
		assert.deepEqual(finalCatalog.assets, media.assets);
		assert.deepEqual(
			finalCatalog.media.filter((entry) => entry.id !== reference.media),
			media.media
		);
		const outlineCount = await svg.locator('polygon').count();
		await svg.scrollIntoViewIfNeeded();
		await page.screenshot({ path: 'target/m06-composition-ui.png' });
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual Composition forms/source/order/copy/removal/translate/rotate/scale/Escape/rejection/source-loss/panel/history input; native preview and extent outlines remain unverified',
				browser: browser.version(),
				reference,
				primary,
				copy: copy.reference,
				final_transform: accepted,
				source_outline_count: outlineCount,
				page_errors: errors.length,
				screenshot: 'target/m06-composition-ui.png'
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
		await page.screenshot({ path: 'target/m06-composition-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
