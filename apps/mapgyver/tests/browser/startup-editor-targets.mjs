// One ordinary owned desktop or headless host; public project operations and real panel input.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { contentLock, persistedPanelState } from './content-lock.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const headless = process.argv.includes('--headless');
const host = headless
	? await openInitialScreenHeadless(directory, { frontend: true })
	: await openNativeEditor();
const browser = headless ? await chromium.launch({ headless: true }) : null;
const page = host.page ?? (await browser.newPage({ viewport: { width: 1600, height: 1000 } }));
page.setDefaultTimeout(7000);
let token,
	stage = 'startup';
const checks = [],
	errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.screens',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
async function newProject(previous) {
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	const deadline = Date.now() + 7000;
	for (;;) {
		const current = (await inspect()).screens;
		if (current.length === 1 && current[0].id !== previous) break;
		assert(Date.now() < deadline, `${stage}: New did not publish fresh identities`);
		await delay(20);
	}
	await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
}
const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
const panel = (id) => page.locator(`section.workbench-editor[data-panel-id="${id}"]`);
async function shown(id, screen, surface = null) {
	await tab(id).click();
	await page.waitForFunction(
		({ id, screen, surface }) => {
			const node = document.querySelector(`section.workbench-editor[data-panel-id="${id}"]`);
			return (
				node &&
				node.dataset.screenTarget === screen &&
				(surface === null || node.dataset.surfaceTarget === surface) &&
				(!screen || node.getAttribute('aria-busy') === 'false')
			);
		},
		{ id, screen, surface }
	);
	assert(await panel(id).isVisible());
	if (screen) {
		const type = await panel(id).getAttribute('data-panel-type');
		await panel(id)
			.getByRole('application', {
				name: type === 'screenWarp' ? 'Screen view' : 'Surface source view',
				exact: true
			})
			.waitFor();
	}
}
async function fresh() {
	const before = await snapshot();
	const values = await inspect();
	assert.equal(values.screens.length, 1);
	const screen = values.screens[0],
		reference = screen.surfaces[0].reference;
	for (const editor of ['Screen', 'Surface']) {
		const { page: result } = await edit({
			kind: 'InspectEditor',
			query: {
				editor,
				selected: null,
				document: null,
				locked: null,
				retained: null,
				surface: null
			}
		});
		assert.equal(result.screen.id, screen.id);
		if (editor === 'Surface') assert.deepEqual(result.surface.reference, reference);
	}
	await shown('screen-warp', screen.id);
	await shown('surface-source', screen.id, reference.surface);
	assert.deepEqual(await inspect(), values);
	const after = await snapshot();
	assert.equal(after.history.undo_len, 0);
	assert.deepEqual(after.history, before.history);
	checks.push({ stage, screen: screen.id, surface: reference.surface, history: after.history });
	return screen;
}
try {
	token = (await post('/api/ui/session', {})).token;
	if (headless) await page.goto(host.base);
	let initial = await fresh();
	// A locked Source and a hidden Screen both receive the new document's identities.
	await (await contentLock(panel('surface-source'))).click();
	for (let cycle = 0; cycle < 3; cycle++) {
		stage = `New ${cycle + 1}`;
		const old = initial.id;
		await newProject(old);
		initial = await fresh();
		assert.notEqual(initial.id, old);
		assert.equal(await panel('surface-source').getAttribute('data-content-locked'), 'true');
	}
	stage = 'late and reopened Source panels';
	for (let opened = 0; opened < 2; opened++) {
		const inspectorGroup = page
			.getByRole('tab')
			.filter({ hasText: /^Inspector(?::|$)/ })
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await inspectorGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Surface Source', { exact: true }) })
			.click();
		const id = await page
			.locator('.gc-dv-tab[data-panel-type="surfaceSource"]')
			.evaluateAll(
				(nodes) =>
					nodes.find((node) => node.dataset.contentPanelId !== 'surface-source')?.dataset
						.contentPanelId
			);
		assert(id);
		await shown(id, initial.id, initial.surfaces[0].reference.surface);
		assert.equal((await snapshot()).history.undo_len, 0);
		await tab(id).getByRole('button', { name: 'Close panel', exact: true }).click();
		await tab(id).waitFor({ state: 'detached' });
	}
	checks.push({ stage, twiceResolvedExactSeed: true });
	await (await contentLock(panel('surface-source'), true)).click();
	stage = 'explicit second Screen and Surface';
	const created = await edit({ kind: 'CreateScreen', label: 'Saved target' });
	await edit({ kind: 'RenameSurface', reference: created.surface, label: 'Saved Surface' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	const tree = page.locator('section[aria-label="Screens"]');
	await tree.getByRole('button', { name: 'Saved target', exact: true }).click();
	await shown('screen-warp', created.screen);
	// Select the exact existing Surface row; no synthetic authored selection edits.
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	const row = tree
		.locator('[data-tree-node-id]')
		.filter({ has: page.getByRole('button', { name: 'Saved target', exact: true }) });
	const surfaceButton = tree.getByRole('button', { name: 'Saved Surface', exact: true });
	if (!(await surfaceButton.isVisible())) await row.getByRole('button', { name: /Expand/ }).click();
	await surfaceButton.click();
	await shown('surface-source', created.screen, created.surface.surface);
	await (await contentLock(panel('surface-source'))).click();
	const saved = path.join(directory, 'targets.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: saved })).ok, true);
	const { page: bound } = await edit({
		kind: 'InspectEditor',
		query: {
			editor: 'Surface',
			selected: created.surface.surface,
			document: null,
			locked: null,
			retained: null,
			surface: null
		}
	});
	await persistedPanelState(page, 'surface-source', {
		document: bound.document,
		target: bound.target,
		locked: true
	});
	stage = 'loaded explicit targets and reload';
	assert.equal((await post('/api/ui/project-load', { path: saved })).path, saved);
	await shown('screen-warp', created.screen);
	await shown('surface-source', created.screen, created.surface.surface);
	await page.reload();
	await shown('screen-warp', created.screen);
	await shown('surface-source', created.screen, created.surface.surface);
	assert.equal(await panel('surface-source').getAttribute('data-content-locked'), 'true');
	checks.push({ stage, savedScreen: created.screen, savedSurface: created.surface.surface });
	stage = 'empty loaded project';
	for (const screen of (await inspect()).screens)
		await edit({ kind: 'RemoveScreen', screen: screen.id });
	const empty = path.join(directory, 'empty.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: empty })).ok, true);
	assert.equal((await post('/api/ui/project-load', { path: empty })).path, empty);
	await shown('screen-warp', '');
	await shown('surface-source', '', '');
	assert.equal((await inspect()).screens.length, 0);
	checks.push({ stage, noInventedTargets: true });
	stage = 'New after empty load';
	await newProject(null);
	await fresh();
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ status: 'pass', headless, pid: host.app.pid, checks }, null, 2)
	);
	console.log(
		JSON.stringify({ status: 'pass', headless, pid: host.app.pid, checks: checks.length })
	);
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, log: host.log() }, null, 2)
	);
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
