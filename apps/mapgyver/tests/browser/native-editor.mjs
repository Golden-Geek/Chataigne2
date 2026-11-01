// Actual bundled WebView2 input through Playwright's documented CDP connection.
// https://playwright.dev/docs/webview2 — no desktop activation or host bypass.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright-core');

async function listener() {
	const socket = createServer();
	socket.listen(0, '127.0.0.1');
	await once(socket, 'listening');
	return socket;
}

export async function openNativeEditor() {
	const directory = await mkdtemp(path.join(tmpdir(), 'mapgyver-native-editor-'));
	const hostSocket = await listener();
	const cdpSocket = await listener();
	const hostPort = hostSocket.address().port;
	const cdpPort = cdpSocket.address().port;
	await Promise.all(
		[hostSocket, cdpSocket].map((socket) => new Promise((resolve) => socket.close(resolve)))
	);
	const shutdown = path.join(directory, 'host.stop');
	const base = `http://127.0.0.1:${hostPort}`;
	const cdp = `http://127.0.0.1:${cdpPort}`;
	const app = spawn(
		path.resolve('target/debug/MapGyver.exe'),
		['--no-remote', '--automation-shutdown-file', shutdown],
		{
			windowsHide: true,
			env: {
				...process.env,
				APPDATA: path.join(directory, 'data'),
				GC_UI_BIND: `127.0.0.1:${hostPort}`,
				WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
				WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort} --remote-debugging-address=127.0.0.1`
			},
			stdio: ['ignore', 'pipe', 'pipe']
		}
	);
	let log = '';
	for (const stream of [app.stdout, app.stderr])
		stream.on('data', (data) => {
			log = (log + data).slice(-16384);
		});
	const exited = new Promise((resolve, reject) => {
		app.once('error', reject);
		app.once('close', resolve);
	});
	let browser;
	const watchdog = setTimeout(() => app.kill(), 10 * 60 * 1000);
	async function close() {
		try {
			if (app.exitCode === null) await writeFile(shutdown, 'stop');
			const code = await Promise.race([
				exited,
				delay(7000).then(() => {
					throw new Error('native editor normal shutdown timed out');
				})
			]);
			assert.equal(code, 0, `native editor must exit normally: ${log}`);
		} finally {
			clearTimeout(watchdog);
			if (app.exitCode === null) {
				app.kill();
				await exited;
			}
			await browser?.close().catch(() => {});
			const target = path.resolve(directory);
			assert.equal(path.dirname(target), path.resolve(tmpdir()));
			assert(path.basename(target).startsWith('mapgyver-native-editor-'));
			// A live WebView file lease can outlast host exit; keep this exact private directory on failure.
			await rm(target, { recursive: true, force: true, maxRetries: 4, retryDelay: 100 });
		}
	}
	try {
		const deadline = Date.now() + 20_000;
		while (true) {
			assert.equal(app.exitCode, null, `native editor exited during startup: ${log}`);
			try {
				if ((await fetch(`${cdp}/json/version`, { signal: AbortSignal.timeout(500) })).ok) break;
			} catch {
				/* One owned process, bounded observation; never restart on an expired read. */
			}
			assert(Date.now() < deadline, `WebView2 CDP endpoint unavailable: ${log}`);
			await delay(100);
		}
		browser = await chromium.connectOverCDP(cdp, { timeout: 5000 });
		const pages = browser.contexts().flatMap((context) => context.pages());
		assert.equal(pages.length, 1, 'owned native app must expose exactly one WebView');
		const page = pages[0];
		page.setDefaultTimeout(5000);
		await page.waitForLoadState('domcontentloaded');
		return { app, browser, page, base, directory, close, log: () => log };
	} catch (error) {
		await close().catch((cleanup) => {
			console.error(`Owned native cleanup: ${cleanup}`);
		});
		throw error;
	}
}

// Run from the repository root: node apps/mapgyver/tests/browser/native-editor.mjs
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
	const native = await openNativeEditor();
	try {
		const errors = [];
		native.page.on('pageerror', (error) => errors.push(String(error)));
		let token;
		async function post(route, body) {
			const response = await fetch(`${native.base}${route}`, {
				method: 'POST',
				signal: AbortSignal.timeout(5000),
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
		token = (await post('/api/ui/session', {})).token;
		const inspect = () =>
			post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.media',
				operation: 'edit',
				payload: { kind: 'InspectManager' }
			});
		const before = await inspect();
		await native.page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		const panel = native.page.locator('section.composition');
		await panel.waitFor();
		assert.deepEqual(await inspect(), before, 'selecting an editor must not author domain state');
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope: 'actual native WebView2 Composition tab input, unchanged catalog',
				pid: native.app.pid,
				browser: native.browser.version(),
				url: native.page.url(),
				panel: await panel.innerText(),
				page_errors: errors
			})
		);
	} finally {
		await native.close();
	}
}
