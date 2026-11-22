import { execFile } from 'node:child_process';
import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openInitialScreenDesktop } from '../headless/initial-screen-host.mjs';
import { linuxOwnedWindows } from './linux_x11_windows.mjs';

const directory = await mkdtemp(join(tmpdir(), 'mapgyver-desktop-readiness-'));
const host = await openInitialScreenDesktop(directory);
const program = promisify(execFile);

async function ownedWindows() {
	if (process.platform !== 'win32') return linuxOwnedWindows(host.app.pid, 'MapGyver 2');
	const { stdout } = await program(
		'python',
		['apps/mapgyver/tests/native_output/window_state.py', String(host.app.pid), '--main'],
		{ windowsHide: true, timeout: 4000 }
	);
	return JSON.parse(stdout);
}

let fixtureFailure;
try {
	const windows = process.platform === 'win32';
	if (windows || (process.platform === 'linux' && process.env.GDK_BACKEND === 'x11')) {
		const view = windows ? 'WebView2' : 'WebKit';
		const deadline = performance.now() + 12000;
		let mapped = false;
		for (;;) {
			if (!mapped) mapped = (await ownedWindows()).some(({ visible }) => visible);
			// Run before launching Chromium so the snapshot can only come from the native view.
			if (mapped && host.log().includes('[ui-ws] snapshot request_id=')) break;
			assert(
				host.app.exitCode === null && host.app.signalCode === null && performance.now() < deadline,
				mapped
					? `The native ${view} did not request the workbench snapshot`
					: 'The MapGyver process did not map its native workbench window'
			);
			await delay(120);
		}
		console.log(`Owned MapGyver workbench is mapped and the native ${view} connected`);
	}
	const { stdout } = await program(
		process.execPath,
		[
			'tools/product-gate/hooks/strict-ui-readiness.mjs',
			'--repository-root',
			process.cwd(),
			'--url',
			host.base + '/',
			'--screenshot',
			join(directory, 'ready.png'),
			'--timeout-ms',
			'70000'
		],
		{ timeout: 75000 }
	);
	console.log(stdout);
} catch (error) {
	fixtureFailure = error;
	console.error(host.log());
	throw error;
} finally {
	try {
		await host.close();
	} catch (error) {
		if (!fixtureFailure) throw error;
		console.error(error);
	}
}
console.log('Native desktop host exited normally');
