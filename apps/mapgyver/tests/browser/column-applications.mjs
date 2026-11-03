// The generic Inspector authors Column settings; actual Matrix controls drive process demand.
import assert from 'node:assert/strict';
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
	await edit({
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
	let pid = null;
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('region', { name: 'Media Manager', exact: true })
			.getByRole('button', { name: 'Column app review', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await grid.getByTitle('Layer 1 / Column 3', { exact: true }).click();
		await page.getByRole('button', { name: 'Inspect Column settings', exact: true }).click();
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
		for (const [label, value] of [
			['Preload columns before', '1'],
			['Keep columns after', '1']
		]) {
			const input = parameter(label).locator('input.number-field');
			await input.fill(value);
			await input.press('Enter');
		}
		await parameter('Override transition').locator('input[type=checkbox]').check();
		const duration = parameter('Transition duration (ms)').locator('input.number-field');
		await duration.fill('1234');
		await duration.press('Enter');
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 1234);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 250);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (s) => s.columns[2].definition.transition?.duration_ms === 1234);
		assert((await runtime()).applications.every((s) => s.process_id === null));
		await grid.getByTitle('Layer 1 / Column 2', { exact: true }).click();
		await page.getByRole('button', { name: 'Cut', exact: true }).click();
		let live = await wait(runtime, (s) =>
			s.applications.some((a) => a.application === columns[2] && a.phase === 'Ready')
		);
		const first = live.applications.find((a) => a.application === columns[2]);
		pid = first.process_id;
		assert(pid);
		assert.equal((await inspect()).columns[2].application, columns[2]);
		await grid.getByTitle('Layer 1 / Column 3', { exact: true }).click();
		await page.getByRole('button', { name: 'Cut', exact: true }).click();
		await wait(
			async () =>
				(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'InspectPlayback' } }))
					.snapshot,
			(s) => s.desired_column === columns[2]
		);
		await delay(100);
		live = await wait(runtime, (s) =>
			s.applications.some((a) => a.application === columns[2] && a.phase === 'Ready')
		);
		assert.equal(live.applications.find((a) => a.application === columns[2]).process_id, pid);
		assert.equal(live.applications.find((a) => a.application === columns[2]).launch, first.launch);
		await grid.getByTitle('Layer 1 / Column 5', { exact: true }).click();
		await page.getByRole('button', { name: 'Cut', exact: true }).click();
		await wait(runtime, (s) =>
			s.applications.some(
				(a) => a.application === columns[2] && a.phase === 'Idle' && a.process_id === null
			)
		);
		pid = null;
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m08-column-applications-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Column-owned generic File/pre/post/transition, exact Undo/Redo, no Applications panel, actual neighbor/exact same-PID launch and final range retirement',
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
		await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'ClearPlayback' } }).catch(
			() => {}
		);
		await browser.close();
		if (pid) {
			await wait(runtime, (s) =>
				s.applications.every((a) => a.application !== columns[2] || a.process_id === null)
			).catch(() => {});
		}
	}
}
