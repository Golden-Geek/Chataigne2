import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { verifyLaunchWindows } from '../headless/launch-windows.mjs';
import { inventory, source } from './native-sender-selection.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const run = promisify(execFile);

export async function verifyColumnGroupRuntime({ page, edit, post, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_GROUP_RUNTIME_DIRECTORY);
	await mkdir(directory, { recursive: true });
	await verifyLaunchWindows({ edit, group: true, directory: path.join(directory, 'windows') });
	await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Group runtime adapter' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Group runtime adapter', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview(?::|$)/ })
		.click();
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const receipts = await observeNativeRejectedRequests(page, () => 'Column Group runtime');
	const { reference: input } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Group endpoint'
	});
	const sender = `MapGyverGroup-${input.media}`;
	await edit({ kind: 'ConfigureChannel', reference: input, source: { kind: 'Spout', sender } });
	const adapter = await observeNativeAdapter(page, await inventory());
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Group output review'
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
	const apps = (value) => edit({ kind: 'Applications', edit: value });
	const authored = (await matrix({ kind: 'Inspect' })).snapshot;
	const [a, b, outside] = authored.columns;
	const row = authored.items.find((item) => item.id.kind === 'Layer');
	const { group: outer } = await matrix({ kind: 'AddColumnGroup', parent: null, label: 'Show' });
	const { group: inner } = await matrix({ kind: 'AddColumnGroup', parent: outer, label: 'Scenes' });
	for (const [column, after] of [
		[a.id, null],
		[b.id, { kind: 'Column', id: a.id }]
	])
		await matrix({
			kind: 'MoveColumnItem',
			item: { kind: 'Column', id: column },
			parent: inner,
			after
		});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 96, height: 64 } }
	});
	await matrix({
		kind: 'ConfigureLayer',
		layer: row.id.id,
		settings: { ...row.layer, size: { width: 96, height: 64 } }
	});
	await matrix({
		kind: 'SetCells',
		assignments: [a, b].map((column) => ({ layer: row.id.id, column: column.id, source: input }))
	});
	const initial = (await apps({ kind: 'Inspect', application: outer })).snapshot;
	const definition = {
		...initial.definition,
		executable: path.resolve('target/debug/application_process_fixture.exe'),
		readiness: { kind: 'Spout', sender, first_frame: true, timeout_ms: 5000 },
		stop: { graceful_ms: 1500, retirement_ms: 5000 }
	};
	for (const application of [outer, inner])
		await apps({ kind: 'Configure', application, definition });
	const owner = async () =>
		(await apps({ kind: 'InspectRuntime' })).snapshot.applications.find((status) =>
			status.consumers.some((consumer) => consumer.application === outer)
		);
	async function wait(read, check) {
		const deadline = Date.now() + 10000;
		while (true) {
			const value = await read();
			if (check(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Group output review', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const band = (id) => grid.locator(`[data-column-group-id="${id}"]`);
	const launch = async (column) =>
		grid
			.locator(`[data-matrix-drop="Column"][data-column-id="${column.id}"]`)
			.getByRole('button', { name: `Launch ${column.label}`, exact: true })
			.click();
	const flag = (id, name) => band(id).getAttribute(`data-group-${name}`);
	const endpoint = await source(adapter.index, sender);
	try {
		await endpoint.command('start 96 64 1 0 0');
		const producerDirectory = path.join(directory, 'producer');
		const { stdout } = await run(
			path.resolve('target/debug/native_qualification.exe'),
			[
				'--role',
				'pixels',
				'--adapter',
				String(adapter.index),
				'--name',
				sender,
				'--directory',
				producerDirectory
			],
			{ windowsHide: true, timeout: 15000 }
		);
		assert.deepEqual(JSON.parse(stdout), { width: 96, height: 64, retired: true });
		const { stdout: colors } = await run(
			'python',
			[
				'-c',
				'import json,sys; from PIL import Image; print(json.dumps(sorted(set(Image.open(sys.argv[1]).convert("RGBA").getdata()))))',
				path.join(producerDirectory, 'pixels.png')
			],
			{ windowsHide: true }
		);
		assert.deepEqual(
			JSON.parse(colors),
			[[255, 0, 0, 255]],
			'independent producer image before readiness'
		);
		await launch(a);
		await wait(
			() => flag(outer, 'requested'),
			(value) => value === 'true'
		);
		const first = await wait(owner, (status) => status?.phase === 'Ready');
		assert.deepEqual(
			new Set(first.consumers.map((consumer) => consumer.application)),
			new Set([outer, inner])
		);
		await output.demand(reference);
		for (const group of [outer, inner])
			await wait(
				() => flag(group, 'submitted'),
				(value) => value === 'true'
			);
		await delay(250);
		await output.observePixels('group-a-red', [255, 0, 0, 255]);
		await launch(b);
		await wait(
			async () => (await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column,
			(column) => column === b.id
		);
		await wait(
			() =>
				grid.locator(`[data-matrix-drop="Column"][data-column-id="${b.id}"]`).getAttribute('class'),
			(value) => value.includes('submitted')
		);
		const switched = await wait(owner, (status) => status?.phase === 'Ready');
		assert.equal(switched.process_id, first.process_id);
		assert.equal(switched.launch, first.launch);
		await endpoint.command('stop');
		const absent = await owner();
		assert.equal(absent.process_id, first.process_id);
		await endpoint.command('start 96 64 0 1 0');
		await delay(300);
		await output.observePixels('group-b-recovered-green', [0, 255, 0, 255]);
		const recovered = await owner();
		assert.equal(recovered.process_id, first.process_id);
		assert.equal(recovered.launch, first.launch);
		await page.screenshot({ path: path.join(directory, 'submitted.png') });
		await launch(outside);
		await wait(
			() => flag(outer, 'requested'),
			(value) => value === 'false'
		);
		for (const group of [outer, inner])
			await wait(
				() => flag(group, 'stopping'),
				(value) => value === 'true'
			);
		const stopping = await owner();
		assert.equal(stopping.phase, 'Stopping');
		assert(stopping.consumers.every((consumer) => consumer.configuration === 'Retained'));
		await page.screenshot({ path: path.join(directory, 'stopping.png') });
		await wait(
			() => flag(outer, 'submitted'),
			(value) => value === 'false'
		);
		await delay(250);
		await output.observePixels('outside-transparent', [0, 0, 0, 0]);
		await wait(
			async () => (await apps({ kind: 'InspectRuntime' })).snapshot,
			(snapshot) =>
				snapshot.applications.every(
					(status) => status.phase === 'Idle' && status.process_id === null
				)
		);
		await wait(
			() => flag(outer, 'stopping'),
			(value) => value === 'false'
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(receipts.rejected, []);
		const report = {
			result: 'pass',
			nativePid,
			adapter,
			outer,
			inner,
			first,
			switched,
			absent,
			recovered,
			stopping,
			pixels: output.observations,
			transport: receipts.observed,
			page_errors: errors,
			limits:
				'Actual group Normal/Hidden window visibility, native UI nested shared app first-frame readiness, same-group PID continuity, endpoint recreation and independent full SDK output pixels; requested/submitted/stopping markers remain separate. No presentation latency or cadence claim.'
		};
		await writeFile(path.join(directory, 'runtime.json'), JSON.stringify(report, null, 2));
		console.log(
			JSON.stringify({
				result: 'pass',
				nativePid,
				adapter,
				pid: first.process_id,
				pixels: output.observations
			})
		);
	} finally {
		try {
			await matrix({ kind: 'ClearPlayback' });
		} finally {
			try {
				await output.finish();
			} finally {
				await endpoint.close();
			}
		}
	}
}
