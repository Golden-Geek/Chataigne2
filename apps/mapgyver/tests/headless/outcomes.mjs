// Finite test-owned targets exercised through the ordinary app's public operations.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtemp, readFile, readdir, rmdir, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyTargetOutcomes({ applications, matrix, edit, fixture, alive }) {
	const directory = await mkdtemp(path.join(tmpdir(), 'mapgyver-target-outcomes-'));
	const file = (name) => path.join(directory, name);
	let singleton;
	let singletonExit;
	const results = {};
	async function json(report) {
		const deadline = Date.now() + 5000;
		while (true) {
			try {
				return JSON.parse(await readFile(report, 'utf8'));
			} catch (error) {
				if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error;
			}
			assert(Date.now() < deadline, `fixture report timeout: ${report}`);
			await delay(10);
		}
	}
	async function observe(application, predicate, forbidReady = false) {
		const deadline = Date.now() + 5000;
		while (true) {
			const { snapshot } = await applications({ kind: 'InspectRuntime' });
			assert.equal(snapshot.callbacks.fault, null);
			assert.equal(snapshot.supervision.demand.kind, 'Running');
			const status = snapshot.applications.find((entry) => entry.application === application);
			if (status) {
				assert.notEqual(
					status.process_id,
					singleton?.pid,
					'external singleton must never be adopted'
				);
				if (forbidReady)
					assert.notEqual(status.phase, 'Ready', 'root resume is not sender readiness');
				if (predicate(status) && snapshot.callbacks.pending === 0) return status;
			}
			assert(Date.now() < deadline, `target outcome timeout: ${JSON.stringify(snapshot)}`);
			await delay(10);
		}
	}
	try {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Target outcomes'
		});
		const { columns } = await matrix(reference.media, { kind: 'AddColumns', count: 1 });
		async function target(
			label,
			argumentsValue,
			readiness = { kind: 'ProcessStarted' },
			executable = fixture
		) {
			const { application } = await applications({ kind: 'Create', label });
			const { snapshot } = await applications({ kind: 'Inspect', application });
			const definition = {
				...snapshot.definition,
				executable,
				arguments: argumentsValue,
				readiness,
				stop: { graceful_ms: 30, retirement_ms: 1000 },
				restart: { ...snapshot.definition.restart, enabled: false }
			};
			await applications({ kind: 'Configure', application, definition });
			await matrix(reference.media, {
				kind: 'ConfigureColumn',
				column: columns[0],
				definition: { transition: null, applications: [{ application, pre: 0, post: 0 }] }
			});
			await matrix(reference.media, { kind: 'LaunchColumn', column: columns[0], mode: 'Cut' });
			return { application, definition };
		}
		async function unload(application) {
			await matrix(reference.media, { kind: 'ClearPlayback' });
			const status = await observe(application, (status) =>
				['Idle', 'RetirementUnverified'].includes(status.phase)
			);
			// An explicit observation gap is not complete retirement; retain it in the result.
			if (status.process_id !== null) assert(!alive(status.process_id));
			if (singleton)
				assert.equal(singleton.exitCode, null, 'owned unload must preserve the singleton');
			return status.phase;
		}
		const absentSender = {
			kind: 'Spout',
			sender: `MapGyver-absent-${randomUUID()}`,
			first_frame: true,
			timeout_ms: 4000
		};
		const name = `Local\\MapGyver-singleton-${randomUUID()}`;
		singleton = spawn(fixture, ['--lifecycle-singleton', name, file('external.json')], {
			windowsHide: true,
			stdio: 'ignore'
		});
		singletonExit = new Promise((resolve, reject) => {
			singleton.once('error', reject);
			singleton.once('close', resolve);
		});
		const external = await json(file('external.json'));
		assert.equal(external.pid, singleton.pid);
		assert.equal(external.already_exists, false);
		const duplicate = await target(
			'Singleton duplicate',
			['--lifecycle-singleton', name, file('duplicate.json')],
			absentSender
		);
		const duplicateExit = await observe(
			duplicate.application,
			(status) => ['Failed', 'RetirementUnverified'].includes(status.phase),
			true
		);
		assert.equal(duplicateExit.exit_code, 17);
		if (duplicateExit.process_id !== null) assert(!alive(duplicateExit.process_id));
		assert.equal(duplicateExit.next_action_ms, null);
		assert.equal((await json(file('duplicate.json'))).already_exists, true);
		const duplicateRetirement = await unload(duplicate.application);
		results.singleton = {
			external: singleton.pid,
			exit_code: duplicateExit.exit_code,
			adopted: false,
			phase: duplicateExit.phase,
			retirement: duplicateRetirement
		};

		const launcher = await target(
			'Launcher root exit',
			['--lifecycle-launcher', file('launcher.json')],
			absentSender
		);
		const tree = await json(file('launcher.json'));
		const rootExit = await observe(
			launcher.application,
			(status) => ['Failed', 'RetirementUnverified'].includes(status.phase),
			true
		);
		assert.equal(rootExit.exit_code, 0);
		assert(rootExit.message.includes('owned root exited'));
		const retirementDeadline = Date.now() + 3000;
		while ((alive(tree.root) || alive(tree.child)) && Date.now() < retirementDeadline)
			await delay(10);
		assert(
			!alive(tree.root) && !alive(tree.child),
			'launcher exit must retire its known owned descendant'
		);
		results.launcher = {
			...tree,
			phase: rootExit.phase,
			exit_code: rootExit.exit_code,
			retirement: await unload(launcher.application)
		};

		const breakaway = await target('Denied breakaway', [
			'--lifecycle-breakaway',
			file('breakaway.json')
		]);
		const owned = await observe(breakaway.application, (status) => status.phase === 'Ready');
		const attempt = await json(file('breakaway.json'));
		assert.equal(attempt.pid, owned.process_id);
		assert.equal(attempt.breakaway_error, 5, 'owned job must deny CREATE_BREAKAWAY_FROM_JOB');
		assert(!(await readdir(directory)).includes('breakaway.escaped'));
		const breakawayRetirement = await unload(breakaway.application);
		assert(!alive(owned.process_id));
		results.breakaway = { ...attempt, retirement: breakawayRetirement };

		await writeFile(file('invalid.exe'), 'MapGyver test data, not an executable');
		for (const [name, admittedErrors] of [
			['missing.exe', [2]],
			// Windows can classify an invalid native image as bad format or machine incompatibility.
			['invalid.exe', [193, 216]]
		]) {
			const invalid = await target(name, [], { kind: 'ProcessStarted' }, file(name));
			const failed = await observe(invalid.application, (status) => status.phase === 'Failed');
			assert.equal(failed.process_id, null);
			assert.equal(failed.exit_code, null);
			assert.equal(failed.next_action_ms, null);
			const error = Number(failed.message.match(/\(os error (\d+)\)/)?.[1]);
			assert(admittedErrors.includes(error), failed.message);
			const authored = (await applications({ kind: 'Inspect', application: invalid.application }))
				.snapshot;
			await assert.rejects(
				() =>
					applications({
						kind: 'Configure',
						application: invalid.application,
						definition: { ...invalid.definition, executable: file('unsupported.cmd') }
					}),
				/\.exe/
			);
			assert.deepEqual(
				(await applications({ kind: 'Inspect', application: invalid.application })).snapshot,
				authored
			);
			await delay(100);
			const stable = await observe(invalid.application, (status) => status.phase === 'Failed');
			assert.equal(stable.launch, failed.launch);
			assert.equal(stable.failures, 1);
			await unload(invalid.application);
			results[name] = { error, launch: failed.launch };
		}
		assert.equal(singleton.exitCode, null);
		return {
			...results,
			complete_retirement_verified: ['singleton', 'launcher', 'breakaway'].every(
				(name) => results[name].retirement === 'Idle'
			),
			elevated_target_verified: false
		};
	} catch (error) {
		console.error(JSON.stringify({ verified_target_outcomes: results }));
		throw error;
	} finally {
		if (singleton && singleton.exitCode === null) singleton.kill();
		if (singletonExit) await singletonExit;
		// Only flat files in this exact mkdtemp directory; do not recursively delete a computed path.
		for (const entry of await readdir(directory)) await unlink(path.join(directory, entry));
		await rmdir(directory);
	}
}
