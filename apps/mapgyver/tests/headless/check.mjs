// Run after building MapGyver and application_process_fixture with application-process-fixture.
// Exercises the ordinary --headless entrypoint through public HTTP intents, without a frontend/GPU.
// Add --target-outcomes for the bounded singleton/launcher/breakaway/invalid-image acceptance cases.
// Add --project-replacement for New/Save/Load and known-tree host-exit acceptance (build Golden IO's fixture too).
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { verifyTargetOutcomes } from './outcomes.mjs';
import { verifyProjectReplacement } from './project-replacement.mjs';

assert(
	!(process.argv.includes('--target-outcomes') && process.argv.includes('--project-replacement')),
	'Run target-outcome and project-replacement acceptance separately'
);
const runDirectory = await mkdtemp(path.join(tmpdir(), 'mapgyver-headless-'));

const listener = createServer();
await new Promise((resolve, reject) => {
	listener.once('error', reject);
	listener.listen(0, '127.0.0.1', resolve);
});
const port = listener.address().port;
await new Promise((resolve) => listener.close(resolve));
const base = `http://127.0.0.1:${port}`;
const fixture = path.resolve('target/debug/application_process_fixture.exe');
const external = spawn(fixture, [], { windowsHide: true, stdio: 'ignore' });
const app = spawn(
	path.resolve('target/debug/MapGyver.exe'),
	['--headless', '--no-frontend', '--no-remote'],
	{
		env: {
			...process.env,
			APPDATA: path.join(runDirectory, 'data'),
			GC_UI_BIND: `127.0.0.1:${port}`
		},
		windowsHide: true,
		stdio: ['ignore', 'pipe', 'pipe']
	}
);
let log = '';
let token;
const appExit = new Promise((resolve, reject) => {
	app.once('error', reject);
	app.once('close', (code) => resolve(code));
});
const externalExit = new Promise((resolve, reject) => {
	external.once('error', reject);
	external.once('close', resolve);
});
for (const stream of [app.stdout, app.stderr]) {
	stream.on('data', (data) => {
		log = (log + data).slice(-65536);
	});
}
async function post(route, payload) {
	const response = await fetch(`${base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(payload)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const applications = (editValue) => edit({ kind: 'Applications', edit: editValue });
const matrix = (media, editValue) => edit({ kind: 'Matrix', media, edit: editValue });
async function waitFor(phase) {
	const deadline = Date.now() + 7000;
	while (true) {
		const { snapshot } = await applications({ kind: 'InspectRuntime' });
		assert.equal(snapshot.callbacks.fault, null);
		if (
			snapshot.applications[0]?.phase === phase &&
			snapshot.callbacks.pending === 0 &&
			snapshot.callbacks.acknowledged > 0
		)
			return snapshot;
		assert(Date.now() < deadline, `headless ${phase} timeout: ${JSON.stringify(snapshot)}\n${log}`);
		await delay(25);
	}
}
function alive(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		if (error.code === 'ESRCH') return false;
		throw error;
	}
}
try {
	const deadline = Date.now() + 15000;
	while (true) {
		assert.equal(app.exitCode, null, `headless app exited: ${log}`);
		try {
			if ((await fetch(`${base}/api/ui/health`, { signal: AbortSignal.timeout(500) })).ok) break;
		} catch {
			/* Bounded startup; never restart the process. */
		}
		assert(Date.now() < deadline, `headless startup timeout: ${log}`);
		await delay(50);
	}
	token = (await post('/api/ui/session', {})).token;
	const { application } = await applications({ kind: 'Create', label: 'Headless owned fixture' });
	const { snapshot: configured } = await applications({ kind: 'Inspect', application });
	await applications({
		kind: 'Configure',
		application,
		definition: {
			...configured.definition,
			executable: fixture,
			readiness: { kind: 'ProcessStarted' },
			stop: { graceful_ms: 30, retirement_ms: 1000 },
			restart: { ...configured.definition.restart, enabled: false }
		}
	});
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Headless columns'
	});
	const { columns } = await matrix(reference.media, { kind: 'AddColumns', count: 4 });
	const { snapshot: authored } = await matrix(reference.media, { kind: 'Inspect' });
	await matrix(reference.media, {
		kind: 'ConfigureColumn',
		column: columns[1],
		definition: {
			...authored.columns.find((column) => column.id === columns[1]).definition,
			applications: [{ application, pre: 1, post: 1 }]
		}
	});
	const launch = (column) => matrix(reference.media, { kind: 'LaunchColumn', column, mode: 'Cut' });
	await launch(columns[0]);
	const initial = await waitFor('Ready');
	assert.equal(initial.supervision.host, 'Headless');
	assert.equal(initial.supervision.demand.kind, 'Running');
	assert.equal(initial.supervision.callbacks.kind, 'Running');
	const first = initial.applications[0];
	assert(first.process_id && first.launch);
	assert.notEqual(first.process_id, external.pid);
	assert(initial.callbacks.acknowledged >= 2);
	await launch(columns[2]);
	const retained = (await waitFor('Ready')).applications[0];
	assert.equal(retained.launch, first.launch);
	assert.equal(retained.process_id, first.process_id);
	await launch(columns[3]);
	const unloaded = await waitFor('Idle');
	assert.equal(unloaded.applications[0].process_id, null);
	assert(!alive(first.process_id), 'unload must retire the owned process');
	assert(alive(external.pid), 'same-executable external process must survive owned unload');
	await delay(200);
	assert.equal((await waitFor('Idle')).applications[0].phase, 'Idle');
	await applications({ kind: 'SetEnabled', application, enabled: false });
	assert.equal(
		(await applications({ kind: 'Inspect', application })).snapshot.enablement,
		'Disabled'
	);
	await launch(columns[1]);
	assert.equal((await waitFor('Unavailable')).applications[0].process_id, null);
	await applications({ kind: 'SetEnabled', application, enabled: true });
	const restarted = await waitFor('Ready');
	const second = restarted.applications[0];
	assert.notEqual(second.launch, first.launch);
	assert(restarted.callbacks.acknowledged > unloaded.callbacks.acknowledged);
	async function configuration(expected) {
		const deadline = Date.now() + 5000;
		while (true) {
			const { snapshot } = await applications({ kind: 'InspectRuntime' });
			const status = snapshot.applications.find((entry) => entry.application === application);
			if (status.configuration === expected) {
				assert.equal(status.process_id, second.process_id);
				return;
			}
			assert(Date.now() < deadline, `availability did not reconcile to ${expected}`);
			await delay(25);
		}
	}
	await applications({ kind: 'SetEnabled', application, enabled: false });
	await configuration('Retained');
	await applications({ kind: 'SetEnabled', application, enabled: true });
	await configuration('Current');
	const extra = [];
	for (let i = 0; i < 65; i++)
		extra.push((await applications({ kind: 'Create', label: null })).application);
	const overloaded = [];
	for (const assigned of [extra.slice(0, 64), extra.slice(64)]) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label: null });
		const { columns } = await matrix(reference.media, { kind: 'AddColumns', count: 1 });
		await matrix(reference.media, {
			kind: 'ConfigureColumn',
			column: columns[0],
			definition: {
				transition: null,
				applications: assigned.map((application) => ({ application, pre: 0, post: 0 }))
			}
		});
		await matrix(reference.media, { kind: 'LaunchColumn', column: columns[0], mode: 'Cut' });
		overloaded.push(reference.media);
	}
	const admissionDeadline = Date.now() + 5000;
	let admissionMessage;
	while (true) {
		const { snapshot } = await applications({ kind: 'InspectRuntime' });
		if (snapshot.supervision.demand.kind === 'Retrying') {
			admissionMessage = snapshot.supervision.demand.message;
			assert(admissionMessage.includes('at most 64'));
			assert.equal(
				snapshot.applications.length,
				1,
				'failed aggregate admission must preserve the prior process set'
			);
			assert.equal(snapshot.applications[0].process_id, second.process_id);
			break;
		}
		assert(
			Date.now() < admissionDeadline,
			'ordinary headless host did not expose admission failure'
		);
		await delay(25);
	}
	for (const media of overloaded) await matrix(media, { kind: 'ClearPlayback' });
	while (true) {
		const { snapshot } = await applications({ kind: 'InspectRuntime' });
		if (snapshot.supervision.demand.kind === 'Running') {
			assert.equal(
				snapshot.applications.find((entry) => entry.application === application).process_id,
				second.process_id
			);
			break;
		}
		assert(
			Date.now() < admissionDeadline,
			'ordinary headless host did not publish demand recovery'
		);
		await delay(25);
	}
	const targetOutcomes = process.argv.includes('--target-outcomes')
		? await verifyTargetOutcomes({ applications, matrix, edit, fixture, alive })
		: undefined;
	const replacement = process.argv.includes('--project-replacement')
		? await verifyProjectReplacement({
				post,
				applications,
				matrix,
				edit,
				fixture,
				alive,
				directory: runDirectory,
				application,
				prior: second,
				external
			})
		: undefined;
	app.kill();
	await appExit;
	const cleanupDeadline = Date.now() + 3000;
	const shutdownPids = replacement?.shutdown_pids ?? [second.process_id];
	while (shutdownPids.some(alive) && Date.now() < cleanupDeadline) await delay(25);
	assert(
		shutdownPids.every((pid) => !alive(pid)),
		'host exit must release every known owned tree member'
	);
	assert(!alive(second.process_id), 'host exit must release its owned process tree');
	assert(alive(external.pid), 'host exit must not kill the external same-executable process');
	assert(
		!log.includes('[mapgyver-output] native session'),
		'headless startup must not create native rendering'
	);
	console.log(
		JSON.stringify({
			passed: true,
			host: app.pid,
			external: external.pid,
			first: first.process_id,
			second: second.process_id,
			acknowledged: restarted.callbacks.acknowledged,
			admission_recovered: true,
			availability_verified: true,
			target_outcomes: targetOutcomes,
			project_replacement: replacement,
			admission_message: admissionMessage
		})
	);
} finally {
	if (app.exitCode === null) app.kill();
	if (external.exitCode === null) external.kill();
	await Promise.all([appExit, externalExit]);
	// Verify the exact mkdtemp target before removing this test's private preferences/artifacts.
	const cleanupTarget = path.resolve(runDirectory);
	assert.equal(path.dirname(cleanupTarget), path.resolve(tmpdir()));
	assert(path.basename(cleanupTarget).startsWith('mapgyver-headless-'));
	await rm(cleanupTarget, { recursive: true, force: true });
}
