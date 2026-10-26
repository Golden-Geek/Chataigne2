import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import {
	appendFileSync,
	closeSync,
	mkdirSync,
	openSync,
	readFileSync,
	readdirSync,
	writeFileSync
} from 'node:fs';
import { createServer } from 'node:net';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';

const repository = fileURLToPath(new URL('../../../../../', import.meta.url));
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');

/** Owns isolated app data, loopback host, production fixture and browser; never opens the desktop. */
export async function nativeWorkbench({ fixture, name, exercise, observePage }) {
	assert.match(name, /^[a-z][a-z0-9-]*$/);
	const binary = resolve(repository, process.argv[2] ?? 'target/debug/Chataigne2.exe');
	const evidence = resolve(repository, 'target/qualification', `${name}-${randomUUID()}`);
	const outDir = resolve(repository, 'target/qualification', `${name}-app`);
	const root = fileURLToPath(fixture);
	mkdirSync(evidence, { recursive: true });
	const report = resolve(evidence, 'browser.jsonl');
	const emit = (value) => {
		appendFileSync(report, JSON.stringify(value) + '\n');
		console.log(JSON.stringify(value));
	};
	await build({
		configFile: false,
		root,
		plugins: [svelte({ configFile: false })],
		logLevel: 'warn',
		resolve: { alias: { '#lib': fileURLToPath(new URL('../../src/lib', import.meta.url)) } },
		build: { outDir, emptyOutDir: true, sourcemap: process.env.GC_GRAPH_SCALE_CPU_PROFILE === '1' }
	});
	if (process.env.GC_GRAPH_SCALE_CPU_PROFILE === '1') {
		for (const name of readdirSync(resolve(outDir, 'assets'))) {
			if (name.endsWith('.map'))
				writeFileSync(resolve(evidence, name), readFileSync(resolve(outDir, 'assets', name)));
		}
	}
	const server = await preview({
		configFile: false,
		root,
		logLevel: 'warn',
		build: { outDir },
		preview: { host: '127.0.0.1', port: 0, strictPort: true }
	});
	let child, browser, page, startupError;
	const errors = [];
	try {
		const frontend = `http://127.0.0.1:${server.httpServer.address().port}`;
		const reservation = createServer();
		await new Promise((done) => reservation.listen(0, '127.0.0.1', done));
		const port = reservation.address().port;
		await new Promise((done) => reservation.close(done));
		const backend = `http://127.0.0.1:${port}`;
		const log = openSync(resolve(evidence, 'headless.log'), 'w');
		child = spawn(binary, ['--headless', '--no-frontend', '--no-remote'], {
			windowsHide: true,
			stdio: ['ignore', log, log],
			env: {
				...process.env,
				GC_UI_BIND: `127.0.0.1:${port}`,
				GC_UI_FRONTEND_URL: frontend,
				APPDATA: evidence,
				LOCALAPPDATA: evidence,
				USERPROFILE: evidence,
				XDG_DATA_HOME: evidence
			}
		});
		child.on('error', (error) => {
			startupError = error;
		});
		closeSync(log);
		let healthy = false;
		const deadline = Date.now() + 30000;
		while (!healthy && Date.now() < deadline) {
			assert.ifError(startupError);
			assert.equal(child.exitCode, null, 'Owned app exited before readiness');
			try {
				healthy = (await fetch(`${backend}/api/ui/health`, { signal: AbortSignal.timeout(1000) }))
					.ok;
			} catch {
				/* Readiness only. */
			}
			if (!healthy) await new Promise((done) => setTimeout(done, 50));
		}
		assert.ok(healthy, 'Owned app readiness deadline expired');
		browser = await chromium.launch({
			headless: true,
			...(process.env.GOLDEN_UI_BROWSER_PATH
				? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
				: { channel: 'msedge' })
		});
		page = await browser.newPage({ viewport: { width: 1680, height: 1200 } });
		page.on('pageerror', (error) => errors.push(error.message));
		await observePage?.({ page });
		emit({
			kind: 'environment',
			production: true,
			browser: await browser.version(),
			binaryHash: hash(binary),
			assets: readdirSync(resolve(outDir, 'assets')).map((name) => ({
				name,
				sha256: hash(resolve(outDir, 'assets', name))
			})),
			report
		});
		await page.goto(`${frontend}/?backend=${encodeURIComponent(backend)}`, {
			waitUntil: 'networkidle'
		});
		await exercise({ page, evidence, emit });
		assert.deepEqual(errors, []);
		emit({ kind: 'result', passed: true, errors });
	} catch (error) {
		emit({
			kind: 'failure',
			message: String(error),
			errors,
			body: await page?.evaluate(() => document.body.innerText.slice(0, 2000)).catch(() => null)
		});
		throw error;
	} finally {
		await browser?.close();
		if (child?.pid && child.exitCode === null && child.signalCode === null) {
			const exited = new Promise((done) => child.once('exit', done));
			child.kill();
			await exited;
		}
		await new Promise((done, reject) =>
			server.httpServer.close((error) => (error ? reject(error) : done()))
		);
	}
}
