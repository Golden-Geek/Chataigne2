import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const program = promisify(execFile);
const timeout = 4000;

export async function linuxOutputWindows(pid) {
	const { stdout: tree } = await program('xwininfo', ['-root', '-tree'], { timeout });
	const windows = [];
	for (const [, id] of tree.matchAll(/(0x[\da-f]+) "MapGyver Screen Output"/g)) {
		const { stdout: owner } = await program('xprop', ['-id', id, '_NET_WM_PID'], { timeout });
		if (Number(owner.match(/=\s*(\d+)\s*$/)?.[1]) !== pid) continue;
		const { stdout: info } = await program('xwininfo', ['-id', id], { timeout });
		const value = (key) => Number(info.match(new RegExp(`^\\s*${key}:\\s+(-?\\d+)`, 'm'))?.[1]);
		windows.push({
			id,
			visible: info.includes('Map State: IsViewable'),
			bounds: {
				left: value('Absolute upper-left X'),
				top: value('Absolute upper-left Y'),
				extent: { width: value('Width'), height: value('Height') }
			}
		});
	}
	return windows;
}

export async function linuxOutputCenter(id) {
	const { stdout } = await program(
		'ffmpeg',
		[
			'-nostdin',
			'-hide_banner',
			'-loglevel',
			'error',
			'-f',
			'x11grab',
			'-framerate',
			'30',
			'-window_id',
			String(parseInt(id, 16)),
			'-i',
			process.env.DISPLAY,
			'-frames:v',
			'1',
			'-vf',
			'crop=8:8,format=rgb24',
			'-f',
			'rawvideo',
			'pipe:1'
		],
		{ timeout, encoding: 'buffer' }
	);
	assert.equal(stdout.length, 8 * 8 * 3, 'X11 capture has the wrong sample size');
	const pixels = [];
	for (let offset = 0; offset < stdout.length; offset += 3) {
		pixels.push(stdout.subarray(offset, offset + 3));
	}
	return pixels;
}
