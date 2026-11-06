// Real SDK discovery and the bundled generic Inspector, without native desktop input.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { source } from './native-sender-selection.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifySenderDefaultsUi({ base, edit, post, directory, hostPid }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(7000);
	const errors = [],
		sources = [],
		checks = [],
		creationCases = [];
	let result;
	page.on('pageerror', (error) => errors.push(String(error)));
	const manager = page.locator('section[data-panel-id="media-manager"]');
	const mode = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Sender selection', { exact: true }) })
		.locator('select');
	const sender = page
		.locator(
			'.channels-section > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
		)
		.first()
		.locator('select.dropdown-editor');
	async function wait(read, accept, label) {
		const deadline = Date.now() + 10000;
		for (;;) {
			const value = await read();
			if (accept(value)) return value;
			assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
			await delay(30);
		}
	}
	async function select(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await manager.getByRole('button', { name: label, exact: true }).click();
		await page
			.locator('.inspector-header .title-text')
			.filter({ hasText: new RegExp(`^${RegExp.escape(label)}$`) })
			.waitFor();
		await mode.waitFor();
		await sender.waitFor();
		await wait(
			options,
			(items) => items.length > 0,
			'Selected Inspector options must be installed'
		);
	}
	const options = () =>
		sender.locator('option').evaluateAll((options) =>
			options.map((option) => ({
				value: option.value,
				label: option.textContent
			}))
		);
	const selected = async (reference) =>
		(await edit({ kind: 'InspectChannel', reference })).snapshot.definition.source.sender;
	const state = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	try {
		await page.goto(base);
		const { reference: observer } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SharedTexture',
			label: 'Catalog witness'
		});
		await select('Catalog witness');
		assert.equal(await mode.inputValue(), 'Auto');
		const a = await source(0, `MapGyver-M131-A-${process.pid}`);
		sources.push(a);
		const b = await source(0, `MapGyver-M131-B-${process.pid}`);
		sources.push(b);
		await a.command('start 64 64 1 0 0');
		await b.command('start 64 64 0 0 1');
		const catalog = await wait(
			options,
			(items) =>
				items.some((item) => item.value === a.name) && items.some((item) => item.value === b.name),
			'Both owned SDK senders must be discovered'
		);
		assert.equal(catalog[0].value, '');
		assert.equal(catalog[0].label, 'None');
		const expected = catalog.find((item) => item.value !== '').value;
		const { reference: probe } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SharedTexture',
			label: 'Auto creation probe'
		});
		assert.equal(
			await selected(probe),
			expected,
			'Creation itself must select the first displayed sender'
		);
		assert.equal(
			(await state()).media.find((media) => media.id === probe.media).sender_selection,
			'Auto'
		);
		await select('Auto creation probe');
		assert.equal(await sender.inputValue(), expected);
		assert.deepEqual(
			await options(),
			catalog,
			'Creation seeds the same options without a later UI write'
		);
		creationCases.push({
			senders: catalog.length - 1,
			reference: probe,
			selected: expected,
			mode: await mode.inputValue(),
			options: await options()
		});
		checks.push(
			'Actual SDK ordering, backend creation selection and the initial generic Auto/None controls agree'
		);
		await sender.selectOption('');
		await wait(
			() => selected(probe),
			(value) => value === null,
			'Explicit None must commit'
		);
		await b.command('stop');
		await wait(
			options,
			(items) => !items.some((item) => item.value === b.name),
			'Stopped unselected sender must disappear'
		);
		assert.equal(await selected(probe), null);
		await a.command('stop');
		await wait(
			options,
			(items) => items.length === 1 && items[0].value === '',
			'The controlled SDK catalog must reach zero senders'
		);
		const { reference: zero } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SharedTexture',
			label: 'No sender probe'
		});
		assert.equal(await selected(zero), null);
		await select('No sender probe');
		assert.equal(await mode.inputValue(), 'Auto');
		assert.deepEqual(await options(), [{ value: '', label: 'None' }]);
		creationCases.push({
			senders: 0,
			reference: zero,
			selected: await selected(zero),
			mode: await mode.inputValue(),
			options: await options()
		});
		await a.command('start 64 64 1 0 0');
		await wait(
			options,
			(items) => items.length === 2 && items[1].value === a.name,
			'The controlled SDK catalog must reach one sender'
		);
		const { reference: one } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SharedTexture',
			label: 'One sender probe'
		});
		assert.equal(await selected(one), a.name);
		await select('One sender probe');
		assert.equal(await mode.inputValue(), 'Auto');
		assert.equal(await sender.inputValue(), a.name);
		creationCases.push({
			senders: 1,
			reference: one,
			selected: await selected(one),
			mode: await mode.inputValue(),
			options: await options()
		});
		checks.push(
			'Fresh Auto creation with actual zero, one and multiple SDK senders selects None or the first entry atomically'
		);
		await b.command('start 64 64 0 0 1');
		await select('Auto creation probe');
		await wait(
			options,
			(items) => items.some((item) => item.value === b.name),
			'Restarted sender must return'
		);
		assert.equal(await selected(probe), null);
		checks.push('Explicit None survives real discovery disappearance and reappearance');
		await sender.selectOption(a.name);
		await wait(
			() => selected(probe),
			(value) => value === a.name,
			'Exact sender must commit'
		);
		await a.command('stop');
		await wait(
			() => sender.locator('option:checked').textContent(),
			(label) => label.includes('(unavailable)'),
			'Missing exact selection stays authored'
		);
		assert.equal(await selected(probe), a.name);
		await a.command('start 64 64 1 0 0');
		await wait(
			() => sender.locator('option:checked').textContent(),
			(label) => label === a.name,
			'Exact sender recovery must clear unavailable metadata'
		);
		assert.equal(await selected(probe), a.name);
		checks.push(
			'Exact selection survives owned SDK stop/restart with truthful unavailable/recovered metadata'
		);
		await mode.selectOption('Manual');
		await wait(
			() => page.locator('.channels-section input.string-editor').first().inputValue(),
			(value) => value === a.name,
			'Mode switch keeps the current exact sender'
		);
		await mode.selectOption('Auto');
		await wait(
			() => sender.inputValue(),
			(value) => value === a.name,
			'Auto mode keeps the same sender'
		);
		assert((await post('/api/ui/intent', { kind: 'undo' })).success);
		await wait(
			() => mode.inputValue(),
			(value) => value === 'Manual',
			'Undo restores Manual'
		);
		assert((await post('/api/ui/intent', { kind: 'redo' })).success);
		await wait(
			() => mode.inputValue(),
			(value) => value === 'Auto',
			'Redo restores Auto'
		);
		assert.equal(await selected(probe), a.name);
		checks.push('Mode changes and grouped Undo/Redo preserve the current exact sender');
		const before = await state();
		const project = path.join(directory, 'sender-defaults.mapgyver');
		assert((await post('/api/ui/project-save', { path: project })).ok);
		assert((await post('/api/ui/project-new', {})).ok);
		await post('/api/ui/project-load', { path: project });
		assert.deepEqual(await state(), before);
		await select('Auto creation probe');
		await wait(
			() => sender.inputValue(),
			(value) => value === a.name,
			'Reload must retain the exact sender'
		);
		assert.equal(await mode.inputValue(), 'Auto');
		assert.equal((await options()).find((item) => item.value === '').label, 'None');
		checks.push(
			'Save/New/Load restores authored mode/sender without applying creation policy again'
		);
		assert.deepEqual(errors, []);
		result = {
			result: 'pass',
			host_pid: hostPid,
			scope:
				'Ordinary headless host, actual SDK discovery and headless Chromium generic Inspector; no native desktop input, rendering-pixel or 240Hz claim',
			observer,
			probe,
			catalog,
			creation_cases: creationCases,
			checks,
			sources: sources.map(({ pid, name }) => ({ pid, name })),
			errors
		};
	} finally {
		const cleanup = await Promise.allSettled([
			browser.close(),
			...sources.map((child) => child.close())
		]);
		for (const outcome of cleanup) if (outcome.status === 'rejected') throw outcome.reason;
	}
	await writeFile(path.join(directory, 'sender-defaults.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify({ ...result, directory }));
}
