import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { isDeepStrictEqual, promisify } from 'node:util';
import { linuxOutputCenter, linuxOutputWindows } from './linux_x11_windows.mjs';

const program = promisify(execFile);
const timeout = 4000;

async function activeBounds() {
	const { stdout } = await program('xrandr', ['--query'], { timeout });
	const description = stdout.split('\n').find((line) => line.startsWith('DUMMY1 connected '));
	const geometry = / (\d+)x(\d+)([+-]\d+)([+-]\d+)/.exec(description ?? '');
	if (!geometry) return undefined;
	const [, width, height, left, top] = geometry.map(Number);
	return { left, top, extent: { width, height } };
}

export async function x11RecoveryOutput(pid) {
	const bounds = await activeBounds();
	assert.deepEqual(bounds, { left: 640, top: 0, extent: { width: 640, height: 480 } });
	return {
		name: 'X11 DUMMY1',
		matchesIdentity: (id) => id === 'x11:DUMMY1',
		ownedWindows: () => linuxOutputWindows(pid),
		pixels: (window) =>
			window.visible && isDeepStrictEqual(window.bounds, bounds) ? linuxOutputCenter(window) : [],
		setActive: (enabled) =>
			program(
				'xrandr',
				[
					'--output',
					'DUMMY1',
					...(enabled ? ['--mode', '640x480-fixture', '--pos', '640x0'] : ['--off'])
				],
				{ timeout }
			),
		isActive: async () => (await activeBounds()) !== undefined
	};
}
