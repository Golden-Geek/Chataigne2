// Actual association/launch/availability controls with the finite owned executable. Queries only read.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { selectMediaChannel } from './media-tree.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
function alive(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		if (error.code === 'ESRCH') return false;
		throw error;
	}
}

export async function verifyColumnApplicationsUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [],
		pids = new Set(),
		matrices = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup',
		application;
	const rejected = observeRejectedRequests(page, () => stage);
	const appQuery = async (value) => edit({ kind: 'Applications', edit: value });
	const catalog = async () =>
		(await appQuery({ kind: 'InspectCatalog', search: '', start: 0, count: 128 })).page;
	const appSnapshot = async () => (await appQuery({ kind: 'Inspect', application })).snapshot;
	const runtime = async () => (await appQuery({ kind: 'InspectRuntime' })).snapshot;
	const inspect = async (media) =>
		(await edit({ kind: 'Matrix', media, edit: { kind: 'Inspect' } })).snapshot;
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const matrix = page.getByRole('region', { name: 'Matrix', exact: true });
	const apps = page.getByRole('region', { name: 'Applications', exact: true });
	const properties = page.locator('.properties');
	const actions = page.locator('section[aria-label="Matrix"] .editor-toolbar, .properties');
	const button = (name) => actions.getByRole('button', { name, exact: true });
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	async function selectMatrix(media) {
		const catalog = await inspectMedia();
		const snapshot = await inspect(media);
		await selectMediaChannel(page, catalog, snapshot.final_channel);
		await tab('Matrix').click();
		await wait(
			() => matrix.getByRole('grid').getAttribute('aria-colcount'),
			(value) => value === String(snapshot.columns.length + 1)
		);
		await wait(() => button('Add Matrix').isEnabled(), Boolean);
	}
	try {
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		async function restore(media, before, after, back = false) {
			await undo();
			await wait(
				() => inspect(media),
				(value) => isDeepStrictEqual(value, before)
			);
			await redo();
			await wait(
				() => inspect(media),
				(value) => isDeepStrictEqual(value, after)
			);
			if (back) {
				await undo();
				await wait(
					() => inspect(media),
					(value) => isDeepStrictEqual(value, before)
				);
			}
		}

		async function selectColumn(label) {
			const grid = matrix.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
			await grid.getByTitle(`Select ${label}`, { exact: true }).click();
			await wait(
				() => properties.getByLabel('Column name', { exact: true }).inputValue(),
				(value) => value === label
			);
		}
		async function createMatrix(count) {
			await tab('Matrix').click();
			const before = await inspectMedia();
			await button('Add Matrix').click();
			const current = await wait(inspectMedia, (value) =>
				value.media.some(
					(entry) =>
						entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
				)
			);
			const root = current.media.find(
				(entry) => entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
			);
			matrices.push(root.id);
			for (let index = 1; index <= count; index++) {
				await button('Add column').click();
				await wait(
					() => inspect(root.id),
					(value) => value.columns.length === index
				);
			}
			return await inspect(root.id);
		}
		async function associate(media, column, pre, post) {
			await selectMatrix(media);
			await selectColumn(column.label);
			const form = properties.getByRole('group', {
				name: 'Associate a shared application',
				exact: true
			});
			await form.getByRole('combobox', { name: /^Shared application/ }).selectOption(application);
			await form.getByLabel('Preload columns', { exact: true }).fill(String(pre));
			await form.getByLabel('Postload columns', { exact: true }).fill(String(post));
			const before = await inspect(media);
			await button('Add application association').click();
			const after = await wait(
				() => inspect(media),
				(value) =>
					value.columns.find((entry) => entry.id === column.id).definition.applications.length === 1
			);
			assert.deepEqual(
				after.columns.find((entry) => entry.id === column.id).definition.applications,
				[{ application, pre, post }]
			);
			return { before, after };
		}
		async function phase(wanted) {
			const value = await wait(
				runtime,
				(value) =>
					value.applications.find((entry) => entry.application === application)?.phase === wanted &&
					value.callbacks.fault === null &&
					value.callbacks.pending === 0
			);
			assert.equal(value.supervision.host, 'Headless');
			assert.equal(value.supervision.demand.kind, 'Running');
			assert.equal(value.supervision.callbacks.kind, 'Running');
			const current = value.applications.find((entry) => entry.application === application);
			if (current.process_id) pids.add(current.process_id);
			return current;
		}
		async function showRuntime(expected) {
			await selectApplication();
			const details = properties.locator('details');
			if ((await details.getAttribute('open')) === null) await details.locator('summary').click();
			const view = properties.getByRole('region', { name: 'Application runtime', exact: true });
			await view.getByRole('button', { name: 'Refresh runtime status', exact: true }).click();
			const article = view.getByRole('article', { name: `Runtime ${application}`, exact: true });
			await wait(
				() => article.locator('dd').nth(0).innerText(),
				(value) => value === expected.phase
			);
			if (expected.process_id)
				await article.getByText(String(expected.process_id), { exact: true }).waitFor();
			assert((await view.innerText()).includes('Snapshot observations; refresh to update'));
		}
		async function selectApplication() {
			await tab('Applications').click();
			await apps.getByRole('button', { name: 'UI Column Fixture', exact: true }).click();
			await wait(
				() => properties.getByLabel('Application name', { exact: true }).inputValue(),
				(value) => value === 'UI Column Fixture'
			);
		}
		async function launch(media, column) {
			await selectMatrix(media);
			await selectColumn(column.label);
			await button('Cut').click();
		}
		await page.goto(base);
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('New', { exact: true }) })
			.click();
		await wait(catalog, (value) => value.total === 0);
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		stage = 'minimal owned application definition';
		await tab('Applications').click();
		await apps.getByRole('button', { name: 'Add child item', exact: true }).click();
		application = (await wait(catalog, (value) => value.total === 1)).applications[0].id;
		await properties.getByLabel('Application name', { exact: true }).fill('UI Column Fixture');
		await properties.getByLabel('Application name', { exact: true }).press('Enter');
		await wait(appSnapshot, (value) => value.label === 'UI Column Fixture');
		await properties
			.getByLabel('Executable path', { exact: true })
			.fill(path.resolve('target/debug/application_process_fixture.exe'));
		await properties.getByLabel('Window').selectOption('Hidden');
		await properties.getByLabel('Ready when').selectOption('ProcessStarted');
		await properties.getByLabel('Graceful stop (ms)', { exact: true }).fill('30');
		await properties.getByLabel('Retirement timeout (ms)', { exact: true }).fill('1000');
		await properties.getByLabel('Restart after failure').uncheck();
		await properties
			.getByRole('button', { name: 'Save application definition', exact: true })
			.click();
		await wait(
			appSnapshot,
			(value) => value.definition.executable !== null && !value.definition.restart.enabled
		);
		stage = 'association distance and history';
		const first = await createMatrix(4),
			columns = first.columns;
		let { before, after } = await associate(first.media, columns[1], 1, 1);
		await restore(first.media, before, after);
		const settings = properties.getByRole('group', { name: 'Column applications', exact: true });
		await wait(
			() => settings.locator('.association').count(),
			(value) => value === 1
		);
		await settings.getByLabel('Preload columns', { exact: true }).fill('2');
		await settings.getByLabel('Postload columns', { exact: true }).fill('0');
		assert.deepEqual(await inspect(first.media), after, 'distance edits remain drafts');
		before = after;
		await button('Save column application distances').click();
		after = await wait(
			() => inspect(first.media),
			(value) => value.columns[1].definition.applications[0].pre === 2
		);
		assert.deepEqual(after, {
			...before,
			columns: before.columns.map((column, index) =>
				index === 1
					? {
							...column,
							definition: { ...column.definition, applications: [{ application, pre: 2, post: 0 }] }
						}
					: column
			)
		});
		await restore(first.media, before, after, true);
		before = await inspect(first.media);
		await button('Remove association').click();
		after = await wait(
			() => inspect(first.media),
			(value) => value.columns[1].definition.applications.length === 0
		);
		await restore(first.media, before, after, true);
		await button('Copy column').click();
		after = await wait(
			() => inspect(first.media),
			(value) => value.columns.length === 5
		);
		const copy = after.columns.find(
			(column) => !before.columns.some((old) => old.id === column.id)
		);
		assert.deepEqual(copy.definition.applications, [{ application, pre: 1, post: 1 }]);
		await restore(first.media, before, after, true);
		await associate(first.media, columns[2], 0, 0);
		const firstAuthored = await inspect(first.media);
		stage = 'missing definition retains saved column references';
		await selectApplication();
		await properties.getByRole('button', { name: 'Remove definition', exact: true }).click();
		await wait(catalog, (value) => value.total === 0);
		await selectMatrix(first.media);
		await selectColumn(columns[1].label);
		await wait(
			() => settings.innerText(),
			(value) => value.includes('Missing definition')
		);
		assert.deepEqual(await inspect(first.media), firstAuthored);
		await undo();
		await wait(catalog, (value) => value.total === 1);
		assert.equal((await appSnapshot()).id, application);
		stage = 'second Matrix shares the same application';
		const second = await createMatrix(1);
		await associate(second.media, second.columns[0], 0, 0);
		const secondAuthored = await inspect(second.media);
		assert.equal((await catalog()).total, 1);
		stage = 'actual preload postload and shared process holds';
		await launch(first.media, columns[0]);
		const initial = await phase('Ready');
		assert(initial.process_id && alive(initial.process_id));
		await showRuntime(initial);
		await launch(first.media, columns[2]);
		assert.equal((await phase('Ready')).process_id, initial.process_id);
		await launch(second.media, second.columns[0]);
		assert.equal((await phase('Ready')).process_id, initial.process_id);
		await launch(first.media, columns[3]);
		assert.equal(
			(await phase('Ready')).process_id,
			initial.process_id,
			'second Matrix retains the shared owner'
		);
		await selectMatrix(second.media);
		await button('Unload Matrix').click();
		const stopped = await phase('Idle');
		assert.equal(stopped.process_id, null);
		assert(!alive(initial.process_id));
		await showRuntime(stopped);
		stage = 'availability blocks only new launches and recovers';
		await launch(first.media, columns[0]);
		const running = await phase('Ready');
		assert.notEqual(running.launch, initial.launch);
		await selectApplication();
		await properties.getByLabel('Allow new launches', { exact: true }).uncheck();
		await wait(appSnapshot, (value) => value.enablement === 'Disabled');
		assert.equal((await phase('Ready')).process_id, running.process_id);
		await selectMatrix(first.media);
		await button('Unload Matrix').click();
		await phase('Idle');
		assert(!alive(running.process_id));
		await launch(first.media, columns[0]);
		await wait(runtime, (value) => value.applications.length === 0);
		// Unavailable neighbors are not preloaded; direct column demand must expose Unavailable.
		await launch(first.media, columns[1]);
		const unavailable = await phase('Unavailable');
		assert.equal(unavailable.process_id, null);
		await showRuntime(unavailable);
		await properties.getByLabel('Allow new launches', { exact: true }).check();
		await wait(appSnapshot, (value) => value.enablement === 'Enabled');
		const recovered = await phase('Ready');
		await showRuntime(recovered);
		assert.notEqual(recovered.launch, running.launch);
		assert.deepEqual(await inspect(first.media), firstAuthored);
		assert.deepEqual(await inspect(second.media), secondAuthored);
		stage = 'actual project New retires active owned process';
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('New', { exact: true }) })
			.click();
		await wait(runtime, (value) => value.applications.length === 0);
		await wait(catalog, (value) => value.total === 0);
		await wait(async () => [...pids].every((pid) => !alive(pid)), Boolean);
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		assert.deepEqual(rejected, []);
		assert.deepEqual(errors, []);
		await page.screenshot({ path: 'target/m08-column-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual column association/distance/history/missing-reference/shared-scope/preload/postload/availability/runtime-refresh/New input with owned processes; native dialog/elevation/singleton/full-app gaps remain',
				browser: browser.version(),
				application,
				matrices,
				owned_pids: [...pids],
				initial_launch: initial.launch,
				recovered_launch: recovered.launch,
				owned_pids_absent: [...pids].every((pid) => !alive(pid)),
				page_errors: errors.length,
				rejected_requests: rejected.length,
				screenshot: 'target/m08-column-applications-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				application,
				owned_pids: [...pids],
				rejected_requests: rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-4500)
			})
		);
		await page
			.screenshot({ path: 'target/m08-column-applications-ui-failure.png' })
			.catch(() => {});
		throw error;
	} finally {
		// Cleanup uses the owned Matrix controls; never adopt or terminate a process by its name.
		for (const media of matrices) {
			try {
				if (!(await inspectMedia()).media.some((entry) => entry.id === media)) continue;
				await selectMatrix(media);
				if (await button('Unload Matrix').isEnabled()) await button('Unload Matrix').click();
			} catch {}
		}
		await browser.close();
	}
}
