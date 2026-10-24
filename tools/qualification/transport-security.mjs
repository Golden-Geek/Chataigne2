import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { createServer, request } from 'node:http';
import { networkInterfaces, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { UI_PROTOCOL_VERSION } from '../../packages/golden-ui/generated/rust_protocol/protocol-version.ts';

// Network/DOM qualification only: no keyboard, pointer, visible windows or native commands.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [binaryArg, browserArg, outputArg] = process.argv.slice(2);
if (!binaryArg || !browserArg || !outputArg) {
	throw new Error('usage: node transport-security.mjs BINARY HEADLESS_BROWSER OUTPUT_JSON');
}
const binary = resolve(binaryArg);
const output = resolve(outputArg);
const sandbox = await mkdtemp(join(tmpdir(), 'chataigne-security-'));
const report = {
	started: new Date().toISOString(),
	binarySha256: createHash('sha256')
		.update(await readFile(binary))
		.digest('hex'),
	sandbox,
	surfaces: [],
	nativeWebview: 'Not launched; native desktop interaction remains P17 qualification.'
};
const processes = new Set();
const pause = (ms) => new Promise((done) => setTimeout(done, ms));

function child(command, args, options = {}) {
	const process = spawn(command, args, {
		windowsHide: true,
		stdio: ['ignore', 'pipe', 'pipe'],
		...options
	});
	process.diagnostics = '';
	for (const stream of [process.stdout, process.stderr]) {
		stream.on('data', (data) => {
			process.diagnostics = (process.diagnostics + data).slice(-16000);
		});
	}
	process.on('error', (error) => {
		process.launchError = error;
	});
	processes.add(process);
	return process;
}

async function stop(process) {
	if (process.exitCode === null && process.signalCode === null) {
		const closed = once(process, 'close');
		process.kill();
		await closed;
	}
	processes.delete(process);
}

async function availablePort() {
	const server = createServer();
	server.listen(0, '127.0.0.1');
	await once(server, 'listening');
	const port = server.address().port;
	await new Promise((done) => server.close(done));
	return port;
}

async function waitFor(probe, label, process) {
	const deadline = Date.now() + 60000;
	let last;
	while (Date.now() < deadline) {
		if (process?.launchError) throw process.launchError;
		if (process && process.exitCode !== null) {
			throw new Error(`${label}: child exited ${process.exitCode}\n${process.diagnostics}`);
		}
		try {
			if (await probe()) return;
		} catch (error) {
			last = error;
		}
		await pause(100);
	}
	throw new Error(`${label}: timed out (${last?.message ?? 'not ready'})`);
}

async function api(base, path, token, body, headers = {}) {
	return fetch(`${base}/api/ui/${path}`, {
		method: body === undefined ? 'GET' : 'POST',
		headers: {
			...(body === undefined ? {} : { 'content-type': 'application/json' }),
			...(token ? { authorization: `Bearer ${token}` } : {}),
			...headers
		},
		body: body === undefined ? undefined : JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
}

async function startHost(port, frontend) {
	const env = { ...process.env };
	for (const key of Object.keys(env)) {
		if (key.startsWith('GC_UI_') || key === 'GC_PROJECT') delete env[key];
	}
	for (const key of ['APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME']) {
		env[key] = join(sandbox, key);
		await mkdir(env[key], { recursive: true });
	}
	env.GC_UI_BIND = `0.0.0.0:${port}`;
	if (frontend) env.GC_UI_FRONTEND_URL = frontend;
	const host = child(binary, ['--headless'], { cwd: root, env });
	const base = `http://127.0.0.1:${port}`;
	await waitFor(
		async () => {
			const health = await (await api(base, 'health')).json();
			return health.backend_ready && health.engine_read_model_ready;
		},
		'headless host',
		host
	);
	return host;
}

async function websocket(url, protocols, message) {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url, protocols);
		const timer = setTimeout(() => {
			socket.close();
			reject(new Error('WebSocket timed out'));
		}, 5000);
		const done = (value) => {
			clearTimeout(timer);
			socket.close();
			resolve(value);
		};
		socket.onerror = () => done({ admitted: false });
		socket.onopen = () => {
			if (message) socket.send(JSON.stringify(message));
			else done({ admitted: true, protocol: socket.protocol });
		};
		socket.onmessage = (event) => {
			const data = JSON.parse(event.data);
			if (data.request_id === message?.request_id) done({ admitted: true, message: data });
		};
	});
}

async function nativeSurface(base) {
	const ws = `${base.replace('http:', 'ws:')}/api/ui/ws`;
	assert.equal((await api(base, 'project-save', null, 'invalid payload')).status, 401);
	assert.equal(
		(await api(base, 'session', null, {}, { origin: 'http://unapproved.invalid' })).status,
		403
	);
	const wrongHost = await new Promise((resolve, reject) => {
		const req = request(
			`${base}/api/ui/health`,
			{ headers: { host: `unapproved.invalid:${new URL(base).port}` } },
			(response) => {
				response.resume();
				resolve(response.statusCode);
			}
		);
		req.on('error', reject);
		req.end();
	});
	assert.equal(wrongHost, 403);
	assert.equal((await websocket(ws)).admitted, false);
	const readonly = await (
		await api(base, 'session', null, {
			capabilities: { observe: true, author: false, project_files: false, scripts: false }
		})
	).json();
	assert.ok(readonly.token);
	assert.equal((await api(base, 'snapshot', readonly.token, { scope: 'wholeGraph' })).status, 200);
	for (const route of ['intent', 'project-save', 'project-load', 'script-state', 'script-config']) {
		assert.equal((await api(base, route, readonly.token, 'invalid payload')).status, 403);
	}
	const denied = await websocket(ws, readonly.websocket_protocols, {
		kind: 'intent',
		request_id: 'denied',
		intent: { kind: 'reevaluateGraph' },
		include_self_events: false
	});
	assert.equal(denied.message.kind, 'error');
	assert.equal((await api(base, 'session/revoke', readonly.token, {})).status, 204);
	assert.equal((await api(base, 'snapshot', readonly.token, { scope: 'wholeGraph' })).status, 401);
	assert.equal((await websocket(ws, readonly.websocket_protocols)).admitted, false);
	const session = await (await api(base, 'session', null, { capabilities: null })).json();
	assert.equal(session.protocol_version, UI_PROTOCOL_VERSION);
	assert.equal((await websocket(ws, session.websocket_protocols)).protocol, 'golden-ui');
	return session.token;
}

async function browserSurface(browser, name, frontend, backend) {
	const context = await browser.newContext();
	try {
		const page = await context.newPage();
		const errors = [];
		const tokens = [];
		const pending = [];
		page.on('pageerror', (error) => errors.push(error.message));
		page.on('response', (response) => {
			if (response.url() === `${backend}/api/ui/session` && response.status() === 200) {
				pending.push(response.json().then((body) => tokens.push(body.token)));
			}
		});
		await page.addInitScript(() => {
			window.qualificationViolations = [];
			document.addEventListener('securitypolicyviolation', (event) => {
				window.qualificationViolations.push({
					directive: event.effectiveDirective,
					blocked: event.blockedURI
				});
			});
		});
		let policy;
		for (let reload = 0; reload < 4; reload += 1) {
			const response = reload ? await page.reload() : await page.goto(frontend);
			assert.equal(
				response.status(),
				200,
				`${name}: HTML response: ${(await response.text()).slice(0, 1500)}`
			);
			policy = response.headers()['content-security-policy'];
			assert.ok(policy, `${name}: HTML CSP header missing`);
			assert.ok(!policy.match(/script-src[^;]*(unsafe-inline|unsafe-eval)/));
			await page.waitForSelector('.gc-main', { timeout: 60000 });
			await waitFor(async () => {
				const health = await (await api(backend, 'health')).json();
				return health.active_subscribed_websocket_clients > 0;
			}, `${name}: authenticated subscription`);
			await Promise.all(pending);
			assert.ok(tokens.length >= reload + 1, `${name}: missing bootstrap on reload`);
			assert.equal(new Set(tokens).size, 1, `${name}: reload leaked a session slot`);
			assert.deepEqual(
				await page.evaluate(() => window.qualificationViolations),
				[],
				`${name}: unexpected CSP violation`
			);
		}
		assert.deepEqual(errors, [], `${name}: runtime errors`);
		const privilege = await page.evaluate(async (backend) => {
			const token = sessionStorage.getItem(`golden-ui-session:${backend}/api/ui`);
			const response = await fetch(`${backend}/api/ui/snapshot`, {
				method: 'POST',
				headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
				body: JSON.stringify({ scope: 'wholeGraph' })
			});
			return { status: response.status, hasSnapshot: Array.isArray((await response.json()).nodes) };
		}, backend);
		assert.deepEqual(privilege, { status: 200, hasSnapshot: true });
		await page.evaluate(() => {
			const inline = document.createElement('script');
			inline.textContent = 'window.qualificationInjected = true';
			document.head.append(inline);
			const data = document.createElement('script');
			data.src = 'data:text/javascript,window.qualificationInjected=true';
			document.head.append(data);
		});
		await waitFor(
			() => page.evaluate(() => window.qualificationViolations.length >= 2),
			`${name}: CSP probes`
		);
		assert.equal(await page.evaluate(() => window.qualificationInjected === true), false);
		report.surfaces.push({
			name,
			frontend,
			backend,
			reloads: 3,
			reusedSession: true,
			snapshot: true,
			subscribed: true,
			blockedInjectedScripts: 2,
			runtimeErrors: errors
		});
	} finally {
		await context.close();
	}
}

async function unapprovedBrowser(browser, backend) {
	const server = createServer((_request, response) => {
		response.setHeader('content-type', 'text/html');
		response.end('<!doctype html><title>Unapproved origin probe</title>');
	});
	server.listen(0, '127.0.0.1');
	await once(server, 'listening');
	const context = await browser.newContext();
	try {
		const page = await context.newPage();
		await page.goto(`http://127.0.0.1:${server.address().port}`);
		const denied = await page.evaluate(async (backend) => {
			const http = await fetch(`${backend}/api/ui/session`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: '{}'
			}).then(
				() => false,
				() => true
			);
			const websocket = await new Promise((resolve, reject) => {
				const socket = new WebSocket(`${backend.replace('http:', 'ws:')}/api/ui/ws`);
				const timer = setTimeout(() => {
					socket.close();
					reject(new Error('WS denial timed out'));
				}, 5000);
				socket.onerror = () => {
					clearTimeout(timer);
					resolve(true);
				};
				socket.onopen = () => {
					clearTimeout(timer);
					socket.close();
					resolve(false);
				};
			});
			return { http, websocket };
		}, backend);
		assert.deepEqual(denied, { http: true, websocket: true });
		report.surfaces.push({ name: 'unapproved browser origin', ...denied });
	} finally {
		await context.close();
		server.closeAllConnections();
		await new Promise((done) => server.close(done));
	}
}

let browser;
let host;
let devServer;
try {
	const port = await availablePort();
	const devPort = await availablePort();
	const backend = `http://127.0.0.1:${port}`;
	const frontend = `http://127.0.0.1:${devPort}`;
	host = await startHost(port, frontend);
	const stale = await nativeSurface(backend);
	report.surfaces.push({ name: 'native HTTP/WS', admittedAndDenied: true });
	browser = await chromium.launch({ executablePath: resolve(browserArg), headless: true });
	report.browserVersion = browser.version();
	await browserSurface(browser, 'served loopback', backend, backend);
	await unapprovedBrowser(browser, backend);
	const lan = Object.values(networkInterfaces())
		.flat()
		.find(
			(nic) =>
				!nic.internal &&
				nic.family === 'IPv4' &&
				/^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(nic.address)
		);
	if (lan) {
		const origin = `http://${lan.address}:${port}`;
		await browserSurface(browser, 'served local LAN interface', origin, origin);
	} else {
		report.surfaces.push({ name: 'served LAN', status: 'unavailable: no private IPv4 interface' });
	}
	// Launch Vite directly, so cleanup owns one process and never needs a desktop or shell.
	devServer = child(
		process.execPath,
		[
			join(root, 'node_modules/vite/bin/vite.js'),
			'--host',
			'127.0.0.1',
			'--port',
			String(devPort),
			'--strictPort'
		],
		{
			cwd: join(root, 'apps/chataigne/ui'),
			env: { ...process.env, VITE_GC_UI_BACKEND_PORT: String(port) }
		}
	);
	await waitFor(
		async () => (await fetch(frontend, { signal: AbortSignal.timeout(5000) })).ok,
		'Vite',
		devServer
	);
	await browserSurface(browser, 'configured development frontend', frontend, backend);
	await stop(devServer);
	await stop(host);
	host = await startHost(port, frontend);
	assert.equal((await api(backend, 'snapshot', stale, { scope: 'wholeGraph' })).status, 401);
	await nativeSurface(backend);
	report.surfaces.push({ name: 'host restart', staleTokenRejected: true, freshSessionWorks: true });
	report.passed = true;
} catch (error) {
	report.passed = false;
	report.error = error.stack;
	if (devServer) report.developmentDiagnostics = devServer.diagnostics;
	if (host?.exitCode !== null) report.hostDiagnostics = host?.diagnostics;
	process.exitCode = 1;
} finally {
	await browser?.close();
	for (const process of processes) await stop(process);
	report.finished = new Date().toISOString();
	await mkdir(dirname(output), { recursive: true });
	await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
	console.log(JSON.stringify(report, null, 2));
}
