import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const program = promisify(execFile);
const timeout = 4000;
const sampler = fileURLToPath(new URL('./x11_sample.py', import.meta.url));

async function queryWindow(command, args) {
	try {
		return (await program(command, args, { timeout })).stdout;
	} catch (error) {
		// Retirement can destroy the owned window between the tree and property queries.
		if (/\bBad\s?(?:Window|Drawable)\b/.test(error.stderr ?? '')) return null;
		throw error;
	}
}

function absoluteBounds(info) {
	const value = (key) => Number(info.match(new RegExp(`^\\s*${key}:\\s+(-?\\d+)`, 'm'))?.[1]);
	return {
		left: value('Absolute upper-left X'),
		top: value('Absolute upper-left Y'),
		extent: { width: value('Width'), height: value('Height') }
	};
}

export async function linuxRootBounds() {
	const { stdout } = await program('xwininfo', ['-root'], { timeout });
	return absoluteBounds(stdout);
}

export async function linuxWindow(id) {
	const info = await queryWindow('xwininfo', ['-id', id]);
	return info
		? { id, visible: info.includes('Map State: IsViewable'), bounds: absoluteBounds(info) }
		: null;
}

export async function linuxOutputWindows(pid) {
	return linuxOwnedWindows(pid, 'MapGyver Screen Output');
}

export async function linuxOwnedWindows(pid, title) {
	const { stdout: tree } = await program('xwininfo', ['-root', '-tree'], { timeout });
	const windows = [];
	for (const [, id, name] of tree.matchAll(/(0x[\da-f]+) "([^"]+)"/g)) {
		if (name !== title) continue;
		const owner = await queryWindow('xprop', ['-id', id, '_NET_WM_PID']);
		if (Number(owner?.match(/=\s*(\d+)\s*$/)?.[1]) !== pid) continue;
		const window = await linuxWindow(id);
		if (window) windows.push(window);
	}
	return windows;
}

export async function linuxOutputCenter({ id, bounds }) {
	const { width, height } = bounds.extent;
	// Query the owned drawable directly. FFmpeg's x11grab returned black for display-sized
	// windows on Xvfb while the same drawable contained the expected green and blue pixels.
	const stdout = await queryWindow('python3', [sampler, id, String(width), String(height)]);
	if (stdout === null) return [];
	const pixels = JSON.parse(stdout);
	assert.equal(pixels.length, 8 * 8, 'X11 capture has the wrong sample size');
	return pixels;
}
