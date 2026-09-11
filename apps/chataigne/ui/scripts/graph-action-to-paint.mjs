import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';

const repositoryRoot = path.resolve(import.meta.dirname, '../../../..');
const protocolSource = await readFile(
	path.join(repositoryRoot, 'packages/golden-ui/generated/rust_protocol/protocol-version.ts'),
	'utf8'
);
const protocolVersion = protocolSource.match(/UI_PROTOCOL_VERSION = "([^"]+)"/)?.[1];
if (!protocolVersion) {
	throw new Error('generated UI protocol version could not be read');
}

const readArg = (name, fallback) => {
	const index = process.argv.indexOf(`--${name}`);
	return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
};

const uiUrl = readArg('url', 'http://127.0.0.1:4173/');
const reportPath = path.resolve(
	readArg('report', './artifacts/graph-action-to-paint.browser-report.json')
);
const samples = Number(readArg('samples', '20'));
const insertedNodeCount = Number(readArg('inserted-nodes', '600'));
const baseNodeCounts = readArg('base-nodes', '10000,100000').split(',').map(Number);
const timeoutMs = Number(readArg('timeout', '120000'));
const sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], {
	cwd: repositoryRoot,
	encoding: 'utf8'
}).trim();
const sourceDirty =
	execFileSync('git', ['status', '--porcelain', '--untracked-files=normal'], {
		cwd: repositoryRoot,
		encoding: 'utf8'
	}).trim().length > 0;

if (
	!Number.isSafeInteger(samples) ||
	samples < 1 ||
	!Number.isSafeInteger(insertedNodeCount) ||
	insertedNodeCount < 1 ||
	baseNodeCounts.some((count) => !Number.isSafeInteger(count) || count < 1)
) {
	throw new Error('sample and node counts must be positive safe integers');
}

const eventTime = (tick) => ({ tick, micro: 0, seq: 0 });

const nodeMeta = (label) => ({
	label,
	short_name: label,
	enabled: true,
	can_be_disabled: false,
	user_permissions: {
		can_edit_name: false,
		can_remove_and_duplicate: false,
		can_edit_constraints: false,
		can_edit_tags: false,
		can_edit_color: false
	},
	tags: []
});

const graphNode = (nodeId, children) => ({
	node_id: nodeId,
	uuid: `benchmark-node-${nodeId}`,
	decl_id: `node-${nodeId}`,
	node_type: 'benchmark',
	meta: nodeMeta(`Node ${nodeId}`),
	data: { kind: 'node', node_type: 'benchmark' },
	user_role: 'regular',
	user_item_kind: 'benchmark',
	accepted_user_item_kinds: [],
	creatable_user_items: [],
	children
});

const snapshot = (nodeCount) => ({
	protocol_version: protocolVersion,
	scope: { kind: 'wholeGraph' },
	at: eventTime(0),
	nodes: Array.from({ length: nodeCount }, (_, index) => {
		const nodeId = index + 1;
		return graphNode(nodeId, nodeId < nodeCount ? [nodeId + 1] : []);
	}),
	schema: { node_types: [], declared_descriptions: [], enums: [] },
	history: {
		can_undo: false,
		can_redo: false,
		undo_len: 0,
		redo_len: 0,
		active_edit_session: false,
		current_history_state_id: 0
	},
	logger: { max_entries: 100, records: [] },
	project_file: { display_name: 'Graph paint benchmark', extension: 'noisette', current_path: null }
});

const insertionEvent = (tick, parent, firstNode) => {
	const lastNode = firstNode + insertedNodeCount - 1;
	return {
		time: eventTime(tick),
		kind: 'graphTransaction',
		tx_id: tick,
		epoch: 1,
		base_graph_version: tick - 1,
		next_graph_version: tick,
		ops: [
			{
				kind: 'subtreeInserted',
				root: firstNode,
				parent,
				nodes: Array.from({ length: insertedNodeCount }, (_, index) => {
					const nodeId = firstNode + index;
					return graphNode(nodeId, nodeId < lastNode ? [nodeId + 1] : []);
				}),
				parent_children_after: [firstNode]
			}
		]
	};
};

const percentile = (values, quantile) => {
	const sorted = [...values].sort((left, right) => left - right);
	return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * quantile) - 1)];
};

const summarize = (values) => ({
	p50Ms: percentile(values, 0.5),
	p95Ms: percentile(values, 0.95),
	p99Ms: percentile(values, 0.99),
	maxMs: Math.max(...values)
});

const browserExecutable = process.env.GC_UI_BROWSER_EXECUTABLE?.trim();
const browserChannel =
	process.env.GC_UI_BROWSER_CHANNEL?.trim() ||
	(process.platform === 'win32' ? 'msedge' : undefined);
const browser = await chromium.launch({
	headless: true,
	...(browserExecutable ? { executablePath: browserExecutable } : {}),
	...(!browserExecutable && browserChannel ? { channel: browserChannel } : {})
});
const browserVersion = browser.version();
const results = [];

try {
	for (const baseNodeCount of baseNodeCounts) {
		const context = await browser.newContext({
			viewport: { width: 1600, height: 1000 },
			colorScheme: 'dark',
			reducedMotion: 'reduce'
		});
		const page = await context.newPage();
		const browserErrors = [];
		page.on('console', (message) => {
			if (message.type() === 'error') browserErrors.push(message.text());
		});
		page.on('pageerror', (error) => browserErrors.push(error.message));

		let socketRoute;
		let subscriptionId;
		await page.routeWebSocket(/\/api\/ui\/ws$/, (route) => {
			socketRoute = route;
			route.onMessage((raw) => {
				const message = JSON.parse(String(raw));
				if (message.kind === 'hello') {
					route.send(
						JSON.stringify({
							kind: 'hello',
							protocol_version: protocolVersion,
							client_id: 1,
							session_id: `benchmark-${baseNodeCount}`
						})
					);
				} else if (message.kind === 'snapshot') {
					route.send(
						JSON.stringify({
							kind: 'snapshot',
							request_id: message.request_id,
							snapshot: snapshot(baseNodeCount)
						})
					);
				} else if (message.kind === 'subscribe') {
					subscriptionId = message.subscription_id;
				}
			});
		});

		await page.addInitScript(() => {
			globalThis.__gcLongTasks = [];
			new PerformanceObserver((entries) => {
				for (const entry of entries.getEntries()) {
					globalThis.__gcLongTasks.push({ startTime: entry.startTime, duration: entry.duration });
				}
			}).observe({ type: 'longtask', buffered: true });
		});
		await page.goto(uiUrl, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
		await page.locator('.gc-loading-overlay').waitFor({ state: 'hidden', timeout: timeoutMs });
		await page.waitForFunction(
			(expected) =>
				Number(document.querySelector('.gc-main')?.getAttribute('data-graph-node-count')) ===
				expected,
			baseNodeCount,
			{ timeout: timeoutMs }
		);
		const subscriptionDeadline = Date.now() + timeoutMs;
		while (!subscriptionId && Date.now() < subscriptionDeadline) {
			await page.waitForTimeout(10);
		}
		if (!socketRoute || !subscriptionId) {
			throw new Error(`workbench subscription was not ready for ${baseNodeCount} nodes`);
		}
		await page.evaluate(() => {
			globalThis.__gcLongTasks = [];
		});

		const latencies = [];
		let currentNodeCount = baseNodeCount;
		let parent = baseNodeCount;
		for (let sample = 1; sample <= samples; sample += 1) {
			const firstNode = currentNodeCount + 1;
			const targetNodeCount = currentNodeCount + insertedNodeCount;
			await page.evaluate((target) => {
				const root = document.querySelector('.gc-main');
				if (!root) throw new Error('mounted workbench root is missing');
				globalThis.__gcGraphPaintMeasurement = new Promise((resolve) => {
					const startedAt = performance.now();
					const observer = new MutationObserver(() => {
						if (Number(root.getAttribute('data-graph-node-count')) !== target) return;
						observer.disconnect();
						requestAnimationFrame(() =>
							requestAnimationFrame(() => resolve(performance.now() - startedAt))
						);
					});
					observer.observe(root, { attributes: true, attributeFilter: ['data-graph-node-count'] });
				});
			}, targetNodeCount);
			const event = insertionEvent(sample, parent, firstNode);
			socketRoute.send(
				JSON.stringify({
					kind: 'delta',
					subscription_id: subscriptionId,
					deltas: [
						{
							plane: 'structure',
							batch: {
								from: eventTime(sample - 1),
								to: eventTime(sample),
								runtime: null,
								events: [event]
							}
						}
					]
				})
			);
			latencies.push(await page.evaluate(() => globalThis.__gcGraphPaintMeasurement));
			currentNodeCount = targetNodeCount;
			parent = currentNodeCount;
		}

		const longTasks = await page.evaluate(() => globalThis.__gcLongTasks);
		const summary = summarize(latencies);
		const p95TargetMs = baseNodeCount === 10_000 ? 250 : 500;
		const maxLongTaskMs = longTasks.reduce((maximum, task) => Math.max(maximum, task.duration), 0);
		results.push({
			baseNodeCount,
			insertedNodeCount,
			samples,
			latenciesMs: latencies,
			...summary,
			p95TargetMs,
			passed: summary.p95Ms <= p95TargetMs && longTasks.length === 0 && browserErrors.length === 0,
			maxLongTaskMs,
			longTasks,
			browserErrors
		});
		await context.close();
	}
} finally {
	await browser.close();
}

const report = {
	contract: 'golden-ui-graph-action-to-paint-v1',
	source: { commit: sourceCommit, dirty: sourceDirty },
	platform: `${process.platform}-${process.arch}`,
	browser: { channel: browserChannel ?? null, version: browserVersion },
	protocolVersion,
	uiUrl,
	results,
	passed: results.length === baseNodeCounts.length && results.every((result) => result.passed)
};
await mkdir(path.dirname(reportPath), { recursive: true });
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
console.log(JSON.stringify(report, null, 2));
if (!report.passed) process.exitCode = 1;
