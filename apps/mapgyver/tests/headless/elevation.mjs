// Actual elevation-required image through the normal public API; never use shell/runas fallback.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';

export async function verifyElevationRequired({
	applications,
	matrix,
	edit,
	directory,
	alive,
	externalPid
}) {
	const executable = path.resolve('target/debug/application_elevation_required_fixture.exe');
	const manifest = await readFile(executable.replace(/\.exe$/, '.manifest'), 'utf8');
	assert.match(manifest, /requestedExecutionLevel\s+level="requireAdministrator"/);
	const { stdout } = await promisify(execFile)(
		'powershell.exe',
		[
			'-NoProfile',
			'-NonInteractive',
			'-Command',
			'([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)'
		],
		{ windowsHide: true, timeout: 5000 }
	);
	assert(['True', 'False'].includes(stdout.trim()), 'caller token must be observed');
	const callerElevated = stdout.trim() === 'True';
	await stat(executable);
	const report = path.join(directory, 'elevation-required-executed.txt');
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Elevation denial'
	});
	const { columns } = await matrix(reference.media, { kind: 'AddColumns', count: 1 });
	const { application } = await applications({
		kind: 'Create',
		label: 'Elevation-required target'
	});
	const { snapshot } = await applications({ kind: 'Inspect', application });
	await applications({
		kind: 'Configure',
		application,
		definition: {
			...snapshot.definition,
			executable,
			arguments: ['--lifecycle-token', report],
			window: 'Hidden',
			readiness: { kind: 'ProcessStarted' },
			stop: { graceful_ms: 30, retirement_ms: 1000 },
			restart: { ...snapshot.definition.restart, enabled: false }
		}
	});
	const authored = (await applications({ kind: 'Inspect', application })).snapshot;
	await matrix(reference.media, {
		kind: 'ConfigureColumn',
		column: columns[0],
		definition: { transition: null, applications: [{ application, pre: 0, post: 0 }] }
	});
	async function observe(phase) {
		const deadline = Date.now() + 5000;
		while (true) {
			const { snapshot } = await applications({ kind: 'InspectRuntime' });
			assert.equal(snapshot.supervision.host, 'Headless');
			assert.equal(snapshot.supervision.demand.kind, 'Running');
			assert.equal(snapshot.callbacks.fault, null);
			const status = snapshot.applications.find((entry) => entry.application === application);
			if (status) {
				assert.notEqual(status.process_id, externalPid, 'external process must not be adopted');
				if (!callerElevated) {
					assert.equal(status.process_id, null, 'non-elevated caller must not start target');
					assert.notEqual(status.phase, 'Ready');
				}
				if (status.phase === phase && snapshot.callbacks.pending === 0) return status;
			}
			assert(Date.now() < deadline, `elevation ${phase} timeout: ${JSON.stringify(snapshot)}`);
			await delay(10);
		}
	}
	await matrix(reference.media, { kind: 'LaunchColumn', column: columns[0], mode: 'Cut' });
	if (callerElevated) {
		const ready = await observe('Ready');
		assert(ready.process_id && alive(ready.process_id));
		const deadline = Date.now() + 5000;
		let targetToken;
		while (true) {
			try {
				targetToken = JSON.parse(await readFile(report, 'utf8'));
				break;
			} catch (error) {
				if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error;
			}
			assert(Date.now() < deadline, 'owned token report timeout');
			await delay(10);
		}
		assert.deepEqual(targetToken, { pid: ready.process_id, elevated: true });
		assert.deepEqual((await applications({ kind: 'Inspect', application })).snapshot, authored);
		await matrix(reference.media, { kind: 'ClearPlayback' });
		await observe('Idle');
		assert(!alive(ready.process_id), 'owned elevated process must retire on unload');
		assert(alive(externalPid), 'unload must preserve the external process');
		return {
			application,
			executable,
			phase: ready.phase,
			launch: ready.launch,
			process_id: ready.process_id,
			caller_elevated: true,
			target_token: targetToken,
			owned_pid_absent: true,
			elevation_requested: false,
			denial_verified: false,
			running_elevated_process_verified: true,
			retirement: 'Idle'
		};
	}
	const failed = await observe('Failed');
	assert.match(failed.message, /\(os error 740\)/);
	assert.equal(failed.exit_code, null);
	assert.equal(failed.next_action_ms, null);
	assert.equal(failed.failures, 1);
	await delay(150);
	const stable = await observe('Failed');
	assert.equal(stable.launch, failed.launch);
	assert.equal(stable.failures, 1);
	assert.deepEqual((await applications({ kind: 'Inspect', application })).snapshot, authored);
	await assert.rejects(stat(report), { code: 'ENOENT' }, 'target main must not execute');
	await matrix(reference.media, { kind: 'ClearPlayback' });
	await observe('Idle');
	return {
		application,
		executable,
		error: 740,
		phase: failed.phase,
		launch: failed.launch,
		failures: stable.failures,
		process_id: failed.process_id,
		target_main_not_executed: true,
		caller_elevated: false,
		elevation_requested: false,
		denial_verified: true,
		running_elevated_process_verified: false,
		retirement: 'Idle'
	};
}
