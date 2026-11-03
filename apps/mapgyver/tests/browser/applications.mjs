// Generic Inspector inputs author state; public operations only inspect committed projections.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyApplicationUi({ base, edit }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'typed app control');
	const query = (value) => edit({ kind: 'Applications', edit: value });
	const catalog = async () =>
		(await query({ kind: 'InspectCatalog', search: '', start: 0, count: 128 })).page;
	const inspect = async (application) => (await query({ kind: 'Inspect', application })).snapshot;
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	async function wait(read, predicate) {
		const deadline = Date.now() + 6000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `typed app-control UI: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	async function text(input, value) {
		await input.fill(value);
		await input.press('Tab');
	}
	async function number(label, value) {
		const input = parameter(label).locator('input.number-field');
		await input.fill(String(value));
		await input.press('Enter');
	}
	async function add(kind) {
		await page
			.locator('.inspector-header')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await page.getByRole('menuitem', { name: kind, exact: true }).click();
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Applications$/ })
			.click();
		const panel = page.getByRole('region', { name: 'Applications', exact: true });
		await panel.getByRole('button', { name: 'Add child item', exact: true }).click();
		const application = (await wait(catalog, (value) => value.total === 1)).applications[0].id;
		const initial = (await inspect(application)).definition;
		const definition = async () => (await inspect(application)).definition;
		await parameter('App file').waitFor();
		await wait(
			() => parameter('Window').count(),
			(count) => count === 0
		);
		const file = parameter('App file').locator('input.path-input');
		const fileId = await parameter('App file').getAttribute('data-node-id');
		const executable = path.resolve('target/debug/application_process_fixture.exe');
		await text(file, executable);
		await wait(definition, (value) => value.executable === executable);
		await undo();
		await wait(definition, (value) => isDeepStrictEqual(value, initial));
		await redo();
		await wait(definition, (value) => value.executable === executable);
		assert.equal(await parameter('App file').getAttribute('data-node-id'), fileId);
		await text(
			parameter('Working directory').locator('input.path-input'),
			path.dirname(executable)
		);
		await parameter('Window').locator('select').selectOption('Normal');
		for (const [index, value] of ['literal & value', 'two words'].entries()) {
			await add('Argument');
			await text(parameter('Value').locator('input,textarea').nth(index), value);
		}
		await parameter('Inherit environment').getByRole('checkbox').uncheck();
		await add('Environment');
		await text(parameter('Name').locator('input,textarea'), 'MAPGYVER_UI');
		await text(parameter('Value').locator('input,textarea').nth(2), 'literal = value');
		await parameter('Ready when').locator('select').selectOption('Spout');
		await text(parameter('Spout sender').locator('input,textarea'), 'UI Exact Sender');
		await parameter('Wait for first frame').getByRole('checkbox').check();
		await number('Ready timeout (ms)', 2500);
		await number('Graceful stop (ms)', 25);
		await number('Retirement timeout (ms)', 1000);
		await number('Initial restart delay (ms)', 750);
		await number('Maximum restart delay (ms)', 3000);
		await parameter('Restart on failure').getByRole('checkbox').uncheck();
		const expected = {
			...initial,
			executable,
			directory: path.dirname(executable),
			arguments: ['literal & value', 'two words'],
			window: 'Normal',
			environment: [['MAPGYVER_UI', 'literal = value']],
			readiness: { kind: 'Spout', sender: 'UI Exact Sender', first_frame: true, timeout_ms: 2500 },
			stop: { graceful_ms: 25, retirement_ms: 1000 },
			restart: { enabled: false, initial_ms: 750, maximum_ms: 3000 }
		};
		await wait(definition, (value) => isDeepStrictEqual(value, expected));
		await wait(
			() => parameter('Initial restart delay (ms)').count(),
			(count) => count === 0
		);
		await parameter('Inherit environment').getByRole('checkbox').check();
		await wait(definition, (value) => value.environment === null);
		await undo();
		await wait(definition, (value) => isDeepStrictEqual(value, expected));
		await parameter('App file').getByRole('button', { name: 'Clear', exact: true }).click();
		await wait(definition, (value) => value.executable === null);
		await wait(
			() => parameter('Window').count(),
			(count) => count === 0
		);
		await undo();
		await wait(definition, (value) => isDeepStrictEqual(value, expected));
		const runtime = (await query({ kind: 'InspectRuntime' })).snapshot;
		assert(runtime.applications.every((value) => value.process_id === null));
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m08-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled generic app-control Parameters, real Argument/Environment children, conditional visibility, clearing and Undo/Redo; no native picker or process launch',
				browser: browser.version(),
				application,
				definition: expected,
				screenshot: 'target/m08-applications-ui.png'
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m08-applications-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-7000)
			})
		);
		throw error;
	} finally {
		await browser.close();
	}
}
