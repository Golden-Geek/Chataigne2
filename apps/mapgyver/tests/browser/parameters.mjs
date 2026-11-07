// Real generic Golden parameter editors against the ordinary bundled app; queries only inspect.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { verifyMediaParameters } from './media-parameters.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
export async function verifyParameterUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [],
		intents = [];
	let stage = 'Composition parameter creation';
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent') intents.push(frame.intent);
				if (frame.kind === 'intentBatch') intents.push(...frame.intents);
			} catch {}
		})
	);
	const rejected = observeRejectedRequests(page, () => stage);
	async function wait(read, test) {
		const deadline = Date.now() + 5000;
		while (true) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page.getByRole('tab').first().click();
		const manager = page.locator('section[data-panel-id="media-manager"]');
		const beforeCreation = await inspectMedia();
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name: 'Composition', exact: true }).click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.click();
		const catalog = await wait(inspectMedia, (value) =>
			value.media.some((item) => !beforeCreation.media.some((old) => old.id === item.id))
		);
		const composition = catalog.media.find(
			(item) => !beforeCreation.media.some((old) => old.id === item.id)
		).channels[0].reference;
		const owner = catalog.media.find((item) => item.id === composition.media);
		await manager.getByRole('button', { name: owner.label, exact: true }).click();
		const source = catalog.media.find((item) => item.media_kind === 'SolidColor');
		const inspect = async () =>
			(await edit({ kind: 'Composition', reference: composition, edit: { kind: 'Inspect' } }))
				.snapshot;
		const canvas = page.locator('[data-panel-id="composition"] .source-drop-zone');
		await canvas.locator('svg[aria-label="Composition transform controls"]').waitFor();
		const sourceRow = manager
			.getByRole('treeitem', { name: source.label, exact: true })
			.getByRole('button', { name: source.label, exact: true });
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		try {
			await sourceRow.dispatchEvent('dragstart', { dataTransfer: transfer });
			const bounds = await canvas.boundingBox();
			assert(bounds);
			const event = {
				dataTransfer: transfer,
				clientX: bounds.x + bounds.width / 2,
				clientY: bounds.y + bounds.height / 2
			};
			await canvas.dispatchEvent('dragover', event);
			await canvas.getByRole('status').filter({ hasText: 'Add instance' }).waitFor();
			await canvas.dispatchEvent('drop', event);
			await sourceRow.dispatchEvent('dragend', { dataTransfer: transfer });
		} finally {
			await transfer.dispose();
		}
		const initial = await wait(inspect, (value) => value.instances.length === 1);
		const parameters = page.locator('.parameter-inspector');
		await wait(
			() => parameters.count(),
			(count) => count === 10
		);
		const ids = await parameters.evaluateAll((nodes) => nodes.map((node) => node.dataset.nodeId));
		assert.equal(new Set(ids).size, 10, 'Every editor is bound to its own real parameter node');
		const parameter = (label) => parameters.filter({ has: page.getByText(label, { exact: true }) });
		for (const label of [
			'Target',
			'Resolution',
			'Output',
			'Position',
			'Scale',
			'Rotation',
			'Pivot',
			'Opacity',
			'Blend',
			'Enabled'
		])
			assert.equal(await parameter(label).count(), 1);
		const number = (label, index = 0) => parameter(label).locator('input.number-field').nth(index);
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		stage = 'generic parameter edit and history';
		await number('Opacity').fill('0.25');
		await number('Opacity').press('Enter');
		const opacity = await wait(inspect, (value) => value.instances[0].definition.opacity === 0.25);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, initial));
		await redo();
		await wait(inspect, (value) => isDeepStrictEqual(value, opacity));
		await number('Scale').fill('2');
		await number('Scale').press('Enter');
		const scaled = await wait(
			inspect,
			(value) => value.instances[0].definition.transform.scale[0] === 2
		);
		stage = 'generic parameter invariant rejection';
		await number('Scale').fill('0');
		await number('Scale').press('Enter');
		await wait(
			() => rejected.length,
			(count) => count === 1
		);
		assert.deepEqual(await inspect(), scaled);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, opacity));
		await redo();
		await wait(inspect, (value) => isDeepStrictEqual(value, scaled));
		await page.screenshot({ path: 'target/mapgyver-composition-parameters-ui.png' });
		stage = 'parameter-backed duplication and removal';
		await page.getByRole('button', { name: 'Copy instance', exact: true }).click();
		const copied = await wait(inspect, (value) => value.instances.length === 2);
		const child = copied.instances[1].reference.instance;
		assert.notEqual(child, scaled.instances[0].reference.instance);
		assert.deepEqual(copied.instances[1].definition, {
			...scaled.instances[0].definition,
			source: { ...scaled.instances[0].definition.source, media: child }
		});
		const aliases = (await inspectMedia()).media;
		assert.deepEqual(
			aliases.find((item) => item.id === child).channels.map((item) => item.definition),
			aliases
				.find((item) => item.id === scaled.instances[0].reference.instance)
				.channels.map((item) => item.definition)
		);
		// The backend commit can be observed before the UI receives its receipt
		// and selects the returned copy. Wait for that selection, not the old count.
		const copiedIds = await wait(
			() => parameters.evaluateAll((nodes) => nodes.map((node) => node.dataset.nodeId)),
			(values) => values.length === 10 && values.every((id) => !ids.includes(id))
		);
		assert(
			copiedIds.every((id) => !ids.includes(id)),
			'Copy owns independent parameter nodes'
		);
		await page.getByRole('button', { name: 'Remove instance', exact: true }).click();
		await wait(inspect, (value) => value.instances.length === 1);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, copied));
		await verifyMediaParameters({
			page,
			inspectMedia,
			composition,
			rejected,
			wait,
			stage: (value) => {
				stage = value;
			}
		});
		assert.deepEqual(errors, []);
		assert.equal(rejected.length, 2);
		assert.equal(rejected[0].request.intent.kind, 'setParamComponent');
		assert.equal(rejected[1].request.intent.kind, 'setParam');
		const scalarWrites = intents.filter((intent) => intent.kind === 'setParam');
		const componentWrites = intents.filter((intent) => intent.kind === 'setParamComponent');
		assert(
			scalarWrites.length > 0 && componentWrites.length > 0,
			'Verified scalar and vector field edits use generic Parameter intents'
		);
		assert(
			!intents.some((intent) => intent.payload?.edit?.kind === 'ConfigureInstance'),
			'Ordinary fields use generic SetParam'
		);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Composition instance and Media/channel-owned generic Parameters, visibility, history/rejection and independent copy/remove',
				browser: browser.version(),
				parameter_nodes: 10,
				generic_writes: { scalars: scalarWrites.length, components: componentWrites.length },
				expected_rejections: 2,
				page_errors: errors.length
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-4500)
			})
		);
		await page
			.screenshot({ path: 'target/mapgyver-composition-parameters-ui-failure.png' })
			.catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
