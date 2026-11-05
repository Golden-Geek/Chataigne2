// A real owned sender crashes after readiness, then must publish a fresh first frame after restart.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyApplicationCrashRecovery({ edit }) {
	const directory = path.resolve(process.env.MAPGYVER_REENTRY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Owned sender recovery'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const apps = (operation) => edit({ kind: 'Applications', edit: operation });
	const [active, empty] = (await matrix({ kind: 'Inspect' })).snapshot.columns;
	const { definition } = (await apps({ kind: 'Inspect', application: active.id })).snapshot;
	const sender = `MapGyverRecovery-${reference.media}`;
	await matrix({
		kind: 'ConfigureColumnApplication',
		column: active.id,
		pre: 0,
		post: 0,
		definition: {
			...definition,
			executable: path.resolve('target/debug/application_process_fixture.exe'),
			arguments: [sender],
			readiness: { kind: 'Spout', sender, first_frame: true, timeout_ms: 5000 }
		}
	});
	const observations = [];
	async function wait(phase, predicate) {
		const deadline = Date.now() + 10000;
		while (true) {
			const snapshot = (await apps({ kind: 'InspectRuntime' })).snapshot;
			assert.equal(snapshot.callbacks.fault, null);
			const entry = snapshot.applications.find((entry) =>
				entry.consumers.some((consumer) => consumer.application === active.id)
			);
			if (predicate(entry, snapshot)) {
				observations.push({ phase, snapshot });
				return entry;
			}
			assert(Date.now() < deadline, `${phase}: ${JSON.stringify(snapshot)}`);
			await delay(20);
		}
	}
	try {
		await matrix({ kind: 'LaunchColumn', column: active.id, mode: 'Configured' });
		const first = await wait('initial-first-frame', (entry) => entry?.phase === 'Ready');
		assert(first.process_id && first.launch);
		process.kill(first.process_id);
		await wait(
			'crashed-sender-retired',
			(entry) => entry?.phase === 'Backoff' && entry.process_id === null && entry.failures >= 1
		);
		const recovered = await wait(
			'fresh-first-frame',
			(entry) => entry?.phase === 'Ready' && entry.launch !== first.launch
		);
		assert(recovered.process_id && recovered.launch);
		assert.notEqual(recovered.process_id, first.process_id);
		assert.equal(recovered.runtime, first.runtime);
		await matrix({ kind: 'LaunchColumn', column: empty.id, mode: 'Configured' });
		await wait('unloaded', (_entry, snapshot) =>
			snapshot.applications.every((entry) => entry.phase === 'Idle' && entry.process_id === null)
		);
		const report = {
			result: 'pass',
			sender,
			window: definition.window,
			pre: 0,
			post: 0,
			firstPid: first.process_id,
			recoveredPid: recovered.process_id,
			firstLaunch: first.launch,
			recoveredLaunch: recovered.launch,
			observations,
			limits:
				'Owned sender crash removes its endpoint; actual first-frame readiness must recover on a fresh launch. No latency/cadence claim.'
		};
		await writeFile(path.join(directory, 'crash-recovery.json'), JSON.stringify(report, null, 2));
		console.log(
			JSON.stringify({
				result: report.result,
				scope: report.limits,
				firstPid: report.firstPid,
				recoveredPid: report.recoveredPid
			})
		);
	} finally {
		await matrix({ kind: 'LaunchColumn', column: empty.id, mode: 'Configured' });
	}
}
