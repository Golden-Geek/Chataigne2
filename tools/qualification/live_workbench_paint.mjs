import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright-core';

const [binaryArg, fixtureArg, targetArg, outputArg, sampleArg = '20'] = process.argv.slice(2);
if (!binaryArg || !fixtureArg || !targetArg || !outputArg) {
	throw new Error(
		'usage: node live_workbench_paint.mjs BINARY FIXTURE MINIMUM_NODES OUTPUT_DIR [SAMPLES]'
	);
}
const binary = resolve(binaryArg);
const fixture = resolve(fixtureArg);
const outputDir = resolve(outputArg);
const minimumNodes = Number(targetArg);
const samples = Number(sampleArg);
if (
	!Number.isSafeInteger(minimumNodes) ||
	minimumNodes < 1 ||
	!Number.isSafeInteger(samples) ||
	samples < 1
) {
	throw new Error('minimum nodes and samples must be positive safe integers');
}

const hashFile = async (file) =>
	createHash('sha256')
		.update(await readFile(file))
		.digest('hex');
const pause = (ms) => new Promise((done) => setTimeout(done, ms));
const percentile = (values, quantile) => {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * quantile) - 1)];
};

async function availablePort() {
	const server = createServer();
	await new Promise((done, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', done);
	});
	const port = server.address().port;
	await new Promise((done, reject) => server.close((error) => (error ? reject(error) : done())));
	return port;
}

async function request(baseUrl, route, body) {
	const response = await fetch(`${baseUrl}${route}`, {
		method: body === undefined ? 'GET' : 'POST',
		headers: body === undefined ? {} : { 'content-type': 'application/json' },
		body: body === undefined ? undefined : JSON.stringify(body),
		signal: AbortSignal.timeout(180_000)
	});
	const text = await response.text();
	if (!response.ok) throw new Error(`${route} returned ${response.status}: ${text.slice(0, 500)}`);
	return JSON.parse(text);
}

async function waitForHealth(baseUrl, child) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		if (child.exitCode !== null) throw new Error(`headless app exited with ${child.exitCode}`);
		try {
			const health = await request(baseUrl, '/api/ui/health');
			if (health.backend_ready && health.engine_read_model_ready) return;
		} catch {
			// The listener may not have started yet.
		}
		await pause(100);
	}
	throw new Error('headless app did not become healthy');
}

function chooseDuplicateSources(snapshot, minimumInsertedNodes = 600) {
	const byId = new Map(snapshot.nodes.map((node) => [node.node_id, node]));
	const parentByChild = new Map();
	for (const parent of snapshot.nodes) {
		for (const child of parent.children ?? []) parentByChild.set(child, parent.node_id);
	}
	const roots = snapshot.nodes.filter(
		(node) => typeof node.decl_id === 'string' && node.decl_id.startsWith('scale_constant_')
	);
	const nodes = [];
	let insertedNodes = 0;
	for (const root of roots) {
		const parent = parentByChild.get(root.node_id);
		if (!parent) throw new Error(`authored Constant ${root.decl_id} has no parent`);
		const pending = [root.node_id];
		let subtreeNodes = 0;
		while (pending.length > 0) {
			const node = byId.get(pending.pop());
			if (!node) throw new Error('snapshot has a missing subtree child');
			subtreeNodes += 1;
			pending.push(...(node.children ?? []));
		}
		nodes.push({ source: root.node_id, new_parent: parent });
		insertedNodes += subtreeNodes;
		if (insertedNodes >= minimumInsertedNodes) break;
	}
	if (insertedNodes < minimumInsertedNodes) {
		throw new Error(`only ${insertedNodes} duplicable nodes; need ${minimumInsertedNodes}`);
	}
	return { nodes, insertedNodes };
}

async function run() {
	await mkdir(outputDir, { recursive: true });
	const appData = join(outputDir, 'appdata');
	await mkdir(appData, { recursive: true });
	const port = await availablePort();
	const baseUrl = `http://127.0.0.1:${port}`;
	const child = spawn(binary, ['--headless', '--no-remote'], {
		cwd: resolve('.'),
		env: { ...process.env, APPDATA: appData, GC_UI_BIND: `127.0.0.1:${port}` },
		stdio: ['ignore', 'pipe', 'pipe'],
		windowsHide: true
	});
	let serverLog = '';
	child.stdout.on('data', (chunk) => {
		serverLog += chunk.toString();
	});
	child.stderr.on('data', (chunk) => {
		serverLog += chunk.toString();
	});
	const report = {
		contract: 'chataigne-live-workbench-paint-v1',
		status: 'FAIL',
		minimum_nodes: minimumNodes,
		samples,
		binary_sha256: await hashFile(binary),
		fixture_sha256: await hashFile(fixture),
		browser_version: null,
		base_nodes: null,
		duplicated_roots_per_sample: null,
		inserted_nodes_per_sample: null,
		latencies_ms: [],
		http_ack_ms: [],
		mutation_ms: [],
		outliner_items_per_sample: [],
		outliner_projected_rows_per_sample: [],
		p50_ms: null,
		p95_ms: null,
		p99_ms: null,
		max_ms: null,
		p95_target_ms: minimumNodes === 10_000 ? 250 : 500,
		long_tasks: [],
		browser_errors: [],
		browser_perf_logs: [],
		event_trace: [],
		transport: null,
		error: null
	};
	let browser;
	try {
		await waitForHealth(baseUrl, child);
		const browserExecutable = process.env.GC_UI_BROWSER_EXECUTABLE?.trim();
		const browserChannel =
			process.env.GC_UI_BROWSER_CHANNEL?.trim() ||
			(process.platform === 'win32' ? 'msedge' : undefined);
		browser = await chromium.launch({
			headless: true,
			...(browserExecutable ? { executablePath: browserExecutable } : {}),
			...(!browserExecutable && browserChannel ? { channel: browserChannel } : {})
		});
		report.browser_version = browser.version();
		const page = await browser.newPage({
			viewport: { width: 1600, height: 1000 },
			reducedMotion: 'reduce'
		});
		let captureEventTrace = false;
		if (process.env.GC_CAPTURE_EVENT_TRACE === '1') {
			page.on('websocket', (socket) => {
				socket.on('framereceived', ({ payload }) => {
					if (
						!captureEventTrace ||
						typeof payload !== 'string' ||
						!payload.startsWith('{"kind":"delta"')
					)
						return;
					const message = JSON.parse(payload);
					for (const delta of message.deltas ?? []) {
						for (const event of delta.batch?.events ?? []) {
							if (report.event_trace.length >= 1000) return;
							if (event.kind === 'paramChanged') {
								report.event_trace.push({
									plane: delta.plane,
									time: event.time,
									kind: 'paramChanged',
									param: event.param
								});
							} else if (event.kind === 'graphTransaction') {
								report.event_trace.push({
									plane: delta.plane,
									time: event.time,
									kind: 'graphTransaction',
									ops: event.ops?.map((op) => ({
										kind: op.kind,
										root: op.root,
										inserted: op.nodes?.length
									}))
								});
							}
						}
					}
				});
			});
		}
		page.on('console', (message) => {
			if (message.type() === 'error') report.browser_errors.push(message.text());
			if (
				message.text().startsWith('[ui ws]') ||
				message.text().startsWith('[ui graph]') ||
				message.text().startsWith('[ui-perf] [ui] snapshot') ||
				message.text().startsWith('[ui-perf] [ui] batch events=')
			) {
				if (report.browser_perf_logs.length < 256) report.browser_perf_logs.push(message.text());
			}
		});
		page.on('pageerror', (error) => report.browser_errors.push(error.message));
		await page.addInitScript((profileUi) => {
			if (profileUi) {
				localStorage.setItem('gc_ui_perf', '1');
				localStorage.setItem('gc_perf_profiler', '1');
			}
			globalThis.__gcLongTasks = [];
			new PerformanceObserver((entries) => {
				for (const entry of entries.getEntries()) {
					globalThis.__gcLongTasks.push({ start_ms: entry.startTime, duration_ms: entry.duration });
				}
			}).observe({ type: 'longtask', buffered: true });
		}, process.env.GC_UI_PERF_TRACE === '1');
		await page.goto(`${baseUrl}/?gc_debug_runtime=0`, {
			waitUntil: 'domcontentloaded',
			timeout: 180_000
		});
		await page.locator('.gc-loading-overlay').waitFor({ state: 'hidden', timeout: 180_000 });
		await request(baseUrl, '/api/ui/project-load', { path: fixture, recover: false });
		await page.waitForFunction(
			(target) =>
				Number(document.querySelector('.gc-main')?.getAttribute('data-graph-node-count')) >= target,
			minimumNodes,
			{ timeout: 180_000 }
		);
		const snapshot = await request(baseUrl, '/api/ui/snapshot', { scope: 'wholeGraph' });
		report.base_nodes = snapshot.nodes.length;
		report.base_dom = await page.evaluate(() => ({
			outliner_items: document.querySelectorAll('.outliner-item').length,
			outliner_projected_rows: Number(
				document.querySelector('.outliner-tree')?.getAttribute('data-outliner-row-count')
			),
			canvas_visible_nodes: Array.from(document.querySelectorAll('[data-visible-node-count]')).map(
				(node) => Number(node.getAttribute('data-visible-node-count'))
			)
		}));
		const duplicate = chooseDuplicateSources(snapshot);
		report.duplicated_roots_per_sample = duplicate.nodes.length;
		report.inserted_nodes_per_sample = duplicate.insertedNodes;
		await page.evaluate(() => {
			globalThis.__gcLongTasks = [];
		});
		for (let sample = 0; sample < samples; sample += 1) {
			captureEventTrace = true;
			let cpuProfiler;
			if (sample === 0 && process.env.GC_CAPTURE_CPU_PROFILE === '1') {
				cpuProfiler = await page.context().newCDPSession(page);
				await cpuProfiler.send('Profiler.enable');
				await cpuProfiler.send('Profiler.setSamplingInterval', { interval: 1000 });
				await cpuProfiler.send('Profiler.start');
			}
			const baseCount = await page.evaluate(() =>
				Number(document.querySelector('.gc-main')?.getAttribute('data-graph-node-count'))
			);
			await page.evaluate((base) => {
				const root = document.querySelector('.gc-main');
				if (!root) throw new Error('workbench root disappeared');
				globalThis.__gcPaintMeasurement = new Promise((resolve, reject) => {
					const started = performance.now();
					const timer = setTimeout(() => {
						observer.disconnect();
						reject(new Error('graph projection timed out'));
					}, 180_000);
					const observer = new MutationObserver(() => {
						const count = Number(root.getAttribute('data-graph-node-count'));
						if (count <= base) return;
						const mutationMs = performance.now() - started;
						observer.disconnect();
						clearTimeout(timer);
						requestAnimationFrame(() =>
							requestAnimationFrame(() =>
								resolve({
									latency_ms: performance.now() - started,
									mutation_ms: mutationMs,
									node_count: count
								})
							)
						);
					});
					observer.observe(root, { attributes: true, attributeFilter: ['data-graph-node-count'] });
				});
			}, baseCount);
			const httpStarted = performance.now();
			const acknowledgement = await request(baseUrl, '/api/ui/intent', {
				kind: 'duplicateNodes',
				nodes: duplicate.nodes
			});
			report.http_ack_ms.push(performance.now() - httpStarted);
			if (acknowledgement.success !== true) {
				throw new Error(
					`batch duplicate rejected: ${JSON.stringify(acknowledgement).slice(0, 500)}`
				);
			}
			const painted = await page.evaluate(() => globalThis.__gcPaintMeasurement);
			if (painted.node_count !== baseCount + duplicate.insertedNodes) {
				throw new Error(
					`expected ${baseCount + duplicate.insertedNodes} nodes after batch; saw ${painted.node_count}`
				);
			}
			report.latencies_ms.push(painted.latency_ms);
			report.mutation_ms.push(painted.mutation_ms);
			report.outliner_items_per_sample.push(await page.locator('.outliner-item').count());
			report.outliner_projected_rows_per_sample.push(
				Number(await page.locator('.outliner-tree').getAttribute('data-outliner-row-count'))
			);
			if (cpuProfiler) {
				const { profile } = await cpuProfiler.send('Profiler.stop');
				await writeFile(join(outputDir, 'browser-action.cpuprofile'), JSON.stringify(profile));
				await cpuProfiler.detach();
			}
		}
		if (process.env.GC_VERIFY_OUTLINER_SCROLL === '1') {
			const before = await page.evaluate(() => {
				const scroller = document.querySelector('.outliner-content');
				if (!(scroller instanceof HTMLElement)) throw new Error('outliner scroller is missing');
				const first = scroller.querySelector('.outliner-item-content[data-node-id]');
				const firstId = first?.getAttribute('data-node-id');
				if (!firstId || scroller.scrollHeight <= scroller.clientHeight) {
					throw new Error('outliner has no scrollable visible rows');
				}
				return {
					firstId,
					scrollHeight: scroller.scrollHeight,
					clientHeight: scroller.clientHeight
				};
			});
			await page.evaluate(() => {
				const scroller = document.querySelector('.outliner-content');
				if (!(scroller instanceof HTMLElement)) throw new Error('outliner scroller is missing');
				scroller.scrollTop = scroller.scrollHeight / 2;
			});
			await page.waitForFunction(
				(firstId) =>
					document
						.querySelector('.outliner-content .outliner-item-content[data-node-id]')
						?.getAttribute('data-node-id') !== firstId,
				before.firstId,
				{ timeout: 30_000 }
			);
			const after = await page.evaluate(() => {
				const scroller = document.querySelector('.outliner-content');
				if (!(scroller instanceof HTMLElement)) throw new Error('outliner scroller is missing');
				return {
					firstId: scroller
						.querySelector('.outliner-item-content[data-node-id]')
						?.getAttribute('data-node-id'),
					items: scroller.querySelectorAll('.outliner-item').length,
					scrollTop: scroller.scrollTop
				};
			});
			if (after.items > 200 || after.scrollTop <= 0) {
				throw new Error(`outliner scroll window is unbounded: ${JSON.stringify(after)}`);
			}
			report.outliner_scroll_probe = { before, after, status: 'PASS' };
		}
		if (process.env.GC_VERIFY_OUTLINER_META === '1') {
			await page.evaluate(() => {
				const scroller = document.querySelector('.outliner-content');
				if (!(scroller instanceof HTMLElement)) throw new Error('outliner scroller is missing');
				scroller.scrollTop = 0;
			});
			if (report.outliner_scroll_probe) {
				await page.waitForFunction(
					(firstId) =>
						document
							.querySelector('.outliner-content .outliner-item-content[data-node-id]')
							?.getAttribute('data-node-id') === firstId,
					report.outliner_scroll_probe.before.firstId,
					{ timeout: 30_000 }
				);
			}
			const mountedIds = await page.evaluate(() =>
				Array.from(document.querySelectorAll('.outliner-item-content[data-node-id]')).map((row) =>
					Number(row.getAttribute('data-node-id'))
				)
			);
			const nodesById = new Map(snapshot.nodes.map((node) => [node.node_id, node]));
			const source = mountedIds.find(
				(nodeId) => nodesById.get(nodeId)?.meta?.user_permissions?.can_edit_name === true
			);
			if (source === undefined) throw new Error('no editable outliner row is mounted');
			const label = '__outliner_projection_probe__';
			const acknowledgement = await request(baseUrl, '/api/ui/intent', {
				kind: 'patchMeta',
				node: source,
				patch: { label }
			});
			if (acknowledgement.success !== true) {
				throw new Error(`outliner metadata probe rejected: ${JSON.stringify(acknowledgement)}`);
			}
			await page.waitForFunction(
				({ source, label }) =>
					document
						.querySelector(`.outliner-item-content[data-node-id="${source}"] .outliner-item-label`)
						?.textContent?.trim() === label,
				{ source, label },
				{ timeout: 30_000 }
			);
			report.outliner_meta_probe = { source, label, status: 'PASS' };
		}
		if (process.env.GC_CAPTURE_OUTLINER_SCREENSHOT === '1') {
			await page.locator('.outliner-content').screenshot({
				path: join(outputDir, 'outliner-viewport.png')
			});
		}
		report.long_tasks = await page.evaluate(() => globalThis.__gcLongTasks);
		report.p50_ms = percentile(report.latencies_ms, 0.5);
		report.p95_ms = percentile(report.latencies_ms, 0.95);
		report.p99_ms = percentile(report.latencies_ms, 0.99);
		report.max_ms = Math.max(...report.latencies_ms);
		if (report.browser_errors.length > 0) throw new Error('browser reported errors');
		if (report.p95_ms > report.p95_target_ms)
			throw new Error('p95 exceeded the provisional workbench budget');
		report.status = 'PASS';
	} catch (error) {
		report.error = String(error?.stack ?? error);
	} finally {
		await browser?.close().catch(() => {});
		if (child.exitCode === null) {
			child.kill();
			await Promise.race([new Promise((done) => child.once('exit', done)), pause(5_000)]);
		}
		report.transport = {
			slow_client_disconnects: (serverLog.match(/disconnecting slow client/g) ?? []).length,
			overflow_resyncs: (serverLog.match(/pausing overloaded subscription/g) ?? []).length,
			ws_snapshots: (serverLog.match(/\[ui-ws\] snapshot request_id=/g) ?? []).length
		};
		await writeFile(join(outputDir, 'headless-server.log'), serverLog);
		await writeFile(
			join(outputDir, 'workbench-paint-report.json'),
			`${JSON.stringify(report, null, 2)}\n`
		);
	}
	return report;
}

const report = await run();
console.log(JSON.stringify({ status: report.status, p95_ms: report.p95_ms, error: report.error }));
if (report.status !== 'PASS') process.exitCode = 1;
