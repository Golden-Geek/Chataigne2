// Ordinary application project operations; uses only finite, explicitly owned process fixtures.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyProjectReplacement({
	post,
	applications,
	matrix,
	edit,
	fixture,
	alive,
	directory,
	external,
	shutdown
}) {
	const treeFixture = path.resolve('target/debug/golden_io_process_fixture.exe');
	const projectPath = path.join(directory, 'before-replacement.mapgyver');
	async function snapshot(predicate, demand = 'Running') {
		const deadline = Date.now() + 7000;
		while (true) {
			const { snapshot } = await applications({ kind: 'InspectRuntime' });
			assert.equal(snapshot.callbacks.fault, null);
			if (
				snapshot.supervision.host === 'Headless' &&
				snapshot.supervision.demand.kind === demand &&
				snapshot.supervision.callbacks.kind === 'Running' &&
				snapshot.callbacks.pending === 0 &&
				predicate(snapshot)
			)
				return snapshot;
			assert(Date.now() < deadline, `replacement runtime timeout: ${JSON.stringify(snapshot)}`);
			await delay(25);
		}
	}
	const ready = (application) =>
		snapshot((snapshot) =>
			snapshot.applications.some(
				(entry) =>
					entry.consumers.some((consumer) => consumer.application === application) &&
					entry.phase === 'Ready'
			)
		);
	async function target(label, executable, argumentsValue) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label });
		const { columns } = await matrix(reference.media, { kind: 'AddColumns', count: 1 });
		const application = columns[0];
		const { snapshot } = await applications({ kind: 'Inspect', application });
		const definition = {
			...snapshot.definition,
			executable,
			arguments: argumentsValue,
			readiness: { kind: 'ProcessStarted' },
			stop: { graceful_ms: 30, retirement_ms: 1000 },
			restart: { ...snapshot.definition.restart, enabled: false }
		};
		await applications({ kind: 'Configure', application, definition });
		return { application, definition, media: reference.media, column: columns[0] };
	}
	async function launch(target) {
		await matrix(target.media, { kind: 'LaunchColumn', column: target.column, mode: 'Cut' });
		return ready(target.application);
	}
	async function treeIds(stem) {
		const deadline = Date.now() + 5000;
		while (true) {
			try {
				const [root, branch, leaf] = await Promise.all(
					['txt', 'branch', 'leaf'].map(async (extension) =>
						(await readFile(path.join(directory, `${stem}.${extension}`), 'utf8'))
							.trim()
							.split(/\s+/)
							.map(Number)
					)
				);
				if (
					root.length === 2 &&
					branch.length === 2 &&
					leaf.length === 1 &&
					[...root, ...branch, ...leaf].every((pid) => Number.isInteger(pid) && pid > 0)
				) {
					assert.equal(root[1], branch[0]);
					assert.equal(branch[1], leaf[0]);
					return [root[0], branch[0], leaf[0]];
				}
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
			assert(Date.now() < deadline, `owned tree report timeout: ${stem}`);
			await delay(10);
		}
	}
	async function retired(pids) {
		const deadline = Date.now() + 3000;
		while (pids.some(alive) && Date.now() < deadline) await delay(25);
		assert(
			pids.every((pid) => !alive(pid)),
			`known old process must terminate: ${pids}`
		);
		assert.equal(
			external.exitCode,
			null,
			'replacement must preserve the independent external target'
		);
	}
	async function rejectOldCallback(project) {
		await assert.rejects(
			() => applications({ kind: 'DispatchCallbacks', ticket: { project, batch: randomUUID() } }),
			/lifecycle callback belongs to a stale project/
		);
	}
	const initialTarget = await target('Replacement initial owner', fixture, []);
	const application = initialTarget.application;
	const initial = await launch(initialTarget);
	const prior = initial.applications.find((entry) =>
		entry.consumers.some((consumer) => consumer.application === application)
	);
	// Reject an entire over-capacity demand set before creating any of its process workers.
	const { reference: overflow } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Admission recovery'
	});
	const { columns: overflowColumns } = await matrix(overflow.media, {
		kind: 'AddColumns',
		count: 65
	});
	const { snapshot: initialSettings } = await applications({ kind: 'Inspect', application });
	for (const [index, column] of overflowColumns.entries()) {
		await matrix(overflow.media, {
			kind: 'ConfigureColumnApplication',
			column,
			pre: 65,
			post: 65,
			definition: {
				...initialSettings.definition,
				executable: path.join(directory, 'missing-admission.exe'),
				arguments: [String(index)]
			}
		});
	}
	await matrix(overflow.media, { kind: 'LaunchColumn', column: overflowColumns[0], mode: 'Cut' });
	const rejected = await snapshot(
		(state) => state.supervision.demand.message.includes('at most 64'),
		'Retrying'
	);
	assert.equal(
		rejected.applications.length,
		1,
		'failed aggregate admission preserves the existing owner set'
	);
	assert.equal(rejected.applications[0].process_id, prior.process_id);
	assert.equal(rejected.applications[0].launch, prior.launch);
	await matrix(overflow.media, { kind: 'ClearPlayback' });
	const recovered = await ready(application);
	assert.equal(recovered.applications[0].process_id, prior.process_id);
	assert.equal(recovered.applications[0].launch, prior.launch);
	await edit({ kind: 'RemoveMedia', media: overflow.media });
	const authored = (await applications({ kind: 'Inspect', application })).snapshot;
	const oldTree = await target('Replacement owned tree', treeFixture, [
		'tree',
		path.join(directory, 'old.txt'),
		path.join(directory, 'old.stop')
	]);
	await launch(oldTree);
	const oldPids = [prior.process_id, ...(await treeIds('old'))];
	assert(oldPids.every(alive));
	assert.equal((await post('/api/ui/project-save', { path: projectPath })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	const fresh = await snapshot(
		(snapshot) => snapshot.project !== initial.project && snapshot.applications.length === 0
	);
	assert.equal(fresh.callbacks.acknowledged, 0);
	await retired(oldPids);
	await rejectOldCallback(initial.project);
	assert.deepEqual(await applications({ kind: 'Inspect', application }), {
		kind: 'ApplicationMissing',
		application
	});
	assert.deepEqual((await edit({ kind: 'InspectManager' })).snapshot, { assets: [], media: [] });
	console.log(
		JSON.stringify({
			project_replacement_stage: 'new',
			old_project: initial.project,
			new_project: fresh.project,
			terminated_known_pids: oldPids,
			external: external.pid
		})
	);

	const interimTarget = await target('Interim project owner', fixture, []);
	const interim = await launch(interimTarget);
	const interimStatus = interim.applications.find((entry) =>
		entry.consumers.some((consumer) => consumer.application === interimTarget.application)
	);
	assert(interimStatus.process_id && interimStatus.launch);
	await post('/api/ui/project-load', { path: projectPath });
	const loaded = await snapshot(
		(snapshot) =>
			snapshot.project !== initial.project &&
			snapshot.project !== fresh.project &&
			snapshot.applications.length === 0
	);
	assert.equal(loaded.callbacks.acknowledged, 0, 'runtime callbacks are not saved project state');
	await retired([interimStatus.process_id]);
	await rejectOldCallback(initial.project);
	await rejectOldCallback(fresh.project);
	const restored = (await applications({ kind: 'Inspect', application })).snapshot;
	assert.deepEqual(restored.definition, authored.definition);
	assert.equal(restored.id, authored.id);
	assert.notEqual(restored.definition_instance, authored.definition_instance);
	console.log(
		JSON.stringify({
			project_replacement_stage: 'load',
			project: loaded.project,
			terminated_interim_pid: interimStatus.process_id,
			stable_application: restored.id,
			fresh_definition_instance: restored.definition_instance
		})
	);

	// The saved Matrix has typed owner settings, not an implicitly resumed OS process.
	const newReport = path.join(directory, 'shutdown.txt');
	await applications({
		kind: 'Configure',
		application: oldTree.application,
		definition: {
			...oldTree.definition,
			arguments: ['tree', newReport, path.join(directory, 'shutdown.stop')]
		}
	});
	const final = await launch(oldTree);
	const finalStatus = final.applications.find((entry) =>
		entry.consumers.some((consumer) => consumer.application === oldTree.application)
	);
	const shutdownPids = await treeIds('shutdown');
	assert.equal(finalStatus.process_id, shutdownPids[0]);
	assert.notEqual(finalStatus.launch, interimStatus.launch);
	assert.equal(final.project, loaded.project);
	assert(final.callbacks.acknowledged >= 2);
	assert(
		final.applications.every((entry) =>
			entry.consumers.every((consumer) => consumer.application !== interimTarget.application)
		)
	);
	await shutdown();
	await retired(shutdownPids);
	assert(!alive(prior.process_id));
	return {
		projects: [initial.project, fresh.project, loaded.project],
		replaced_pids: oldPids,
		interim_pid: interimStatus.process_id,
		shutdown_pids: shutdownPids,
		stale_callbacks_rejected: 3,
		fresh_callbacks: final.callbacks.acknowledged,
		admission_recovered: true,
		admission_message: rejected.supervision.demand.message,
		normal_shutdown: true,
		external_survived_shutdown: alive(external.pid)
	};
}
