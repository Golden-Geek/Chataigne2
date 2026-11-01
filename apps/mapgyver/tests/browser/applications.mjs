// Actual bundled browser inputs; public queries correlate committed state without mutating it.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyApplicationUi({ base, edit }) {
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
		page.setDefaultTimeout(5000);
		const errors = [];
		page.on('pageerror', (error) => errors.push(String(error)));
		const rejected = observeRejectedRequests(page, () => 'Application definition');
		const query = (value) => edit({ kind: 'Applications', edit: value });
		const catalog = async () =>
			(await query({ kind: 'InspectCatalog', search: '', start: 0, count: 128 })).page;
		const inspect = async (application) => (await query({ kind: 'Inspect', application })).snapshot;
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(
					Date.now() < deadline,
					`application UI did not commit expected state: ${JSON.stringify(value)}`
				);
				await delay(20);
			}
		}
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Applications$/ })
			.click();
		const panel = page.getByRole('region', { name: 'Applications', exact: true });
		const properties = page.locator('.properties');
		await panel.getByRole('button', { name: 'Add child item', exact: true }).click();
		const first = (await wait(catalog, (value) => value.total === 1)).applications[0].id;
		const initial = await inspect(first);
		assert.equal(
			await panel.locator('input:not([type=search]),select,fieldset').count(),
			0,
			'Applications is a filtered outliner'
		);
		await properties.getByLabel('Application name', { exact: true }).fill('UI Shared Application');
		await properties.getByLabel('Application name', { exact: true }).press('Enter');
		await wait(
			() => inspect(first),
			(value) => value.label === 'UI Shared Application'
		);
		assert.equal(
			await properties.getByRole('button', { name: 'Browse executable…', exact: true }).count(),
			0
		);
		const executable = path.resolve('target/debug/application_process_fixture.exe');
		await properties.getByLabel('Executable path', { exact: true }).fill(executable);
		await properties
			.getByLabel('Working directory', { exact: true })
			.fill(path.dirname(executable));
		for (const [index, value] of ['literal & value', 'two words'].entries()) {
			await properties.getByRole('button', { name: 'Add argument', exact: true }).click();
			await properties.getByLabel(`Argument ${index + 1}`, { exact: true }).fill(value);
		}
		await properties.getByLabel('Window').selectOption('Normal');
		await properties.getByLabel('Inherit the current environment').uncheck();
		await properties.getByRole('button', { name: 'Add environment entry', exact: true }).click();
		await properties.getByLabel('Environment name 1', { exact: true }).fill('MAPGYVER_UI');
		await properties.getByLabel('Environment value 1', { exact: true }).fill('literal = value');
		await properties.getByLabel('Ready when').selectOption('Spout');
		await properties.getByLabel('Spout sender name', { exact: true }).fill('UI Exact Sender');
		await properties.getByLabel('Readiness evidence').selectOption('image');
		await properties.getByLabel('Startup timeout (ms)', { exact: true }).fill('2500');
		await properties.getByLabel('Graceful stop (ms)', { exact: true }).fill('25');
		await properties.getByLabel('Retirement timeout (ms)', { exact: true }).fill('1000');
		await properties.getByLabel('Restart after failure').uncheck();
		await properties.getByLabel('Initial restart delay (ms)', { exact: true }).fill('750');
		await properties.getByLabel('Maximum restart delay (ms)', { exact: true }).fill('3000');
		const expected = {
			...initial.definition,
			executable,
			directory: path.dirname(executable),
			arguments: ['literal & value', 'two words'],
			window: 'Normal',
			environment: [['MAPGYVER_UI', 'literal = value']],
			readiness: { kind: 'Spout', sender: 'UI Exact Sender', first_frame: true, timeout_ms: 2500 },
			stop: { graceful_ms: 25, retirement_ms: 1000 },
			restart: { enabled: false, initial_ms: 750, maximum_ms: 3000 }
		};
		const save = () =>
			properties.getByRole('button', { name: 'Save application definition', exact: true }).click();
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		const definition = () => inspect(first).then((value) => value.definition);
		await save();
		await wait(definition, (value) => value.executable === executable);
		assert.deepEqual(await definition(), expected);
		await undo();
		await wait(definition, (value) => value.executable === null);
		assert.deepEqual(await definition(), initial.definition);
		await redo();
		await wait(definition, (value) => value.executable === executable);
		assert.deepEqual(await definition(), expected);
		await properties
			.getByRole('button', { name: 'Remove environment entry 1', exact: true })
			.click();
		await save();
		await wait(
			definition,
			(value) => Array.isArray(value.environment) && value.environment.length === 0
		);
		await properties.getByLabel('Inherit the current environment').check();
		await save();
		await wait(definition, (value) => value.environment === null);
		await undo();
		await wait(
			definition,
			(value) => Array.isArray(value.environment) && value.environment.length === 0
		);
		await undo();
		await wait(definition, (value) => value.environment?.length === 1);
		assert.deepEqual(await definition(), expected);
		await properties.getByRole('button', { name: 'Duplicate application', exact: true }).click();
		const copiedCatalog = await wait(catalog, (value) => value.total === 2);
		const copy = copiedCatalog.applications.find((value) => value.id !== first);
		assert(copy);
		assert.deepEqual((await inspect(copy.id)).definition, expected);
		await wait(
			() => properties.getByLabel('Application name', { exact: true }).inputValue(),
			(value) => value === copy.label
		);
		await page
			.getByRole('tab')
			.filter({ hasText: `Inspector: ${copy.label}` })
			.waitFor();
		await properties.getByLabel('Allow new launches', { exact: true }).uncheck();
		await wait(
			() => inspect(copy.id),
			(value) => value.enablement === 'Disabled'
		);
		assert.equal((await inspect(first)).enablement, 'Enabled');
		await undo();
		await wait(
			() => inspect(copy.id),
			(value) => value.enablement === 'Enabled'
		);
		await redo();
		await wait(
			() => inspect(copy.id),
			(value) => value.enablement === 'Disabled'
		);
		await properties.getByRole('button', { name: 'Remove definition', exact: true }).click();
		await wait(catalog, (value) => value.total === 1);
		await undo();
		await wait(catalog, (value) => value.total === 2);
		await panel.getByRole('button', { name: copy.label, exact: true }).click();
		await properties
			.getByLabel('Executable path', { exact: true })
			.fill(path.resolve('target/unsupported.cmd'));
		await save();
		await properties.getByRole('alert').filter({ hasText: '.exe' }).waitFor();
		assert.deepEqual((await inspect(copy.id)).definition, expected);
		await redo();
		await wait(catalog, (value) => value.total === 1);
		await undo();
		await wait(catalog, (value) => value.total === 2);
		const runtime = (await query({ kind: 'InspectRuntime' })).snapshot;
		assert(
			runtime.applications.every((value) => value.process_id === null),
			'form edits must not launch processes'
		);
		assert.deepEqual(errors, []);
		assert.equal(
			rejected.length,
			1,
			`Only the unsupported executable definition rejects: ${JSON.stringify(rejected)}`
		);
		assert(
			rejected.every(
				(entry) =>
					entry.request?.intent?.payload?.kind === 'Applications' &&
					entry.request.intent.payload.edit?.kind === 'Configure'
			)
		);
		await panel.getByRole('button', { name: copy.label, exact: true }).click();
		await wait(
			() => properties.getByLabel('Application name', { exact: true }).inputValue(),
			(value) => value === copy.label
		);
		await page.screenshot({ path: 'target/m08-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual bundled application form/catalog/history input; no native dialogs or launched process',
				browser: browser.version(),
				first,
				copy: copy.id,
				definition: expected,
				page_errors: errors.length,
				expected_definition_rejections: rejected.length,
				screenshot: 'target/m08-applications-ui.png'
			})
		);
	} finally {
		await browser.close();
	}
}
