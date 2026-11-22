import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';

const program = promisify(execFile);
const timeout = 4000;

async function sway(...args) {
	assert(process.env.SWAYSOCK, 'Select the owned Sway fixture socket');
	const { stdout } = await program('swaymsg', ['-r', ...args], { timeout });
	return JSON.parse(stdout);
}

async function windowsFor(pid) {
	const tree = await sway('-t', 'get_tree');
	const windows = [];
	function visit(node, output) {
		if (node.type === 'output') output = node.name;
		if (node.pid === pid && node.name === 'MapGyver Screen Output') {
			windows.push({ output, fullscreen: node.fullscreen_mode });
		}
		for (const child of [...node.nodes, ...node.floating_nodes]) visit(child, output);
	}
	visit(tree);
	return windows;
}

async function outputPixel(output) {
	const { stdout } = await program('grim', ['-o', output, '-t', 'ppm', '-'], {
		timeout,
		encoding: 'buffer',
		maxBuffer: 2 * 1024 * 1024
	});
	const header = 'P6\n640 480\n255\n';
	assert.equal(stdout.subarray(0, header.length).toString(), header);
	const midpoint = header.length + (240 * 640 + 320) * 3;
	return Array.from(stdout.subarray(midpoint, midpoint + 3));
}

async function waitFor(description, predicate) {
	const deadline = performance.now() + 12000;
	for (;;) {
		const result = await predicate();
		if (result) return result;
		assert(performance.now() < deadline, description);
		await delay(120);
	}
}

export async function verifyWaylandRecovery({
	pid,
	directory,
	screen,
	color,
	edit,
	post,
	displayOptions
}) {
	const active = (await sway('-t', 'get_outputs'))
		.filter((output) => output.active)
		.sort((left, right) => left.rect.x - right.rect.x);
	assert.equal(active.length, 2, 'Start Sway with two fixture outputs');
	const output = active[1].name;
	const option = async () =>
		(await displayOptions()).find(({ variant_id }) => variant_id.startsWith(`wayland:${output}:`));
	const online = () =>
		waitFor('The intended output never appeared', async () => {
			const entry = await option();
			return entry && !entry.label.endsWith(' (offline)') && entry.variant_id;
		});
	const display = await online();
	const target = { kind: 'Fullscreen', display };
	await edit('screens', { kind: 'ConfigureScreen', screen, definition: { enabled: true, target } });

	async function expectAuthoredTarget() {
		const { definition } = await edit('screens', { kind: 'InspectScreen', screen });
		assert.equal(definition.enabled, true);
		assert.deepEqual(definition.target, target);
	}
	async function expectOutput(rgb) {
		await waitFor(`The intended ${output} output never showed RGB ${rgb}`, async () => {
			const windows = await windowsFor(pid);
			if (windows.length !== 1 || windows[0].output !== output || windows[0].fullscreen !== 1) {
				return false;
			}
			const pixel = await outputPixel(output);
			return pixel.every((value, index) => Math.abs(value - rgb[index]) < 16);
		});
	}
	async function setOutput(state) {
		assert((await sway('output', output, state)).every((reply) => reply.success));
		await waitFor(
			`Sway did not ${state} ${output}`,
			async () =>
				(await sway('-t', 'get_outputs')).some((entry) => entry.name === output && entry.active) ===
				(state === 'enable')
		);
	}
	async function expectOffline() {
		await waitFor('Saved display never went offline or its window remained', async () => {
			const entry = await option();
			return (
				entry?.variant_id === display &&
				entry.label.endsWith(' (offline)') &&
				(await windowsFor(pid)).length === 0
			);
		});
		await expectAuthoredTarget();
	}

	await expectOutput([255, 0, 0]);
	const project = join(directory, 'wayland-recovery.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	await setOutput('disable');
	await expectOffline();
	await color([0, 1, 0, 1]);
	await setOutput('enable');
	assert.equal(await online(), display);
	await expectAuthoredTarget();
	await expectOutput([0, 255, 0]);

	await setOutput('disable');
	await expectOffline();
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	assert.equal((await post('/api/ui/project-load', { path: project })).path, project);
	await expectOffline();
	await setOutput('enable');
	assert.equal(await online(), display);
	await expectAuthoredTarget();
	await expectOutput([255, 0, 0]);
	console.log(
		'Wayland output recovered the saved Screen after disconnect and reload while offline'
	);
}
