import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';

const defaultUiUrl = process.env.GC_UI_URL ?? 'http://127.0.0.1:4173/?gc_debug_runtime=1';
const defaultSelector = process.env.GC_UI_READY_SELECTOR ?? 'body';
const defaultTimeoutMs = Number(process.env.GC_UI_TIMEOUT_MS ?? 15000);
const defaultSettledWaitMs = Number(process.env.GC_UI_SETTLE_MS ?? 1200);
const defaultInspectStyles = [
	'display',
	'position',
	'inline-size',
	'block-size',
	'width',
	'height',
	'opacity',
	'transform',
	'overflow',
	'z-index'
];

const [, , command = 'smoke', ...rawArgs] = process.argv;

const parseArgs = (args) => {
	const parsed = new Map();
	for (let index = 0; index < args.length; index += 1) {
		const token = args[index];
		if (!token.startsWith('--')) {
			continue;
		}
		const key = token.slice(2);
		const value = args[index + 1] && !args[index + 1].startsWith('--') ? args[++index] : 'true';
		const existing = parsed.get(key);
		if (existing === undefined) {
			parsed.set(key, [value]);
			continue;
		}
		existing.push(value);
	}
	return parsed;
};

const args = parseArgs(rawArgs);

const getArgValue = (key, fallback) => args.get(key)?.at(-1) ?? fallback;
const getArgValues = (key) => args.get(key) ?? [];
const getNumberArg = (key, fallback) => {
	const value = Number(getArgValue(key, fallback));
	return Number.isFinite(value) && value > 0 ? value : fallback;
};
const getCsvArg = (key, fallback) => {
	const rawValue = getArgValue(key, '');
	if (!rawValue) {
		return fallback;
	}
	return rawValue
		.split(',')
		.map((value) => value.trim())
		.filter((value) => value.length > 0);
};

const matchesIgnoredPattern = (value, patterns) => {
	const normalizedValue = value.toLowerCase();
	return patterns.some((pattern) => normalizedValue.includes(pattern.toLowerCase()));
};

const toSerializableError = (error) => ({
	message: error instanceof Error ? error.message : String(error),
	stack: error instanceof Error ? (error.stack ?? null) : null
});

const resolveBrowserLaunchOptions = () => {
	const executablePath = process.env.GC_UI_BROWSER_EXECUTABLE?.trim();
	const channel =
		process.env.GC_UI_BROWSER_CHANNEL?.trim() ||
		(process.platform === 'win32' ? 'msedge' : undefined);
	const options = {
		headless: process.env.GC_UI_HEADFUL !== '1'
	};
	if (executablePath) {
		return {
			...options,
			executablePath
		};
	}
	if (channel) {
		return {
			...options,
			channel
		};
	}
	return options;
};

const createIssueCollector = () => ({
	consoleErrors: [],
	pageErrors: [],
	requestFailures: [],
	httpErrors: []
});

const attachIssueCollectors = (page, issues) => {
	page.on('console', (message) => {
		if (message.type() !== 'error') {
			return;
		}
		issues.consoleErrors.push({
			text: message.text(),
			location: message.location()
		});
	});
	page.on('pageerror', (error) => {
		issues.pageErrors.push(toSerializableError(error));
	});
	page.on('requestfailed', (request) => {
		issues.requestFailures.push({
			url: request.url(),
			method: request.method(),
			errorText: request.failure()?.errorText ?? 'Unknown request failure'
		});
	});
	page.on('response', (response) => {
		if (response.status() < 400) {
			return;
		}
		issues.httpErrors.push({
			url: response.url(),
			method: response.request().method(),
			status: response.status(),
			statusText: response.statusText()
		});
	});
};

const openAndSettlePage = async (page, url, selector, timeoutMs, settledWaitMs) => {
	await page.goto(url, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
	await page.waitForSelector(selector, { timeout: timeoutMs });
	await page.waitForTimeout(settledWaitMs);
	return page.title();
};

const readRuntimeProbeEntries = async (page) => {
	try {
		return await page.evaluate(() => window.__GC_RUNTIME_PROBE__?.snapshot?.() ?? []);
	} catch {
		return [];
	}
};

const waitForRuntimeReady = async (page, timeoutMs) => {
	await page.locator('.gc-loading-overlay').waitFor({ state: 'hidden', timeout: timeoutMs });
	await page.waitForFunction(
		() => {
			const header = document.querySelector('[role="banner"]');
			return (
				(document.body?.innerText?.trim().length ?? 0) > 100 &&
				(header?.textContent ?? '').toLowerCase().includes('connected')
			);
		},
		undefined,
		{ timeout: timeoutMs }
	);
};

const installProjectUploadObserver = async (page) => {
	await page.addInitScript(() => {
		const originalFetch = globalThis.fetch.bind(globalThis);
		globalThis.fetch = async (input, init) => {
			const response = await originalFetch(input, init);
			const requestUrl = typeof input === 'string' ? input : input.url;
			const requestMethod = init?.method ?? (typeof input === 'string' ? 'GET' : input.method);
			if (
				requestMethod.toUpperCase() === 'POST' &&
				new URL(requestUrl, globalThis.location.href).pathname.endsWith(
					'/api/ui/project-upload-load'
				)
			) {
				void response
					.clone()
					.json()
					.then(
						(value) => {
							globalThis.__gcObservedProjectUpload = { value, error: null };
						},
						(error) => {
							globalThis.__gcObservedProjectUpload = {
								value: null,
								error: error instanceof Error ? error.message : String(error)
							};
						}
					);
			}
			return response;
		};
	});
};

const uploadProjectThroughFileMenu = async (page, fixturePath, timeoutMs) => {
	await page.evaluate(() => {
		globalThis.__gcObservedProjectUpload = null;
	});
	const responsePromise = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname.endsWith('/api/ui/project-upload-load') &&
			response.request().method() === 'POST',
		{ timeout: timeoutMs }
	);
	const fileMenu = page.getByRole('button', { name: 'Open File menu' });
	await fileMenu.click();
	await page.waitForFunction(
		(button) => button?.getAttribute('aria-expanded') === 'true',
		await fileMenu.elementHandle(),
		{ timeout: timeoutMs }
	);
	await page.locator('input.gc-file-menu-upload[type="file"]').setInputFiles(fixturePath);
	const response = await responsePromise;
	if (!response.ok()) {
		throw new Error(`project upload failed with HTTP ${response.status()}`);
	}
	const observed = await page.waitForFunction(
		() => globalThis.__gcObservedProjectUpload,
		undefined,
		{ timeout: timeoutMs }
	);
	const upload = await observed.jsonValue();
	if (upload.error) {
		throw new Error(`project upload response could not be decoded: ${upload.error}`);
	}
	const result = upload.value;
	try {
		await page.locator('.gc-loading-overlay').waitFor({
			state: 'visible',
			timeout: Math.min(2000, timeoutMs)
		});
	} catch {
		// A small fixture can complete its loading overlay before the HTTP response is observed.
	}
	await waitForRuntimeReady(page, timeoutMs);
	await page.keyboard.press('Escape');
	return result;
};

const filterIssues = (
	issues,
	ignoreConsolePatterns,
	ignorePagePatterns,
	ignoreRequestPatterns
) => ({
	consoleErrors: issues.consoleErrors.filter(
		(issue) => !matchesIgnoredPattern(issue.text, ignoreConsolePatterns)
	),
	pageErrors: issues.pageErrors.filter(
		(issue) => !matchesIgnoredPattern(issue.message, ignorePagePatterns)
	),
	requestFailures: issues.requestFailures.filter(
		(issue) =>
			!matchesIgnoredPattern(
				`${issue.method} ${issue.url} ${issue.errorText}`,
				ignoreRequestPatterns
			)
	),
	httpErrors: issues.httpErrors.filter(
		(issue) =>
			!matchesIgnoredPattern(
				`${issue.method} ${issue.url} ${issue.status} ${issue.statusText}`,
				ignoreRequestPatterns
			)
	)
});

const createNetworkTelemetry = () => ({
	apiRequests: [],
	websockets: []
});

const attachNetworkTelemetry = (page, telemetry) => {
	page.on('request', (request) => {
		try {
			const url = new URL(request.url());
			if (!url.pathname.startsWith('/api/ui')) {
				return;
			}
			telemetry.apiRequests.push({
				url: request.url(),
				method: request.method(),
				resourceType: request.resourceType()
			});
		} catch {
			// Playwright can surface non-URL browser-internal requests; they are not runtime traffic.
		}
	});
	page.on('websocket', (socket) => {
		const entry = {
			url: socket.url(),
			receivedFrames: 0,
			sentFrames: 0
		};
		telemetry.websockets.push(entry);
		socket.on('framereceived', () => {
			entry.receivedFrames += 1;
		});
		socket.on('framesent', () => {
			entry.sentFrames += 1;
		});
	});
};

const websocketTotals = (telemetry) =>
	telemetry.websockets.reduce(
		(totals, socket) => ({
			receivedFrames: totals.receivedFrames + socket.receivedFrames,
			sentFrames: totals.sentFrames + socket.sentFrames
		}),
		{ receivedFrames: 0, sentFrames: 0 }
	);

const waitForCondition = async (condition, timeoutMs, message) => {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (await condition()) {
			return;
		}
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	throw new Error(message);
};

const waitForWebSocketReady = async (telemetry, timeoutMs) => {
	await waitForCondition(
		() => {
			const totals = websocketTotals(telemetry);
			return telemetry.websockets.length > 0 && totals.receivedFrames > 0;
		},
		timeoutMs,
		'the mounted product opened no WebSocket with received runtime frames'
	);
};

const waitForWebSocketProgress = async (telemetry, before, timeoutMs) => {
	await waitForCondition(
		() => {
			const totals = websocketTotals(telemetry);
			return totals.sentFrames > before.sentFrames && totals.receivedFrames > before.receivedFrames;
		},
		timeoutMs,
		'the UI mutation produced no WebSocket request/ack frame progress'
	);
};

const exactTextPattern = (value) =>
	new RegExp(`^\\s*${value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*$`);

const visibleTab = (page, title) =>
	page
		.locator('.dv-tab:visible')
		.filter({ hasText: exactTextPattern(title) })
		.first();

const visibleInspectorTitle = (page, title) =>
	page
		.locator('.inspector-header:visible .title-text')
		.filter({ hasText: exactTextPattern(title) })
		.first();

const selectManagerItem = async (page, label, timeoutMs) => {
	const labelButton = page
		.locator('button.outliner-item-label')
		.filter({ hasText: exactTextPattern(label) });
	const row = page
		.locator('.manager-list-content:visible .outliner-item-content[data-node-id]:visible')
		.filter({ has: labelButton })
		.first();
	await row.waitFor({ state: 'visible', timeout: timeoutMs });
	await row
		.locator('button.outliner-item-label')
		.filter({ hasText: exactTextPattern(label) })
		.click();
	await visibleInspectorTitle(page, label).waitFor({ state: 'visible', timeout: timeoutMs });
	return row;
};

const selectVisibleNode = async (page, label, timeoutMs) => {
	const labelButton = page
		.locator('button.outliner-item-label:visible')
		.filter({ hasText: exactTextPattern(label) });
	const row = page
		.locator('.outliner-item-content[data-node-id]:visible')
		.filter({ has: labelButton })
		.first();
	await row.waitFor({ state: 'visible', timeout: timeoutMs });
	await row
		.locator('button.outliner-item-label')
		.filter({ hasText: exactTextPattern(label) })
		.click();
	await visibleInspectorTitle(page, label).waitFor({ state: 'visible', timeout: timeoutMs });
	return row;
};

const renameOutlinerNode = async (page, telemetry, selectedRow, oldLabel, newLabel, timeoutMs) => {
	const nodeId = await selectedRow.getAttribute('data-node-id');
	if (!nodeId) {
		throw new Error(`the selected ${oldLabel} outliner row exposed no node id`);
	}
	const row = page
		.locator(`.manager-list-content:visible .outliner-item-content[data-node-id="${nodeId}"]`)
		.first();
	const label = row
		.locator('button.outliner-item-label')
		.filter({ hasText: exactTextPattern(oldLabel) });
	await label.dblclick();
	const renameInput = row.locator('input.outliner-item-rename-input:visible').first();
	await renameInput.waitFor({ state: 'visible', timeout: Math.min(timeoutMs, 15000) });
	const before = websocketTotals(telemetry);
	await renameInput.fill(newLabel);
	await renameInput.press('Enter');
	await waitForWebSocketProgress(telemetry, before, Math.min(timeoutMs, 15000));
	await row
		.locator('button.outliner-item-label')
		.filter({ hasText: exactTextPattern(newLabel) })
		.waitFor({ state: 'visible', timeout: Math.min(timeoutMs, 15000) });
	await visibleInspectorTitle(page, newLabel).waitFor({
		state: 'visible',
		timeout: Math.min(timeoutMs, 15000)
	});
	return { nodeId, from: oldLabel, to: newLabel };
};

const parameterInspector = (page, label) => {
	const labelElement = page
		.locator('.parameter-label, .custom-prop-name-text')
		.filter({ hasText: exactTextPattern(label) });
	return page
		.locator('.parameter-inspector[data-node-id]:visible')
		.filter({ has: labelElement })
		.first();
};

const setNumericParameter = async (page, telemetry, label, value, timeoutMs) => {
	const inspector = parameterInspector(page, label);
	await inspector.waitFor({ state: 'visible', timeout: timeoutMs });
	const input = inspector.locator('input[type="number"]:not(.readonly)').first();
	await input.waitFor({ state: 'visible', timeout: timeoutMs });
	const previousValue = await input.inputValue();
	const before = websocketTotals(telemetry);
	await input.fill(String(value));
	await input.press('Enter');
	await input.blur();
	await waitForCondition(
		async () => Number(await input.inputValue()) === Number(value),
		timeoutMs,
		`${label} did not retain the edited numeric value ${value}`
	);
	await waitForWebSocketProgress(telemetry, before, timeoutMs);
	return { previousValue, value: await input.inputValue() };
};

const observeChangingSignal = async (page, timeoutMs) => {
	const inspector = parameterInspector(page, 'Signal');
	await inspector.waitFor({ state: 'visible', timeout: timeoutMs });
	const input = inspector.locator('input.readonly[type="number"]').first();
	await input.waitFor({ state: 'visible', timeout: timeoutMs });
	const initialValue = await input.inputValue();
	let finalValue = initialValue;
	await waitForCondition(
		async () => {
			finalValue = await input.inputValue();
			return finalValue !== initialValue;
		},
		Math.min(timeoutMs, 8000),
		'the live Signal value did not visibly change in the inspector'
	);
	return { initialValue, finalValue };
};

const clickFileMenuItem = async (page, label, timeoutMs) => {
	const fileMenu = page.locator('button.gc-file-menu-trigger').first();
	await fileMenu.waitFor({ state: 'visible', timeout: timeoutMs });
	await fileMenu.click();
	await page.waitForFunction(
		(button) => button?.getAttribute('aria-expanded') === 'true',
		await fileMenu.elementHandle(),
		{ timeout: timeoutMs }
	);
	const item = page
		.locator('.gc-context-menu-layer:visible .gc-context-item-label')
		.filter({ hasText: exactTextPattern(label) })
		.first();
	await item.waitFor({ state: 'visible', timeout: timeoutMs });
	await item.click();
};

const invokeProjectMenuEndpoint = async (page, label, endpoint, timeoutMs) => {
	const responsePromise = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname.endsWith(`/api/ui/${endpoint}`) &&
			response.request().method() === 'POST',
		{ timeout: timeoutMs }
	);
	await clickFileMenuItem(page, label, timeoutMs);
	const response = await responsePromise;
	if (!response.ok()) {
		throw new Error(`${label} failed with HTTP ${response.status()}`);
	}
	return response;
};

const waitForOptionalLoadingCycle = async (page, timeoutMs) => {
	try {
		await page.locator('.gc-loading-overlay').waitFor({
			state: 'visible',
			timeout: Math.min(1500, timeoutMs)
		});
	} catch {
		return;
	}
	await waitForRuntimeReady(page, timeoutMs);
};

const openFormulaAndExerciseGraph = async (page, timeoutMs) => {
	await visibleTab(page, 'Formula Library').click();
	const actionTestButton = page
		.locator('.manager-list-content:visible button.outliner-item-label')
		.filter({ hasText: exactTextPattern('ActionTest') })
		.first();
	await actionTestButton.waitFor({ state: 'visible', timeout: timeoutMs });
	const formulaSelectionStarted = Date.now();
	await actionTestButton.click({ timeout: timeoutMs });
	const formulaSelectionMs = Date.now() - formulaSelectionStarted;
	const graph = page
		.getByRole('application', { name: 'Alchemist graph drop target' })
		.filter({ visible: true })
		.first();
	const graphReadyStarted = Date.now();
	await graph.waitFor({ state: 'visible', timeout: timeoutMs });
	const graphReadyMs = Date.now() - graphReadyStarted;
	const homeStarted = Date.now();
	await graph.getByRole('button', { name: 'Home', exact: true }).click();
	const canvas = graph.locator('.graph-canvas').first();
	let previousCamera = null;
	let stableSamples = 0;
	const cameraDeadline = Date.now() + timeoutMs;
	while (stableSamples < 3 && Date.now() < cameraDeadline) {
		const camera = await canvas.getAttribute('style');
		if (camera === previousCamera) {
			stableSamples += 1;
		} else {
			previousCamera = camera;
			stableSamples = 0;
		}
		await page.waitForTimeout(50);
	}
	if (stableSamples < 3) {
		throw new Error('Alchemist graph camera did not settle after Home');
	}
	const totalNodeCount = Number(await canvas.getAttribute('data-node-count'));
	const visibleNodeCount = Number(await canvas.getAttribute('data-visible-node-count'));
	const renderedNodeCount = await graph.locator('article.node').count();
	if (totalNodeCount < 1 || visibleNodeCount < 1 || renderedNodeCount < 1) {
		throw new Error('ActionTest opened no Alchemist graph nodes');
	}
	if (visibleNodeCount !== renderedNodeCount) {
		throw new Error(
			`Alchemist graph visible/rendered node count drifted (${visibleNodeCount} != ${renderedNodeCount})`
		);
	}
	return {
		formula: 'ActionTest',
		formulaSelectionMs,
		graphReadyMs,
		homeSettleMs: Date.now() - homeStarted,
		totalNodeCount,
		visibleNodeCount,
		renderedNodeCount
	};
};

const exerciseStateMachineGraph = async (page, timeoutMs) => {
	await visibleTab(page, 'State Machine').click();
	const stateTitle = page
		.locator('.graph-canvas:visible button.node-title-label')
		.filter({ hasText: exactTextPattern('State') })
		.first();
	await stateTitle.waitFor({ state: 'visible', timeout: timeoutMs });
	await stateTitle.click();
	const processor = page
		.locator('.graph-canvas:visible .processor-list button.outliner-item-label')
		.filter({ hasText: exactTextPattern('Action') })
		.first();
	await processor.waitFor({ state: 'visible', timeout: timeoutMs });
	await processor.click();
	await visibleInspectorTitle(page, 'Action').waitFor({ state: 'visible', timeout: timeoutMs });
	return { state: 'State', processor: 'Action' };
};

const isLoopbackHostname = (hostname) => {
	const normalized = hostname.replace(/^\[|\]$/g, '').toLowerCase();
	return normalized === 'localhost' || normalized === '::1' || normalized.startsWith('127.');
};

const ensureParentDirectory = async (filePath) => {
	await mkdir(path.dirname(filePath), { recursive: true });
};

const median = (values) => {
	const sorted = [...values].sort((left, right) => left - right);
	const middle = Math.floor(sorted.length / 2);
	return sorted.length % 2 === 0 ? (sorted[middle - 1] + sorted[middle]) / 2 : sorted[middle];
};

const summarizeMemoryPlateau = (samples, clients) =>
	clients.map((client) => {
		const values = samples
			.filter(
				(sample) =>
					sample.client === client.index &&
					Number.isFinite(sample.usedJsHeapSize) &&
					sample.usedJsHeapSize > 0
			)
			.map((sample) => sample.usedJsHeapSize);
		if (values.length < 8) {
			return { client: client.index, status: 'insufficient_samples', sampleCount: values.length };
		}
		const steady = values.slice(Math.floor(values.length * 0.2));
		const split = Math.floor(steady.length / 2);
		const baselineMedianBytes = median(steady.slice(0, split));
		const terminalMedianBytes = median(steady.slice(split));
		const growthBytes = Math.max(0, terminalMedianBytes - baselineMedianBytes);
		const allowedGrowthBytes = Math.max(64 * 1024 * 1024, baselineMedianBytes * 0.25);
		return {
			client: client.index,
			status: growthBytes <= allowedGrowthBytes ? 'passed' : 'failed',
			sampleCount: values.length,
			baselineMedianBytes,
			terminalMedianBytes,
			growthBytes,
			allowedGrowthBytes
		};
	});

const sampleRuntimeQueue = async (client) => {
	const title = await client.page.locator('.engine-rate').first().getAttribute('title');
	const match = title?.match(/control queue\s+(\d+)\s+\(peak\s+(\d+)/i);
	if (!match) {
		return { client: client.index, currentDepth: null, peakDepth: null, title };
	}
	return {
		client: client.index,
		currentDepth: Number(match[1]),
		peakDepth: Number(match[2]),
		title
	};
};

const runSmoke = async () => {
	const url = getArgValue('url', defaultUiUrl);
	const selector = getArgValue('selector', defaultSelector);
	const timeoutMs = getNumberArg('timeout', defaultTimeoutMs);
	const settledWaitMs = getNumberArg('wait', defaultSettledWaitMs);
	const screenshotPath = path.resolve(getArgValue('screenshot', './artifacts/ui-smoke.png'));
	const reportPath = path.resolve(getArgValue('report', './artifacts/ui-smoke-report.json'));
	const ignoreConsolePatterns = getArgValues('ignore-console-error');
	const ignorePagePatterns = getArgValues('ignore-page-error');
	const ignoreRequestPatterns = getArgValues('ignore-request-failure');

	const browser = await chromium.launch(resolveBrowserLaunchOptions());
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	const issues = createIssueCollector();
	attachIssueCollectors(page, issues);

	try {
		const title = await openAndSettlePage(page, url, selector, timeoutMs, settledWaitMs);
		const runtimeProbeEntries = await readRuntimeProbeEntries(page);
		await ensureParentDirectory(screenshotPath);
		await page.screenshot({ path: screenshotPath, fullPage: true });

		const filteredIssues = filterIssues(
			issues,
			ignoreConsolePatterns,
			ignorePagePatterns,
			ignoreRequestPatterns
		);
		const report = {
			url,
			title,
			selector,
			timeoutMs,
			settledWaitMs,
			screenshotPath,
			runtimeProbeEntries,
			issues: filteredIssues,
			failed:
				filteredIssues.consoleErrors.length > 0 ||
				filteredIssues.pageErrors.length > 0 ||
				filteredIssues.requestFailures.length > 0 ||
				filteredIssues.httpErrors.length > 0 ||
				runtimeProbeEntries.length > 0
		};

		await ensureParentDirectory(reportPath);
		await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
		console.log(JSON.stringify(report, null, 2));

		if (report.failed) {
			process.exitCode = 1;
		}
	} finally {
		await page.close();
		await browser.close();
	}
};

const runInspect = async () => {
	const url = getArgValue('url', defaultUiUrl);
	const selector = getArgValue('selector', defaultSelector);
	const timeoutMs = getNumberArg('timeout', defaultTimeoutMs);
	const settledWaitMs = getNumberArg('wait', defaultSettledWaitMs);
	const maxElements = getNumberArg('limit', 10);
	const maxTextLength = getNumberArg('text-limit', 200);
	const styleNames = getCsvArg('style', defaultInspectStyles);
	const attributeNames = getCsvArg('attribute', [
		'id',
		'class',
		'role',
		'aria-label',
		'data-node-id'
	]);

	const browser = await chromium.launch(resolveBrowserLaunchOptions());
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	const issues = createIssueCollector();
	attachIssueCollectors(page, issues);

	try {
		const title = await openAndSettlePage(page, url, selector, timeoutMs, settledWaitMs);
		const runtimeProbeEntries = await readRuntimeProbeEntries(page);
		const elements = await page.evaluate(
			({
				selector: resolvedSelector,
				maxElements: resolvedMaxElements,
				maxTextLength: resolvedMaxTextLength,
				styleNames: resolvedStyleNames,
				attributeNames: resolvedAttributeNames
			}) => {
				const normalizeText = (value, limit) =>
					(value ?? '').replace(/\s+/g, ' ').trim().slice(0, limit);
				return Array.from(document.querySelectorAll(resolvedSelector))
					.slice(0, resolvedMaxElements)
					.map((element) => {
						const rect = element.getBoundingClientRect();
						const computedStyle = window.getComputedStyle(element);
						return {
							tagName: element.tagName.toLowerCase(),
							text: normalizeText(element.textContent, resolvedMaxTextLength),
							attributes: Object.fromEntries(
								resolvedAttributeNames.map((name) => [name, element.getAttribute(name)])
							),
							classes: Array.from(element.classList),
							boundingBox: {
								x: Number(rect.x.toFixed(2)),
								y: Number(rect.y.toFixed(2)),
								width: Number(rect.width.toFixed(2)),
								height: Number(rect.height.toFixed(2))
							},
							styles: Object.fromEntries(
								resolvedStyleNames.map((name) => [
									name,
									computedStyle.getPropertyValue(name).trim()
								])
							)
						};
					});
			},
			{
				selector,
				maxElements,
				maxTextLength,
				styleNames,
				attributeNames
			}
		);

		const report = {
			url,
			title,
			selector,
			elementCount: elements.length,
			elements,
			runtimeProbeEntries,
			issues
		};

		console.log(JSON.stringify(report, null, 2));

		if (elements.length === 0) {
			process.exitCode = 1;
		}
	} finally {
		await page.close();
		await browser.close();
	}
};

const runProjectInspect = async () => {
	const url = getArgValue('url', defaultUiUrl);
	const fixturePath = path.resolve(getArgValue('fixture', ''));
	const search = getArgValue('search', '');
	const managerItem = getArgValue('manager-item', '');
	const timeoutMs = getNumberArg('timeout', defaultTimeoutMs);
	const browser = await chromium.launch(resolveBrowserLaunchOptions());
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	await installProjectUploadObserver(page);
	const issues = createIssueCollector();
	attachIssueCollectors(page, issues);

	try {
		await page.goto(url, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
		await waitForRuntimeReady(page, timeoutMs);
		await uploadProjectThroughFileMenu(page, fixturePath, timeoutMs);
		if (search) {
			await page.locator('input.outliner-search').fill(search);
			const row = page
				.locator('.outliner-item-content[data-node-id]')
				.filter({ has: page.getByRole('button', { name: search, exact: true }) })
				.first();
			await row.getByRole('button', { name: search, exact: true }).click();
		}
		if (managerItem) {
			const row = page
				.locator('.manager-list-content .outliner-item-content[data-node-id]')
				.filter({ has: page.getByRole('button', { name: managerItem, exact: true }) })
				.first();
			await row.getByRole('button', { name: managerItem, exact: true }).click();
		}
		await page.waitForTimeout(1000);
		const elements = await page.evaluate(() =>
			Array.from(
				document.querySelectorAll(
					'button,input,[role],.dv-tab,.tree-row,.outliner-row,[data-node-id]'
				)
			)
				.filter((element) => {
					const rect = element.getBoundingClientRect();
					return rect.width > 0 && rect.height > 0;
				})
				.slice(0, 1000)
				.map((element) => ({
					tag: element.tagName.toLowerCase(),
					text: (element.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 180),
					className: element.getAttribute('class'),
					role: element.getAttribute('role'),
					ariaLabel: element.getAttribute('aria-label'),
					title: element.getAttribute('title'),
					type: element.getAttribute('type'),
					dataNodeId: element.getAttribute('data-node-id'),
					value: element instanceof HTMLInputElement ? element.value : null
				}))
		);
		console.log(
			JSON.stringify({ url, fixturePath, search, managerItem, elements, issues }, null, 2)
		);
	} finally {
		await page.close();
		await browser.close();
	}
};

const validateLanTraffic = (url, expectedHost, telemetry) => {
	const visibleUrl = new URL(url);
	if (!expectedHost) {
		throw new Error('--expected-host is required for the LAN gate');
	}
	if (isLoopbackHostname(expectedHost)) {
		throw new Error(`the LAN gate was given loopback host '${expectedHost}'`);
	}
	if (visibleUrl.hostname !== expectedHost) {
		throw new Error(
			`the browser-visible host '${visibleUrl.hostname}' did not equal '${expectedHost}'`
		);
	}
	if (telemetry.websockets.length < 1) {
		throw new Error('the LAN page opened no runtime WebSocket');
	}
	if (telemetry.apiRequests.length < 1) {
		throw new Error('the LAN page made no runtime API requests');
	}

	const runtimeUrls = [
		...telemetry.websockets.map((entry) => entry.url),
		...telemetry.apiRequests.map((entry) => entry.url)
	];
	for (const runtimeUrl of runtimeUrls) {
		const hostname = new URL(runtimeUrl).hostname;
		if (isLoopbackHostname(hostname)) {
			throw new Error(`LAN runtime traffic used forbidden loopback URL '${runtimeUrl}'`);
		}
		if (hostname !== expectedHost) {
			throw new Error(`LAN runtime traffic escaped the browser-visible host: '${runtimeUrl}'`);
		}
	}
};

const assertNoBrowserIssues = (issues) => {
	const counts = Object.fromEntries(
		Object.entries(issues).map(([name, entries]) => [name, entries.length])
	);
	if (Object.values(counts).some((count) => count > 0)) {
		throw new Error(`browser console/network failures were recorded: ${JSON.stringify(counts)}`);
	}
};

const runProductGate = async (mode) => {
	const url = getArgValue('url', '');
	const fixturePath = path.resolve(getArgValue('fixture', ''));
	const artifactDirectory = path.resolve(getArgValue('artifact-directory', `./artifacts/${mode}`));
	const reportPath = path.resolve(
		getArgValue('report', path.join(artifactDirectory, `${mode}.report.json`))
	);
	const timeoutMs = getNumberArg('timeout', 90000);
	const expectedHost = getArgValue('expected-host', '');
	const traceEnabled = getArgValue('trace', 'true') !== 'false';
	const tracePath = path.join(artifactDirectory, `${mode}.trace.zip`);
	const loadedScreenshotPath = path.join(artifactDirectory, '01-loaded.png');
	const interactionScreenshotPath = path.join(artifactDirectory, '02-interaction.png');
	const reloadedScreenshotPath = path.join(artifactDirectory, '03-reloaded.png');
	const failureScreenshotPath = path.join(artifactDirectory, 'failure.png');
	const issues = createIssueCollector();
	const telemetry = createNetworkTelemetry();
	const steps = [];
	const report = {
		contract: 'chataigne-product-browser-gate-v1',
		mode,
		status: 'running',
		url,
		expectedHost: expectedHost || null,
		fixturePath,
		loadedProjectPath: null,
		artifacts: {
			trace: traceEnabled ? tracePath : null,
			loadedScreenshot: loadedScreenshotPath,
			interactionScreenshot: interactionScreenshotPath,
			reloadedScreenshot: mode === 'ui-workflow' ? reloadedScreenshotPath : null,
			failureScreenshot: failureScreenshotPath
		},
		steps,
		issues: null,
		network: null,
		error: null
	};

	await mkdir(artifactDirectory, { recursive: true });
	const browser = await chromium.launch(resolveBrowserLaunchOptions());
	const context = await browser.newContext({
		viewport: { width: 1600, height: 1000 },
		colorScheme: 'dark',
		locale: 'en-US',
		reducedMotion: 'reduce'
	});
	if (traceEnabled) {
		await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
	}
	const page = await context.newPage();
	await installProjectUploadObserver(page);
	attachIssueCollectors(page, issues);
	attachNetworkTelemetry(page, telemetry);
	let failure = null;

	try {
		await page.goto(url, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
		await waitForRuntimeReady(page, timeoutMs);
		await waitForWebSocketReady(telemetry, timeoutMs);
		steps.push({ step: 'runtime-ready', websocket: websocketTotals(telemetry) });

		const uploadResult = await uploadProjectThroughFileMenu(page, fixturePath, timeoutMs);
		report.loadedProjectPath = typeof uploadResult?.path === 'string' ? uploadResult.path : null;
		await page
			.getByRole('banner')
			.filter({ hasText: path.basename(fixturePath) })
			.waitFor({ state: 'visible', timeout: timeoutMs });
		steps.push({
			step: 'fixture-loaded',
			fixture: path.basename(fixturePath),
			projectPath: report.loadedProjectPath
		});
		await page.screenshot({ path: loadedScreenshotPath, fullPage: true });

		const selectedSignals = await selectManagerItem(page, 'Signals', timeoutMs);
		if (mode === 'ui-workflow') {
			await renameOutlinerNode(
				page,
				telemetry,
				selectedSignals,
				'Signals',
				'Product Gate Signals',
				timeoutMs
			);
			steps.push({ step: 'outliner-rename', from: 'Signals', to: 'Product Gate Signals' });
			const updateRate = await setNumericParameter(page, telemetry, 'Update Rate', 47, timeoutMs);
			steps.push({ step: 'inspector-mutation', parameter: 'Update Rate', ...updateRate });
			const signalFeedback = await observeChangingSignal(page, timeoutMs);
			steps.push({ step: 'live-value-feedback', parameter: 'Signal', ...signalFeedback });
			await page.screenshot({ path: interactionScreenshotPath, fullPage: true });

			steps.push({
				step: 'formula-interaction',
				...(await openFormulaAndExerciseGraph(page, timeoutMs))
			});
			steps.push({
				step: 'state-machine-interaction',
				...(await exerciseStateMachineGraph(page, timeoutMs))
			});

			await invokeProjectMenuEndpoint(page, 'Save', 'project-save', timeoutMs);
			await waitForOptionalLoadingCycle(page, timeoutMs);
			steps.push({ step: 'project-save', status: 'acknowledged' });

			await invokeProjectMenuEndpoint(page, 'Open Last', 'project-load', timeoutMs);
			await waitForOptionalLoadingCycle(page, timeoutMs);
			await waitForRuntimeReady(page, timeoutMs);
			await selectManagerItem(page, 'Product Gate Signals', timeoutMs);
			const persistedInput = parameterInspector(page, 'Update Rate')
				.locator('input[type="number"]:not(.readonly)')
				.first();
			await waitForCondition(
				async () => Number(await persistedInput.inputValue()) === 47,
				timeoutMs,
				'Update Rate=47 did not persist through Save/Open Last'
			);
			await page
				.locator('.outliner-item-content[data-node-id]:visible')
				.filter({ hasText: /Product Gate Signals\s+signals_module/ })
				.first()
				.waitFor({ state: 'visible', timeout: timeoutMs });
			const reloadedSignalFeedback = await observeChangingSignal(page, timeoutMs);
			steps.push({
				step: 'save-reload-verified',
				label: 'Product Gate Signals',
				updateRate: await persistedInput.inputValue(),
				liveSignal: reloadedSignalFeedback
			});
			await page.screenshot({ path: reloadedScreenshotPath, fullPage: true });
		} else {
			const firstMutation = await setNumericParameter(
				page,
				telemetry,
				'Update Rate',
				59,
				timeoutMs
			);
			const restoredMutation = await setNumericParameter(
				page,
				telemetry,
				'Update Rate',
				Number(firstMutation.previousValue),
				timeoutMs
			);
			const signalFeedback = await observeChangingSignal(page, timeoutMs);
			steps.push({
				step: 'lan-backend-interaction',
				parameter: 'Update Rate',
				mutation: firstMutation,
				restore: restoredMutation,
				liveSignal: signalFeedback
			});
			validateLanTraffic(url, expectedHost, telemetry);
			steps.push({
				step: 'lan-addressing-verified',
				host: expectedHost,
				apiRequestCount: telemetry.apiRequests.length,
				websocketCount: telemetry.websockets.length
			});
			await page.screenshot({ path: interactionScreenshotPath, fullPage: true });
		}

		await invokeProjectMenuEndpoint(page, 'New', 'project-new', timeoutMs);
		await waitForOptionalLoadingCycle(page, timeoutMs);
		steps.push({ step: 'temporary-project-unloaded' });

		report.issues = filterIssues(issues, [], [], []);
		report.network = {
			apiRequests: telemetry.apiRequests,
			websockets: telemetry.websockets,
			totals: websocketTotals(telemetry)
		};
		assertNoBrowserIssues(report.issues);
		report.status = 'passed';
	} catch (error) {
		failure = error;
		report.status = 'failed';
		report.error = toSerializableError(error);
		report.issues = filterIssues(issues, [], [], []);
		report.network = {
			apiRequests: telemetry.apiRequests,
			websockets: telemetry.websockets,
			totals: websocketTotals(telemetry)
		};
		try {
			await page.screenshot({ path: failureScreenshotPath, fullPage: true });
		} catch {
			// The report still carries the browser error when the page itself disappeared.
		}
	} finally {
		if (traceEnabled) {
			try {
				await context.tracing.stop({ path: tracePath });
			} catch (error) {
				if (!failure) {
					failure = error;
					report.status = 'failed';
					report.error = toSerializableError(error);
				}
			}
		}
		await ensureParentDirectory(reportPath);
		await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
		console.log(
			JSON.stringify(
				{
					contract: report.contract,
					mode: report.mode,
					status: report.status,
					reportPath,
					loadedProjectPath: report.loadedProjectPath,
					stepNames: report.steps.map((step) => step.step),
					websocketTotals: report.network?.totals ?? null,
					error: report.error
				},
				null,
				2
			)
		);
		await page.close().catch(() => {});
		await context.close().catch(() => {});
		await browser.close().catch(() => {});
	}

	if (failure) {
		process.exitCode = 1;
	}
};

const runMultiClientSoak = async () => {
	const url = getArgValue('url', defaultUiUrl);
	const fixturePath = getArgValue('fixture', '');
	const reportPath = path.resolve(getArgValue('report', 'soak.browser-report.json'));
	const artifactDirectory = path.resolve(
		getArgValue('artifact-directory', path.dirname(reportPath))
	);
	const timeoutMs = getNumberArg('timeout', 120_000);
	const durationMs = getNumberArg('duration-ms', 5 * 60 * 1000);
	const clientCount = Math.max(2, Math.floor(getNumberArg('clients', 3)));
	const intervalMs = Math.max(250, getNumberArg('interval-ms', 1000));
	if (!fixturePath) {
		throw new Error('--fixture is required for the multi-client soak');
	}

	await mkdir(artifactDirectory, { recursive: true });
	const browser = await chromium.launch(resolveBrowserLaunchOptions());
	const clients = [];
	const report = {
		contract: 'chataigne-multiclient-soak-v1',
		status: 'running',
		url,
		fixturePath,
		clientCount,
		durationMs,
		intervalMs,
		startedAt: new Date().toISOString(),
		finishedAt: null,
		iterations: 0,
		memorySamples: [],
		memoryPlateau: [],
		queueSamples: [],
		queueSummary: [],
		clients: [],
		error: null
	};
	let failure = null;
	try {
		for (let index = 0; index < clientCount; index += 1) {
			const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
			const page = await context.newPage();
			const issues = createIssueCollector();
			const telemetry = createNetworkTelemetry();
			attachIssueCollectors(page, issues);
			attachNetworkTelemetry(page, telemetry);
			await installProjectUploadObserver(page);
			await page.goto(url, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
			await waitForRuntimeReady(page, timeoutMs);
			await waitForWebSocketReady(telemetry, timeoutMs);
			clients.push({ context, page, issues, telemetry, index });
		}

		const upload = await uploadProjectThroughFileMenu(clients[0].page, fixturePath, timeoutMs);
		report.loadedProjectPath = typeof upload?.path === 'string' ? upload.path : null;
		for (const client of clients) {
			await waitForRuntimeReady(client.page, timeoutMs);
			await selectVisibleNode(client.page, 'Signals', timeoutMs);
		}

		const deadline = Date.now() + durationMs;
		while (Date.now() < deadline) {
			const author = clients[report.iterations % clients.length];
			const value = 31 + (report.iterations % 59);
			await setNumericParameter(author.page, author.telemetry, 'Update Rate', value, timeoutMs);
			for (const client of clients) {
				const input = parameterInspector(client.page, 'Update Rate')
					.locator('input[type="number"]:not(.readonly)')
					.first();
				await waitForCondition(
					async () => Number(await input.inputValue()) === value,
					timeoutMs,
					`client ${client.index} did not observe Update Rate=${value}`
				);
			}
			if (report.iterations % 30 === 0) {
				for (const client of clients) {
					const usedJsHeapSize = await client.page.evaluate(
						() => globalThis.performance?.memory?.usedJSHeapSize ?? null
					);
					report.memorySamples.push({
						iteration: report.iterations,
						client: client.index,
						usedJsHeapSize
					});
					report.queueSamples.push({
						iteration: report.iterations,
						...(await sampleRuntimeQueue(client))
					});
				}
			}
			report.iterations += 1;
			await author.page.waitForTimeout(intervalMs);
		}
		report.memoryPlateau = summarizeMemoryPlateau(report.memorySamples, clients);
		if (
			durationMs >= 5 * 60 * 1000 &&
			report.memoryPlateau.some((summary) => summary.status !== 'passed')
		) {
			throw new Error('one or more browser clients did not reach a bounded heap plateau');
		}

		await invokeProjectMenuEndpoint(clients[0].page, 'Save', 'project-save', timeoutMs);
		await invokeProjectMenuEndpoint(clients[0].page, 'Open Last', 'project-load', timeoutMs);
		for (const client of clients) {
			await waitForRuntimeReady(client.page, timeoutMs);
			await waitForCondition(
				async () => (await sampleRuntimeQueue(client)).currentDepth === 0,
				timeoutMs,
				`client ${client.index} runtime control queue did not drain`
			);
			const queueSamples = report.queueSamples.filter((sample) => sample.client === client.index);
			const finalQueue = await sampleRuntimeQueue(client);
			if (
				durationMs >= 5 * 60 * 1000 &&
				(queueSamples.length < 8 || queueSamples.some((sample) => sample.currentDepth === null))
			) {
				throw new Error(`client ${client.index} has incomplete runtime queue telemetry`);
			}
			report.queueSummary.push({
				client: client.index,
				sampleCount: queueSamples.length,
				maximumObservedDepth: Math.max(
					0,
					...queueSamples.map((sample) => sample.currentDepth ?? 0)
				),
				maximumReportedPeak: Math.max(0, ...queueSamples.map((sample) => sample.peakDepth ?? 0)),
				finalDepth: finalQueue.currentDepth,
				status: finalQueue.currentDepth === 0 ? 'passed' : 'failed'
			});
			const filteredIssues = filterIssues(client.issues, [], [], []);
			assertNoBrowserIssues(filteredIssues);
			const totals = websocketTotals(client.telemetry);
			if (totals.receivedFrames <= 0 || totals.sentFrames <= 0) {
				throw new Error(`client ${client.index} had no bidirectional WebSocket traffic`);
			}
			report.clients.push({
				index: client.index,
				issues: filteredIssues,
				websocketCount: client.telemetry.websockets.length,
				websocketTotals: totals
			});
		}
		await invokeProjectMenuEndpoint(clients[0].page, 'New', 'project-new', timeoutMs);
		report.status = 'passed';
	} catch (error) {
		failure = error;
		report.status = 'failed';
		report.error = toSerializableError(error);
		for (const client of clients) {
			await client.page
				.screenshot({
					path: path.join(artifactDirectory, `soak-client-${client.index}-failure.png`),
					fullPage: true
				})
				.catch(() => {});
		}
	} finally {
		report.finishedAt = new Date().toISOString();
		for (const client of clients) {
			await client.context.close().catch(() => {});
		}
		await browser.close().catch(() => {});
		await ensureParentDirectory(reportPath);
		await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
		console.log(JSON.stringify(report, null, 2));
	}
	if (failure) {
		process.exitCode = 1;
	}
};

const main = async () => {
	if (command === 'smoke') {
		await runSmoke();
		return;
	}
	if (command === 'inspect') {
		await runInspect();
		return;
	}
	if (command === 'inspect-project') {
		await runProjectInspect();
		return;
	}
	if (command === 'product-gate-workflow') {
		await runProductGate('ui-workflow');
		return;
	}
	if (command === 'product-gate-lan') {
		await runProductGate('lan-browser');
		return;
	}
	if (command === 'product-gate-soak') {
		await runMultiClientSoak();
		return;
	}
	console.error(`Unknown command: ${command}`);
	process.exitCode = 1;
};

main().catch((error) => {
	const browserConfigHint = [
		'Set GC_UI_BROWSER_CHANNEL to a locally installed Chromium channel such as msedge or chrome.',
		'Or set GC_UI_BROWSER_EXECUTABLE to a Chromium-based browser binary path.'
	].join(' ');
	console.error(
		JSON.stringify(
			{
				command,
				error: toSerializableError(error),
				hint: browserConfigHint
			},
			null,
			2
		)
	);
	process.exitCode = 1;
});
