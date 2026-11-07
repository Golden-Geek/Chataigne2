// Ordinary desktop/headless entrypoints and public project operations; fresh owned profiles only.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the successfully compiled current app');
const headless = process.argv[3] === 'headless';
const gpuIndex = headless ? null : Number(process.argv[3]);
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware = headless
	? null
	: JSON.parse(
			execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
		);
if (!headless)
	assert(
		Number.isInteger(gpuIndex) &&
			hardware.adapters[gpuIndex] &&
			!hardware.adapters[gpuIndex].software
	);
const native = headless
	? await openInitialScreenHeadless(directory)
	: await openNativeEditor({
			prepareProfile: async (root) => {
				const folder = path.join(root, 'MapGyver');
				await mkdir(folder, { recursive: true });
				execFileSync(
					probe,
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(folder, 'preferences.json'),
						'--selected',
						adapterKey(hardware.adapters[gpuIndex].id)
					],
					{ windowsHide: true }
				);
			}
		});
let token,
	stage = 'startup';
const checks = [],
	errors = [];
native.page?.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function operation(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const screens = (payload) => operation('mapgyver.screens', payload);
const media = (payload) => operation('mapgyver.media', payload);
const inspect = async () => (await screens({ kind: 'InspectManager' })).snapshot;
async function wait(read, ready, maximum = 15000) {
	const deadline = Date.now() + maximum;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(100);
	}
}
async function fresh() {
	const snapshot = await inspect();
	assert.equal(snapshot.screens.length, 1);
	const screen = snapshot.screens[0];
	assert.equal(screen.label, 'Screen');
	assert.equal(screen.definition.enabled, true);
	assert.equal(screen.definition.anti_aliasing, 'Raw');
	assert.deepEqual(screen.definition.target, {
		kind: 'Spout',
		sender: 'MapGyver Screen',
		extent: { width: 1920, height: 1080 }
	});
	assert.equal(screen.surfaces.length, 1);
	assert.equal(screen.surfaces[0].definition.source, null);
	const graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	assert.equal(graph.history.undo_len, 0);
	assert.equal(graph.history.redo_len, 0);
	checks.push({
		stage,
		screen: screen.id,
		surface: screen.surfaces[0].reference,
		generation: graph.project_generation,
		history: graph.history
	});
	return screen;
}
const control = (request) =>
	native.page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
async function senderPresent(name, expected) {
	const old = await control({ kind: 'mapgyverSenders', refresh: true });
	return wait(
		async () => {
			const value = await control({ kind: 'mapgyverSenders', refresh: false });
			if (!value.refreshing && value.names.includes(name) !== expected)
				await control({ kind: 'mapgyverSenders', refresh: true });
			return value;
		},
		(value) =>
			value.revision > old.revision &&
			!value.refreshing &&
			!value.error &&
			value.names.includes(name) === expected
	);
}
async function pixels(name, phase) {
	const target = path.join(directory, phase);
	const result = JSON.parse(
		execFileSync(
			probe,
			['--role', 'pixels', '--adapter', String(gpuIndex), '--name', name, '--directory', target],
			{ encoding: 'utf8', windowsHide: true, timeout: 15000 }
		)
	);
	assert.deepEqual(result, { width: 96, height: 64, retired: true });
	const colors = JSON.parse(
		execFileSync(
			'python',
			[
				'-c',
				'import json,sys; from PIL import Image; print(json.dumps(sorted(set(Image.open(sys.argv[1]).convert("RGBA").getdata()))))',
				path.join(target, 'pixels.png')
			],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
	assert.deepEqual(colors, [[255, 0, 0, 255]]);
	checks.push({ stage: phase, full_output: 'red96x64', receiver_retired: true });
}
try {
	token = (await post('/api/ui/session', {})).token;
	await fresh();
	if (!headless) {
		const beforeSelection = await inspect();
		await native.page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		await native.page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: 'Screen', exact: true })
			.click();
		await native.page
			.locator('.inspector-header .title-text')
			.filter({ hasText: /^Screen$/ })
			.waitFor();
		assert.deepEqual(await inspect(), beforeSelection);
		await fresh();
	}
	stage = 'authored multiple Screens';
	for (let i = 0; i < 2; i++)
		await screens({ kind: 'CreateScreen', label: `Saved Screen ${i + 2}` });
	const many = await inspect();
	assert.equal(many.screens.length, 3);
	const manyFile = path.join(directory, 'multiple.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: manyFile })).ok, true);
	stage = 'intentional empty project';
	for (const screen of many.screens) await screens({ kind: 'RemoveScreen', screen: screen.id });
	assert.equal((await inspect()).screens.length, 0);
	const emptyFile = path.join(directory, 'empty.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: emptyFile })).ok, true);
	for (let cycle = 0; cycle < 2; cycle++) {
		await post('/api/ui/project-load', { path: emptyFile });
		assert.equal((await inspect()).screens.length, 0);
		if (!headless) {
			await native.page.reload();
			assert.equal((await inspect()).screens.length, 0);
		}
	}
	checks.push({ stage, load_cycles: 2, count: 0, browser_reload: !headless });
	stage = 'multiple project load';
	await post('/api/ui/project-load', { path: manyFile });
	assert.deepEqual(await inspect(), many);
	checks.push({ stage, exact_authored_count: 3 });
	for (let cycle = 0; cycle < 3; cycle++) {
		stage = `New ${cycle}`;
		const old = (await inspect()).screens[0].id;
		if (!headless) await native.page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		if (!headless && cycle === 1) {
			// Use supported Windows input for the actual native shortcut check.
			await native.page.getByRole('button', { name: 'Open File menu', exact: true }).focus();
			await writeFile(
				path.join(directory, 'shortcut-ready.json'),
				JSON.stringify({ pid: native.app.pid, old_screen: old })
			);
			await wait(
				inspect,
				(value) => value.screens.length === 1 && value.screens[0].id !== old,
				90000
			);
		} else if (!headless && cycle === 2) {
			await native.page.getByRole('button', { name: 'Open File menu', exact: true }).click();
			await native.page
				.getByRole('menuitem')
				.filter({ has: native.page.getByText('New', { exact: true }) })
				.click();
		} else assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await wait(inspect, (value) => value.screens.length === 1 && value.screens[0].id !== old);
		await fresh();
	}
	if (!headless) {
		stage = 'initial Screen output';
		const adapter = await observeNativeAdapter(native.page, hardware);
		assert.equal(adapter.index, gpuIndex);
		checks.push({ stage: 'pinned native GPU', adapter });
		const screen = (await inspect()).screens[0],
			surface = screen.surfaces[0];
		const { reference } = await media({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Initial Screen red'
		});
		await media({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 32, height: 24 } }
		});
		await media({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		await screens({
			kind: 'ConfigureSurface',
			reference: surface.reference,
			definition: { ...surface.definition, source: reference }
		});
		const sender = `MapGyverInitial-${randomUUID()}`;
		await screens({
			kind: 'ConfigureScreen',
			screen: screen.id,
			definition: {
				...screen.definition,
				target: { kind: 'Spout', sender, extent: { width: 96, height: 64 } }
			}
		});
		await senderPresent(sender, true);
		await pixels(sender, 'initial-output');
		const authored = await inspect(),
			file = path.join(directory, 'authored.mapgyver');
		await post('/api/ui/project-save', { path: file });
		const previousStatus = await control({ kind: 'mapgyverRuntimeStatus' });
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		const replacement = await fresh();
		await senderPresent(sender, false);
		const retired = await wait(
			() => control({ kind: 'mapgyverRuntimeStatus' }),
			(value) =>
				value.sequence > previousStatus.sequence &&
				value.state === 'Running' &&
				value.screens.every((row) => row.screen === replacement.id) &&
				value.resources?.retiring === 0 &&
				!value.error
		);
		checks.push({
			stage: 'old output withdrawn on New',
			sequence: retired.sequence,
			previous_sequence: previousStatus.sequence,
			resources: retired.resources,
			screens: retired.screens
		});
		await post('/api/ui/project-load', { path: file });
		assert.deepEqual(await inspect(), authored);
		await senderPresent(sender, true);
		await pixels(sender, 'loaded-output');
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await fresh();
		await senderPresent(sender, false);
	}
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				mode: headless ? 'headless' : 'native',
				gpuIndex,
				pid: native.app.pid,
				checks,
				errors
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			result: 'pass',
			mode: headless ? 'headless' : 'native',
			gpuIndex,
			pid: native.app.pid,
			checks: checks.length
		})
	);
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, errors, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
