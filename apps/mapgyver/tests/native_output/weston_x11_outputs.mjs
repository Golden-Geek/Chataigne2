import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { linuxWindow } from './linux_x11_windows.mjs';

const program = promisify(execFile);
const timeout = 4000;

export async function westonOutputs() {
	const { stdout } = await program('xwininfo', ['-root', '-tree'], { timeout });
	const windows = new Map(
		Array.from(
			stdout.matchAll(/(0x[\da-f]+) "Weston Compositor - (screen[01])"/g),
			([, id, name]) => [name, id]
		)
	);
	assert.deepEqual([...windows.keys()].sort(), ['screen0', 'screen1']);
	const primary = await linuxWindow(windows.get('screen0'));
	assert(primary?.visible, 'Weston screen0 is not mapped');
	// Weston creates both parent windows at the origin. Separate them so both
	// output framebuffers stay visible to the independent X11 pixel observer.
	await program(
		'xdotool',
		['windowmove', windows.get('screen1'), String(primary.bounds.extent.width), '0'],
		{ timeout }
	);
	const secondary = await linuxWindow(windows.get('screen1'));
	assert(secondary?.visible, 'Weston screen1 is not mapped');
	assert(secondary.bounds.left >= primary.bounds.left + primary.bounds.extent.width);
	return [primary, secondary];
}
