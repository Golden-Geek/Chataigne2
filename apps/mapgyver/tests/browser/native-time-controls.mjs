// Real shared clock editors; backend snapshots remain milliseconds and no application is launched.
import assert from 'node:assert/strict';
import path from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyNativeTimeControls({ page, edit, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_TIME_DIRECTORY);
	await mkdir(directory);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Clock parameters'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const layer = initial.items.find((item) => item.id.kind === 'Layer');
	const column = initial.columns[0];
	const prepared = await matrix({ kind: 'EnsureCell', layer: layer.id.id, column: column.id });
	const application = async () =>
		(
			await edit({
				kind: 'Applications',
				edit: { kind: 'Inspect', application: column.application }
			})
		).snapshot;
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const samples = [];
	async function wait(read, predicate) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	async function clock(label, text, read, expected) {
		const input = parameter(label).locator('input.time-field');
		await input.waitFor();
		assert.equal(await input.getAttribute('type'), 'text');
		await input.fill(text);
		await input.press('Enter');
		await wait(read, (value) => value === expected);
		assert.equal(await input.inputValue(), text);
		samples.push({ label, clock: text, backendMilliseconds: expected });
	}
	try {
		// This fixture uses a fresh profile: its first dock tab is the default media manager.
		await page.getByRole('tab').first().click();
		await page.getByRole('button', { name: 'Clock parameters', exact: true }).click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		await grid.getByTitle(`Select ${layer.label}`, { exact: true }).click();
		const layerDuration = async () =>
			(await inspect()).items.find((item) => item.id.id === layer.id.id).layer.duration_ms;
		await clock('Transition duration', '00:00:01.234', layerDuration, 1234);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(layerDuration, (value) => value === 250);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(layerDuration, (value) => value === 1234);
		await grid
			.getByTitle(`${layer.label} / ${column.label}`, { exact: true })
			.first()
			.click({ position: { x: 5, y: 5 } });
		await grid
			.getByRole('gridcell', { selected: true })
			.click({ button: 'right', position: { x: 5, y: 5 } });
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		await parameter('Override transition').locator('input[type=checkbox]').check();
		await clock(
			'Transition duration',
			'00:00:02.345',
			async () =>
				(await inspect()).cells.find((cell) => cell.id === prepared.cell).definition.transition
					?.duration_ms,
			2345
		);
		await grid.getByTitle(`Select ${column.label}`, { exact: true }).click();
		await parameter('Override transition').locator('input[type=checkbox]').check();
		await clock(
			'Transition duration',
			'00:00:03.456',
			async () => (await inspect()).columns[0].definition.transition?.duration_ms,
			3456
		);
		const executable = path.resolve('target/debug/application_process_fixture.exe');
		const file = parameter('App file').locator('input.path-input');
		await file.fill(executable);
		await file.press('Tab');
		await wait(application, (value) => value.definition.executable === executable);
		await parameter('Ready when').locator('select').selectOption('Spout');
		for (const [label, text, expected, read] of [
			[
				'Ready timeout',
				'00:00:04.321',
				4321,
				async () => (await application()).definition.readiness.timeout_ms
			],
			[
				'Graceful stop',
				'00:00:02.100',
				2100,
				async () => (await application()).definition.stop.graceful_ms
			],
			[
				'Retirement timeout',
				'00:00:08.000',
				8000,
				async () => (await application()).definition.stop.retirement_ms
			],
			[
				'Initial restart delay',
				'00:00:01.234',
				1234,
				async () => (await application()).definition.restart.initial_ms
			],
			[
				'Maximum restart delay',
				'00:00:09.012',
				9012,
				async () => (await application()).definition.restart.maximum_ms
			]
		])
			await clock(label, text, read, expected);
		const runtime = (await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } }))
			.snapshot;
		assert(runtime.applications.every((application) => application.process_id === null));
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		await page.screenshot({ path: path.join(directory, 'time-controls.png') });
		await writeFile(
			path.join(directory, 'time-controls.json'),
			JSON.stringify(
				{
					nativePid,
					samples,
					layerUndoRedo: true,
					applicationProcesses: 0,
					playbackUnchanged: true
				},
				null,
				2
			)
		);
		console.log(
			JSON.stringify({ nativePid, samples, layerUndoRedo: true, applicationProcesses: 0 })
		);
	} catch (error) {
		await writeFile(path.join(directory, 'failure.txt'), await page.locator('body').innerText());
		throw error;
	}
}
