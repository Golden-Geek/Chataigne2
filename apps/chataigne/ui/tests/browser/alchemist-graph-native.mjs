import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	readFileSync,
	writeFileSync,
	appendFileSync,
	openSync,
	closeSync,
	readdirSync
} from 'node:fs';
import { createServer } from 'node:net';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';

const binary = resolve(process.argv[2] ?? '');
const evidence = resolve(process.argv[3] ?? '');
assert.ok(
	process.argv[2] && process.argv[3],
	'usage: alchemist-graph-native <default-binary> <evidence-dir>'
);
const repository = fileURLToPath(new URL('../../../../../', import.meta.url));
assert.ok(
	relative(resolve(repository, 'target/qualification'), evidence).startsWith(
		'p11-alchemist-native-'
	)
);
const root = fileURLToPath(new URL('./alchemist-graph-fixture/', import.meta.url));
const outDir = resolve(repository, 'target/qualification/p11-alchemist-app');
const report = resolve(evidence, 'browser.jsonl');
writeFileSync(report, '');
const emit = (value) => {
	appendFileSync(report, JSON.stringify(value) + '\n');
	console.log(JSON.stringify(value));
};
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');
await build({
	configFile: false,
	root,
	plugins: [svelte({ configFile: false })],
	logLevel: 'warn',
	resolve: { alias: { '#lib': fileURLToPath(new URL('../../src/lib', import.meta.url)) } },
	build: { outDir, emptyOutDir: true }
});
const server = await preview({
	configFile: false,
	root,
	logLevel: 'warn',
	build: { outDir },
	preview: { host: '127.0.0.1', port: 0, strictPort: true }
});
let browser, child, page;
const errors = [];
const consoleErrors = [];
const wire = [];
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
	closeSync(log);
	let healthy = false;
	for (let attempt = 0; attempt < 600; attempt++) {
		assert.equal(child.exitCode, null, 'Owned headless app exited before readiness');
		try {
			healthy = (await fetch(`${backend}/api/ui/health`, { signal: AbortSignal.timeout(1000) })).ok;
		} catch {
			/* Readiness only. */
		}
		if (healthy) break;
		await new Promise((done) => setTimeout(done, 50));
	}
	assert.ok(healthy, 'Headless readiness deadline expired');
	const credentials = await fetch(`${backend}/api/ui/session`, {
		signal: AbortSignal.timeout(10000),
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: '{}'
	}).then((reply) => reply.json());
	assert.equal(typeof credentials.token, 'string');
	const post = async (path, body) => {
		const reply = await fetch(`${backend}${path}`, {
			signal: AbortSignal.timeout(10000),
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${credentials.token}`
			},
			body: JSON.stringify(body)
		});
		assert.ok(reply.ok, `Native request ${path} failed with ${reply.status}`);
		return reply.json();
	};
	const upload = (file) =>
		post('/api/ui/project-upload-load', {
			file_name: 'p11-alchemist-fixture.noisette',
			contents: readFileSync(file, 'utf8'),
			recover: false
		});
	await upload(resolve(evidence, 'fixture.noisette'));
	browser = await chromium.launch({
		headless: true,
		...(process.env.GOLDEN_UI_BROWSER_PATH
			? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
			: { channel: 'msedge' })
	});
	page = await browser.newPage({ viewport: { width: 1680, height: 1200 } });
	page.on('websocket', (socket) => {
		for (const direction of ['framereceived', 'framesent'])
			socket.on(direction, (event) => {
				try {
					const message = JSON.parse(String(event.payload));
					const kind = message.kind;
					if (!['snapshot', 'subscribe', 'unsubscribe', 'resyncRequired', 'hello'].includes(kind))
						return;
					wire.push({
						direction,
						kind,
						id: message.request_id,
						subscription: message.subscription_id,
						reason: message.reason,
						from: message.from,
						at: message.snapshot?.at
					});
				} catch {}
			});
	});
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('console', (message) => {
		if (message.type() === 'error') consoleErrors.push(message.text());
	});
	await page.goto(`${frontend}/?backend=${encodeURIComponent(backend)}`, {
		waitUntil: 'networkidle'
	});
	await page.waitForFunction(
		() =>
			window.alchemistGraphFixture?.status().connection === 'connected' &&
			window.alchemistGraphFixture.states().length === 2
	);
	const call = (method, ...args) =>
		page.evaluate(({ method, args }) => window.alchemistGraphFixture[method](...args), {
			method,
			args
		});
	const states = await call('states');
	const [first, second] = states;
	assert.deepEqual(
		states.map((node) => node.label),
		['First', 'Second']
	);
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
	assert.equal(await call('reveal', first.id), true);
	assert.equal(await call('reveal', second.id, true), true);
	const history = await call('historyLength');
	assert.equal(await call('move', 'ArrowRight'), true);
	await page.waitForFunction(() =>
		window.alchemistGraphFixture
			.states()
			.every(
				(node, index) =>
					node.position?.kind === 'vec2' && node.position.value[0] === (index ? 26.25 : 2.25)
			)
	);
	assert.equal(await call('historyLength'), history + 1);
	for (const node of await call('states')) {
		const x = await page
			.locator(`article[data-node-id="${node.id}"]`)
			.evaluate((element) => parseFloat(element.style.left));
		assert.equal(x, node.position.value[0]);
	}
	await call('history', 'undo');
	await page.waitForFunction(
		() => window.alchemistGraphFixture.states()[0].position?.value[0] === 2
	);
	await call('history', 'redo');
	await page.waitForFunction(
		() => window.alchemistGraphFixture.states()[0].position?.value[0] === 2.25
	);
	emit({ kind: 'movement-and-history', passed: true, states: await call('states') });
	assert.equal(await call('collapse', first.id), true);
	await page.waitForFunction(() => window.alchemistGraphFixture.states()[0].collapsed);
	assert.equal(await call('startConnection', first.id), true);
	assert.equal((await call('status')).endpoint.nodeId, String(first.id));
	assert.equal(await call('completeConnection', second.id), true);
	await page.waitForFunction(() => window.alchemistGraphFixture.transitions().length === 1);
	const transitions = await call('transitions');
	assert.equal(transitions[0].target.param.value.uuid, second.uuid);
	emit({ kind: 'collapse-and-connection', passed: true, transitions });
	const saved = resolve(evidence, 'roundtrip.noisette');
	await call('save', saved);
	const beforeReload = (await call('status')).generation;
	await upload(saved);
	await page.waitForFunction(
		(previous) => window.alchemistGraphFixture.status().generation > previous,
		beforeReload
	);
	assert.deepEqual(
		(await call('states')).map((node) => [node.uuid, node.position.value, node.collapsed]),
		[
			[first.uuid, [2.25, 3], true],
			[second.uuid, [26.25, 3], false]
		]
	);
	assert.equal((await call('transitions'))[0].uuid, transitions[0].uuid);
	const currentFirst = (await call('states'))[0];
	await call('startConnection', currentFirst.id);
	const previousGeneration = (await call('status')).generation;
	assert.equal(await call('refresh'), true);
	assert.ok((await call('status')).generation > previousGeneration);
	assert.equal(
		(await call('status')).endpoint,
		null,
		'Same-identity snapshot kept a stale endpoint'
	);
	await call('startConnection', currentFirst.id);
	await call('remount');
	assert.equal((await call('status')).endpoint, null);
	emit({ kind: 'save-reload-snapshot-unmount', passed: true });
	const beforeReadOnly = (await call('status')).generation;
	await upload(resolve(evidence, 'readonly.noisette'));
	await page.waitForFunction(
		(previous) =>
			window.alchemistGraphFixture.status().generation > previous &&
			window.alchemistGraphFixture.states()[1].readOnly,
		beforeReadOnly
	);
	const readOnlyStates = await call('states');
	await call('reveal', readOnlyStates[0].id);
	await call('reveal', readOnlyStates[1].id, true);
	const beforeRejected = await call('historyLength');
	assert.equal(await call('move', 'ArrowRight'), true);
	await page.waitForFunction(
		() => window.alchemistGraphFixture.status().message === 'Move request failed.'
	);
	assert.deepEqual(
		(await call('states')).map((node) => node.position.value),
		[
			[2, 3],
			[26, 3]
		]
	);
	assert.equal(await call('historyLength'), beforeRejected);
	for (const node of readOnlyStates) {
		const x = await page
			.locator(`article[data-node-id="${node.id}"]`)
			.evaluate((element) => parseFloat(element.style.left));
		assert.equal(x, node.position.value[0]);
	}
	emit({ kind: 'atomic-readonly-rejection', passed: true });
	assert.deepEqual(errors, []);
	emit({ kind: 'result', passed: true, errors });
} catch (error) {
	const diagnostics = await page
		?.evaluate(async () => ({
			status: window.alchemistGraphFixture?.status(),
			states: window.alchemistGraphFixture?.states(),
			backendLayout: await window.alchemistGraphFixture?.backendLayout(),
			body: document.body.innerText.slice(0, 1000)
		}))
		.catch(() => null);
	emit({
		kind: 'failure',
		message: String(error),
		errors,
		wire,
		diagnostics,
		consoleErrors: consoleErrors.slice(-10)
	});
	throw error;
} finally {
	await browser?.close();
	if (child && child.exitCode === null) {
		const exited = new Promise((done) => child.once('exit', done));
		child.kill();
		await exited;
	}
	await new Promise((done, reject) =>
		server.httpServer.close((error) => (error ? reject(error) : done()))
	);
}
