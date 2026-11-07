import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function openInitialScreenHeadless(directory, { frontend = false } = {}) {
	const listener = createServer();
	listener.listen(0, '127.0.0.1');
	await once(listener, 'listening');
	const port = listener.address().port;
	await new Promise((resolve) => listener.close(resolve));
	const base = `http://127.0.0.1:${port}`;
	const stop = path.join(directory, 'host.stop');
	const app = spawn(
		path.resolve(process.env.MAPGYVER_TEST_BINARY),
		[
			'--headless',
			...(frontend ? [] : ['--no-frontend']),
			'--no-remote',
			'--automation-shutdown-file',
			stop
		],
		{
			windowsHide: true,
			stdio: ['ignore', 'pipe', 'pipe'],
			env: {
				...process.env,
				APPDATA: path.join(directory, 'data'),
				GC_UI_BIND: `127.0.0.1:${port}`
			}
		}
	);
	let log = '';
	for (const stream of [app.stdout, app.stderr])
		stream.on('data', (data) => (log = (log + data).slice(-65536)));
	const exited = new Promise((resolve, reject) => {
		app.once('error', reject);
		app.once('close', resolve);
	});
	const watchdog = setTimeout(() => app.kill(), 120000);
	async function close() {
		try {
			if (app.exitCode === null) await writeFile(stop, 'stop');
			const code = await Promise.race([
				exited,
				delay(7000).then(() => {
					throw Error('Headless normal shutdown expired');
				})
			]);
			assert.equal(code, 0, log);
		} finally {
			clearTimeout(watchdog);
			if (app.exitCode === null) {
				app.kill();
				await exited;
			}
		}
	}
	try {
		const deadline = Date.now() + 20000;
		for (;;) {
			assert.equal(app.exitCode, null, log);
			try {
				if ((await fetch(base + '/api/ui/health', { signal: AbortSignal.timeout(500) })).ok) break;
			} catch {}
			assert(Date.now() < deadline, log);
			await delay(100);
		}
		return { app, base, close, log: () => log };
	} catch (error) {
		await close();
		throw error;
	}
}
