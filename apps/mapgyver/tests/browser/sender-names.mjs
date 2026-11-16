// Optional Sender Name through the real Inspector, public transport and independent SDK pixels.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
import { source } from './native-sender-selection.mjs';

const directory = path.resolve(process.argv[2]);
const gpuIndex = Number(process.argv[3]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY, 'Select the successfully compiled current app');
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(probe, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
);
assert(
	Number.isInteger(gpuIndex) && hardware.adapters[gpuIndex] && !hardware.adapters[gpuIndex].software
);
let completedSessions = 0;
const logTails = { stdout: '', stderr: '' };
const native = await openNativeEditor({
	onLogData(data, stream) {
		const lines = (logTails[stream] + data).split('\n');
		logTails[stream] = lines.pop().slice(-4096);
		for (const line of lines)
			if (line.trim() === '[mapgyver-output] native session retired') completedSessions++;
	},
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
let collisionSource;
const checks = [],
	errors = [];
native.page.on('pageerror', (error) => errors.push(String(error)));
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
async function intent(body) {
	const ack = await post('/api/ui/intent', body);
	assert(ack.success, JSON.stringify(ack));
	return ack;
}
async function operation(namespace, payload) {
	return (await intent({ kind: 'invokeAppOperation', namespace, operation: 'edit', payload }))
		.result.value;
}
const screens = (payload) => operation('mapgyver.screens', payload);
const media = (payload) => operation('mapgyver.media', payload);
const inspect = async () => (await screens({ kind: 'InspectManager' })).snapshot;
function senderParameter(graph, screen) {
	const nodes = new Map(graph.nodes.map((node) => [node.node_id, node]));
	const root = graph.nodes.find((node) => node.uuid === screen);
	assert(root, 'Screen must exist');
	const pending = [...(root.children ?? [])];
	while (pending.length) {
		const node = nodes.get(pending.pop());
		if (node.decl_id === 'parameters/sender') return node;
		pending.push(...(node.children ?? []));
	}
	assert.fail('Sender name must be a real Parameter child');
}
async function wait(read, ready, maximum = 15000) {
	const deadline = Date.now() + maximum;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(100);
	}
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
async function names(expected, absent = []) {
	await control({ kind: 'mapgyverSenders', refresh: true });
	return wait(
		async () => {
			const value = await control({ kind: 'mapgyverSenders', refresh: false });
			if (
				!value.refreshing &&
				(!expected.every((name) => value.names.includes(name)) ||
					absent.some((name) => value.names.includes(name)))
			)
				await control({ kind: 'mapgyverSenders', refresh: true });
			return value;
		},
		(value) =>
			!value.refreshing &&
			!value.error &&
			expected.every((name) => value.names.includes(name)) &&
			absent.every((name) => !value.names.includes(name))
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
				'import json,sys; from PIL import Image; print(json.dumps(sorted(set(Image.open(sys.argv[1]).convert("RGBA").get_flattened_data()))))',
				path.join(target, 'pixels.png')
			],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
	assert.deepEqual(colors, [[255, 0, 0, 255]]);
	const status = await wait(
		() => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) => value.state === 'Running' && !value.error && value.resources?.retiring === 0
	);
	checks.push({
		stage: phase,
		name,
		pixels: 'all 6144 RGBA pixels red',
		receiver_retired: true,
		resources: status.resources
	});
}
try {
	token = (await post('/api/ui/session', {})).token;
	let screen = (await inspect()).screens[0];
	const initialField = senderParameter(
		await post('/api/ui/snapshot', { scope: 'wholeGraph' }),
		screen.id
	);
	assert(!initialField.meta.enabled && initialField.meta.can_be_disabled);
	checks.push({ stage: 'fresh disabled typed Sender name', parameter: initialField.node_id });
	const suffix = randomUUID();
	const label = `Names-${suffix}`,
		renamed = `Renamed-${suffix}`,
		finalLabel = `Final-${suffix}`;
	const automatic = `MapGyver - ${label}`,
		renamedAuto = `MapGyver - ${renamed}`,
		finalAuto = `MapGyver - ${finalLabel}`;
	const custom = `Custom-${suffix}`;
	await screens({ kind: 'RenameScreen', screen: screen.id, label });
	const { reference } = await media({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Sender name red'
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
		reference: screen.surfaces[0].reference,
		definition: { ...screen.surfaces[0].definition, source: reference }
	});
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			target: { kind: 'AutomaticSpout', extent: { width: 96, height: 64 } }
		}
	});
	await native.page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await native.page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: label, exact: true })
		.click();
	const controls = screenParameterControls(native.page);
	const sender = controls.parameter('Sender name');
	const toggle = sender.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	await wait(
		() => toggle.getAttribute('aria-checked'),
		(value) => value === 'false'
	);
	const adapter = await observeNativeAdapter(native.page, hardware);
	assert.equal(adapter.index, gpuIndex);
	checks.push({ stage: 'pinned native GPU', adapter });
	const field = Number(await sender.getAttribute('data-node-id'));
	await intent({ kind: 'setParam', node: field, value: { Str: custom }, behaviour: 'Append' });
	await names([automatic]);
	await pixels(automatic, 'automatic');
	stage = 'automatic rename';
	await controls.rename(label, renamed);
	await names([renamedAuto], [automatic]);
	await pixels(renamedAuto, 'automatic-renamed');
	stage = 'custom toggle';
	await toggle.click();
	await wait(
		() => toggle.getAttribute('aria-checked'),
		(value) => value === 'true'
	);
	await names([custom], [renamedAuto]);
	await pixels(custom, 'custom');
	await controls.rename(renamed, finalLabel);
	await names([custom], [finalAuto]);
	await pixels(custom, 'custom-renamed');
	stage = 'disable restores derived name';
	await toggle.click();
	await names([finalAuto], [custom]);
	await pixels(finalAuto, 'disabled-custom');
	await intent({ kind: 'undo' });
	await names([custom], [finalAuto]);
	await intent({ kind: 'redo' });
	await names([finalAuto], [custom]);
	checks.push({ stage: 'mode undo/redo', custom, automatic: finalAuto });
	stage = 'unrelated ConfigureScreen preserves mode';
	screen = (await inspect()).screens.find((item) => item.id === screen.id);
	assert.equal(screen.definition.target.kind, 'AutomaticSpout');
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: { ...screen.definition, frame_rate: { kind: 'Explicit', fps: 30 } }
	});
	await names([finalAuto], [custom]);
	await pixels(finalAuto, 'fps-preserves-mode');
	stage = 'copy preserves automatic mode and dormant custom text';
	let graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const root = graph.nodes.find((node) => node.uuid === screen.id);
	const manager = graph.nodes.find((node) => node.node_type === 'mapgyver.screen.manager');
	await intent({
		kind: 'duplicateNode',
		source: root.node_id,
		new_parent: manager.node_id,
		new_prev_sibling: root.node_id,
		initial_params: []
	});
	const copy = (await inspect()).screens.find((item) => item.id !== screen.id);
	assert(copy && copy.definition.target.kind === 'AutomaticSpout');
	const copyName = `MapGyver - ${copy.label}`;
	const copiedField = senderParameter(
		await post('/api/ui/snapshot', { scope: 'wholeGraph' }),
		copy.id
	);
	assert(!copiedField.meta.enabled && copiedField.data.param.value.Str === custom);
	await names([finalAuto, copyName], [custom]);
	await pixels(copyName, 'copy-automatic');
	await screens({ kind: 'RemoveScreen', screen: copy.id });
	await names([finalAuto], [copyName, custom]);
	stage = 'save New reload';
	const authored = await inspect(),
		file = path.join(directory, 'sender-names.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: file })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await names([], [finalAuto, custom, copyName]);
	await post('/api/ui/project-load', { path: file });
	assert.deepEqual(await inspect(), authored);
	await names([finalAuto], [custom, copyName]);
	await pixels(finalAuto, 'reloaded-automatic');
	graph = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const senderField = senderParameter(graph, screen.id);
	assert(senderField && senderField.data.param.value.Str === custom && !senderField.meta.enabled);
	await intent({ kind: 'patchMeta', node: senderField.node_id, patch: { enabled: true } });
	await names([custom], [finalAuto]);
	await pixels(custom, 'reloaded-dormant-custom');
	stage = 'exact SDK collision and automatic retry';
	screen = (await inspect()).screens.find((item) => item.id === screen.id);
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: { ...screen.definition, enabled: false }
	});
	await names([], [custom, finalAuto]);
	collisionSource = await source(gpuIndex, custom);
	await collisionSource.command('start 96 64 0 1 0');
	await names([custom]);
	await native.page.evaluate(async () => {
		const host = window.__TAURI_INTERNALS__;
		window.__senderNameStatuses = [];
		await host.invoke('plugin:event|listen', {
			event: 'mapgyver-preview-status',
			target: { kind: 'Any' },
			handler: host.transformCallback((event) => {
				window.__senderNameStatuses = event.payload;
			})
		});
	});
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: { ...screen.definition, enabled: true }
	});
	const rejected = await wait(
		() =>
			native.page.evaluate(
				(screen) =>
					window.__senderNameStatuses.find(
						(status) => status.source.kind === 'Screen' && status.source.screen === screen
					),
				screen.id
			),
		(value) => value?.error === 'Screen output error 0x800700B7'
	);
	const collisionNames = await names([custom]);
	assert.deepEqual(
		collisionNames.names.filter((name) => name.startsWith(custom)),
		[custom],
		'SDK collision must not retain a renamed MapGyver sender'
	);
	checks.push({
		stage,
		source_pid: collisionSource.pid,
		error: rejected.error,
		exact_name: custom
	});
	await collisionSource.close();
	collisionSource = null;
	await names([custom]);
	await pixels(custom, 'collision-recovered');
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			target: { kind: 'Rectangle', bounds: { left: 0, top: 0, extent: { width: 96, height: 64 } } }
		}
	});
	await names([], [custom, finalAuto]);
	const completedBeforeNew = completedSessions;
	const beforeNew = await control({ kind: 'mapgyverRuntimeStatus' });
	await post('/api/ui/project-new', {});
	await names([], [custom, finalAuto, copyName]);
	const retired = await wait(
		() => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) =>
			value.sequence > beforeNew.sequence &&
			completedSessions > completedBeforeNew &&
			!value.error &&
			(value.state === 'Idle'
				? value.resources === null && value.gpu_adapter === null && value.screens.length === 0
				: value.state === 'Running' &&
					value.resources?.retiring === 0 &&
					value.screens.every((row) => row.screen !== screen.id))
	);
	checks.push({
		stage: 'completed old native owner retirement',
		completed_sessions: completedSessions - completedBeforeNew,
		state: retired.state,
		resources: retired.resources
	});
	checks.push({ stage: 'target/project withdrawal', absent: [custom, finalAuto, copyName] });
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', gpuIndex, pid: native.app.pid, checks, errors }, null, 2)
	);
	console.log(
		JSON.stringify({ result: 'pass', gpuIndex, pid: native.app.pid, checks: checks.length })
	);
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, errors, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	try {
		await collisionSource?.close();
	} finally {
		await native.close();
	}
}
