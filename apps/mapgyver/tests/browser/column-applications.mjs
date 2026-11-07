// The generic Inspector authors Column settings; actual Matrix controls drive process demand.
import assert from 'node:assert/strict';
import { clearMatrixScene } from './matrix-scene.mjs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyColumnApplicationsUi({ base, edit }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Column app review'
	});

	await clearMatrixScene(edit, reference.media);
	const { items } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'AddLayers', parent: null, count: 1 }
	});
	const { columns } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'AddColumns', count: 5 }
	});
	const inspect = async () =>
		(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } })).snapshot;
	const application = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: columns[2] } }))
			.snapshot;
	const runtime = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } })).snapshot;
	async function wait(read, predicate) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Column app control');
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	async function selectCell(title, clickCount = 1, button = 'left') {
		const target = page
			.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
			.getByTitle(title, { exact: true });
		const bounds = await target.boundingBox();
		assert(bounds);
		await target.click({ position: { x: 8, y: bounds.height - 8 }, clickCount, button });
	}
	let pid = null,
		runtimeId = null,
		otherMedia = null;
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Column app review', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await selectCell('Layer 1 / Column 3');
		await grid.locator(`[role="columnheader"][data-column-id="${columns[2]}"]`).click();
		await parameter('App file').waitFor();
		const state = await inspect();
		assert.equal(state.columns[2].application, columns[2]);
		assert.equal(
			await page
				.getByRole('tab')
				.filter({ hasText: /^Applications$/ })
				.count(),
			0
		);
		assert.equal(await parameter('Window').count(), 0);
		const executable = path.resolve('target/debug/application_process_fixture.exe');
		const file = parameter('App file').locator('input.path-input');
		await file.fill(executable);
		await file.press('Tab');
		await wait(application, (s) => s.definition.executable === executable);
		assert.equal(await parameter('Window').locator('select').inputValue(), 'Normal');
		await parameter('Window').locator('select').selectOption('Hidden');
		await wait(application, (s) => s.definition.window === 'Hidden');
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(application, (s) => s.definition.window === 'Normal');
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(application, (s) => s.definition.window === 'Hidden');
		for (const [label, value] of [
			['Preload columns before', '1'],
			['Keep columns after', '1']
		]) {
			const input = parameter(label).locator('input.number-field');
			await input.fill(value);
			await input.press('Enter');
		}
		await parameter('Override transition').locator('input[type=checkbox]').check();
		const duration = parameter('Transition duration').locator('input.number-field');
		await duration.fill('00:00:01.234');
		await duration.press('Enter');
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 1234);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 250);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 1234);
		assert((await runtime()).applications.every((s) => s.process_id === null));
		const definition = (await application()).definition;
		const prepared = await edit({
			kind: 'Matrix',
			media: reference.media,
			edit: {
				kind: 'ConfigureCellApplication',
				layer: items[0].id,
				column: columns[2],
				definition,
				pre: 1,
				post: 1
			}
		});
		assert.notEqual(prepared.application, columns[2]);
		await selectCell('Layer 1 / Column 2');
		await grid.getByRole('button', { name: 'Launch Column 2', exact: true }).click();
		let live = await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.consumers.some((c) => c.application === columns[2]) && a.phase === 'Ready'
			)
		);
		const first = live.applications.find((a) =>
			a.consumers.some((c) => c.application === columns[2])
		);
		pid = first.process_id;
		runtimeId = first.runtime;
		assert(pid);
		assert.equal(first.consumers.length, 2, 'Column and Cell share one process');
		const { reference: other } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Shared runtime peer'
		});
		otherMedia = other.media;
		const { columns: peers } = await edit({
			kind: 'Matrix',
			media: otherMedia,
			edit: { kind: 'AddColumns', count: 1 }
		});
		await edit({
			kind: 'Matrix',
			media: otherMedia,
			edit: { kind: 'ConfigureColumnApplication', column: peers[0], definition, pre: 0, post: 0 }
		});
		await edit({
			kind: 'Matrix',
			media: otherMedia,
			edit: { kind: 'LaunchColumn', column: peers[0], mode: 'Cut' }
		});
		const shared = await wait(runtime, (s) =>
			s.applications.some((a) => a.runtime === runtimeId && a.consumers.length === 3)
		);
		assert.equal(shared.applications.length, 1);
		assert.equal(shared.applications[0].process_id, pid);
		assert.equal(shared.applications[0].launch, first.launch);
		await grid.locator(`[role="columnheader"][data-column-id="${columns[1]}"]`).click();
		assert.equal(await page.locator('.inspector').getByText('Runtime', { exact: true }).count(), 0);
		assert.equal(
			await page.getByRole('region', { name: 'Application runtime', exact: true }).count(),
			0
		);
		assert.equal(
			await grid.locator(`[role="columnheader"][data-column-id="${columns[1]}"].requested`).count(),
			1
		);
		assert.equal((await inspect()).columns[2].application, columns[2]);
		await selectCell('Layer 1 / Column 3');
		await grid.getByRole('button', { name: 'Launch Column 3', exact: true }).click();
		await wait(
			async () =>
				(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'InspectPlayback' } }))
					.snapshot,
			(s) => s.desired_column === columns[2]
		);
		await delay(100);
		live = await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.consumers.some((c) => c.application === columns[2]) && a.phase === 'Ready'
			)
		);
		assert.equal(
			live.applications.find((a) => a.consumers.some((c) => c.application === columns[2]))
				.process_id,
			pid
		);
		assert.equal(
			live.applications.find((a) => a.consumers.some((c) => c.application === columns[2])).launch,
			first.launch
		);
		await edit({ kind: 'Matrix', media: otherMedia, edit: { kind: 'ClearPlayback' } });
		await wait(runtime, (s) =>
			s.applications.some((a) => a.runtime === runtimeId && a.consumers.length === 2)
		);
		await selectCell('Layer 1 / Column 5');
		await grid.getByRole('button', { name: 'Launch Column 5', exact: true }).click();
		await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.runtime === runtimeId && a.phase === 'Idle' && a.process_id === null
			)
		);
		pid = null;
		await selectCell('Layer 1 / Column 3', 2);
		const direct = await wait(runtime, (s) =>
			s.applications.some(
				(a) =>
					a.phase === 'Ready' && a.consumers.some((c) => c.application === prepared.application)
			)
		);
		const active = direct.applications.find(
			(a) => a.phase === 'Ready' && a.consumers.some((c) => c.application === prepared.application)
		);
		runtimeId = active.runtime;
		pid = active.process_id;
		assert.equal(
			active.consumers.length,
			1,
			'Independent Cell does not move the anchor or launch the other Column control'
		);
		const playback = (
			await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'InspectPlayback' } })
		).snapshot;
		assert.equal(playback.desired_column, columns[4]);
		assert.equal(playback.layers[0].column, columns[2]);
		await wait(
			() => grid.getByTitle('Layer 1 / Column 3', { exact: true }).getAttribute('data-requested'),
			(value) => value === 'true'
		);
		await wait(
			() => grid.getByTitle('Layer 1 / Column 5', { exact: true }).getAttribute('data-requested'),
			(value) => value === 'false'
		);
		await grid.locator('[data-matrix-stop-all]').click();
		await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.runtime === runtimeId && a.phase === 'Idle' && a.process_id === null
			)
		);
		pid = null;
		assert.deepEqual(errors, []);
		await edit({
			kind: 'Matrix',
			media: reference.media,
			edit: {
				kind: 'ConfigureCellApplication',
				layer: items[0].id,
				column: columns[4],
				definition,
				pre: 0,
				post: 0
			}
		});
		await selectCell('Layer 1 / Column 5');
		await selectCell('Layer 1 / Column 5', 1, 'right');
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		await parameter('App file').locator('input.path-input').waitFor();
		const enabled = page
			.locator('.inspector')
			.getByRole('switch', { name: 'Toggle node enabled', exact: true })
			.first();
		await enabled.click();
		await wait(
			() => enabled.getAttribute('aria-checked'),
			(value) => value === 'false'
		);
		await grid.getByRole('button', { name: 'Launch Column 3', exact: true }).click();
		const prior = await wait(runtime, (s) =>
			s.applications.some((a) => a.phase === 'Ready' && a.consumers.length === 2)
		);
		const held = prior.applications.find((a) => a.phase === 'Ready' && a.consumers.length === 2);
		pid = held.process_id;
		runtimeId = held.runtime;
		await grid.getByRole('button', { name: 'Launch Column 5', exact: true }).click();
		await wait(
			async () =>
				(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'InspectPlayback' } }))
					.snapshot,
			(s) =>
				s.desired_column === columns[4] && s.layers.some((layer) => layer.column === columns[2])
		);
		const retained = await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.runtime === runtimeId && a.phase === 'Ready' && a.consumers.length === 2
			)
		);
		assert.equal(retained.applications.find((a) => a.runtime === runtimeId).process_id, pid);
		assert.equal(retained.applications.find((a) => a.runtime === runtimeId).launch, held.launch);
		await grid.locator('[data-matrix-stop-all]').click();
		await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.runtime === runtimeId && a.phase === 'Idle' && a.process_id === null
			)
		);
		pid = null;
		assert.deepEqual(rejected, []);
		assert.deepEqual(errors, []);
		await page.screenshot({ path: 'target/m08-column-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Shared Column/Cell runtime and generic Inspector; independent Cell/anchor/marker; disabled destination preserves held Column/Cell PID without a renderer; actual Unload retirement',
				application: columns[2],
				browser: browser.version()
			})
		);
	} catch (error) {
		await page
			.screenshot({ path: 'target/m08-column-applications-ui-failure.png' })
			.catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-6500)
			})
		);
		throw error;
	} finally {
		if (otherMedia)
			await edit({ kind: 'Matrix', media: otherMedia, edit: { kind: 'ClearPlayback' } }).catch(
				() => {}
			);
		await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'ClearPlayback' } }).catch(
			() => {}
		);
		await browser.close();
		if (pid) {
			await wait(runtime, (s) =>
				s.applications.every((a) => a.runtime !== runtimeId || a.process_id === null)
			).catch(() => {});
		}
	}
}
