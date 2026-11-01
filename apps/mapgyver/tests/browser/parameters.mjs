// Real generic Golden parameter editors against the ordinary bundled app; queries only inspect.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';

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
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		await page.getByRole('button', { name: 'Add Composition', exact: true }).click();
		const catalog = await wait(inspectMedia, (value) =>
			value.media.some((item) => item.media_kind === 'Composition')
		);
		const composition = catalog.media.find((item) => item.media_kind === 'Composition').channels[0]
			.reference;
		const source = catalog.media.find((item) => item.media_kind === 'SolidColor');
		const inspect = async () =>
			(await edit({ kind: 'Composition', reference: composition, edit: { kind: 'Inspect' } }))
				.snapshot;
		const add = page
			.locator('.properties form')
			.filter({ has: page.getByRole('button', { name: 'Add instance', exact: true }) });
		await add.getByLabel('Find source channel', { exact: true }).fill(source.label);
		await add
			.getByLabel('Source channel', { exact: true })
			.selectOption({ label: `${source.label} / ${source.channels[0].label}` });
		await add.getByRole('button', { name: 'Add instance', exact: true }).click();
		const initial = await wait(inspect, (value) => value.instances.length === 1);
		const parameters = page.locator('.parameter-inspector');
		await wait(
			() => parameters.count(),
			(count) => count === 9
		);
		const ids = await parameters.evaluateAll((nodes) => nodes.map((node) => node.dataset.nodeId));
		assert.equal(new Set(ids).size, 9, 'Every editor is bound to its own real parameter node');
		const parameter = (label) => parameters.filter({ has: page.getByText(label, { exact: true }) });
		for (const label of [
			'Source media',
			'Source channel',
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
		stage = 'parameter-backed duplication and removal';
		await page.getByRole('button', { name: 'Copy instance', exact: true }).click();
		const copied = await wait(inspect, (value) => value.instances.length === 2);
		assert.deepEqual(copied.instances[1].definition, scaled.instances[0].definition);
		await wait(
			() => parameters.count(),
			(count) => count === 9
		);
		const copiedIds = await parameters.evaluateAll((nodes) =>
			nodes.map((node) => node.dataset.nodeId)
		);
		assert(
			copiedIds.every((id) => !ids.includes(id)),
			'Copy owns independent parameter nodes'
		);
		await page.getByRole('button', { name: 'Remove instance', exact: true }).click();
		await wait(inspect, (value) => value.instances.length === 1);
		await undo();
		await wait(inspect, (value) => isDeepStrictEqual(value, copied));
		assert.deepEqual(errors, []);
		assert.equal(rejected.length, 1);
		assert.equal(rejected[0].request.intent.kind, 'setParam');
		assert(intents.filter((intent) => intent.kind === 'setParam').length >= 3);
		assert(
			!intents.some((intent) => intent.payload?.edit?.kind === 'ConfigureInstance'),
			'Ordinary fields use generic SetParam'
		);
		await page.screenshot({ path: 'target/mapgyver-composition-parameters-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Composition generic Parameter nodes, edit/history/rejection and independent duplicate/remove',
				browser: browser.version(),
				parameter_nodes: 9,
				expected_rejections: 1,
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
