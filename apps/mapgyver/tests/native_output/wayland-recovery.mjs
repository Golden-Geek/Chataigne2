import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
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

export async function waylandRecoveryOutput(pid) {
	const active = (await sway('-t', 'get_outputs'))
		.filter((output) => output.active)
		.sort((left, right) => left.rect.x - right.rect.x);
	assert.equal(active.length, 2, 'Start Sway with two fixture outputs');
	const connector = active[1].name;
	return {
		name: `Wayland ${connector}`,
		matchesIdentity: (id) => id.startsWith(`wayland:${connector}:`),
		ownedWindows: () => windowsFor(pid),
		pixels: async (window) =>
			window.output === connector && window.fullscreen === 1 ? [await outputPixel(connector)] : [],
		setActive: async (enabled) => {
			const replies = await sway('output', connector, enabled ? 'enable' : 'disable');
			assert(replies.every((reply) => reply.success));
		},
		isActive: async () =>
			(await sway('-t', 'get_outputs')).some((entry) => entry.name === connector && entry.active)
	};
}
