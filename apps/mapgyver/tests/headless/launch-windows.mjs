// Check actual STARTUPINFO and visible state on windows owned by the finite Golden fixture.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyLaunchWindows({ edit, group = false, directory: targetDirectory }) {
	const directory = path.resolve(targetDirectory ?? process.env.MAPGYVER_REENTRY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Launch window defaults'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const apps = (operation) => edit({ kind: 'Applications', edit: operation });
	const [active, empty] = (await matrix({ kind: 'Inspect' })).snapshot.columns;
	let application = active.id;
	if (group) {
		const result = await matrix({ kind: 'AddColumnGroup', parent: null, label: 'Window group' });
		application = result.group;
		await matrix({
			kind: 'MoveColumnItem',
			item: { kind: 'Column', id: active.id },
			parent: application,
			after: null
		});
	}
	const initial = (await apps({ kind: 'Inspect', application })).snapshot;
	assert.equal(initial.definition.window, 'Normal');
	const results = [];
	for (const window of ['Normal', 'Hidden']) {
		const report = path.join(directory, `${application}-${window}.txt`);
		const definition = {
			...initial.definition,
			window,
			executable: path.resolve('target/debug/golden_io_process_fixture.exe'),
			arguments: ['launch-window', report, path.join(directory, `${application}-${window}.stop`)]
		};
		if (group) await apps({ kind: 'Configure', application, definition });
		else
			await matrix({
				kind: 'ConfigureColumnApplication',
				column: active.id,
				pre: 0,
				post: 0,
				definition
			});
		await matrix({ kind: 'LaunchColumn', column: active.id, mode: 'Configured' });
		const deadline = Date.now() + 8000;
		let record;
		while (!record) {
			const text = await readFile(report, 'utf8').catch(() => null);
			if (text)
				record = Object.fromEntries(
					text
						.trim()
						.split('\n')
						.map((line) => line.split('='))
				);
			assert(Date.now() < deadline, `${window} fixture window readiness`);
			if (!record) await delay(25);
		}
		assert.equal(record.visible, String(window === 'Normal'));
		assert.equal(Number(record.startup_show), window === 'Normal' ? 1 : 0);
		let live = (await apps({ kind: 'InspectRuntime' })).snapshot;
		while (
			!live.applications.some(
				(entry) =>
					entry.phase === 'Ready' &&
					entry.process_id === Number(record.pid) &&
					entry.consumers.some((consumer) => consumer.application === application)
			)
		) {
			assert(Date.now() < deadline, `${window} process readiness: ${JSON.stringify(live)}`);
			await delay(25);
			live = (await apps({ kind: 'InspectRuntime' })).snapshot;
		}
		await matrix({ kind: 'LaunchColumn', column: empty.id, mode: 'Configured' });
		while (true) {
			live = (await apps({ kind: 'InspectRuntime' })).snapshot;
			if (live.applications.every((entry) => entry.phase === 'Idle' && entry.process_id === null))
				break;
			assert(
				Date.now() < deadline,
				`${window} graceful window retirement: ${JSON.stringify(live)}`
			);
			await delay(25);
		}
		results.push({ window, ...record });
	}
	await writeFile(
		path.join(directory, 'windows.json'),
		JSON.stringify(
			{ result: 'pass', owner: group ? 'ColumnGroup' : 'Column', application, results },
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope: 'Actual Normal and explicit Hidden launch windows, zero pre/post, graceful retirement',
			results
		})
	);
}
