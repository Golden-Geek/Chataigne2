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
	stack: error instanceof Error ? error.stack ?? null : null
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
	requestFailures: []
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

const filterIssues = (issues, ignoreConsolePatterns, ignorePagePatterns, ignoreRequestPatterns) => ({
	consoleErrors: issues.consoleErrors.filter((issue) => !matchesIgnoredPattern(issue.text, ignoreConsolePatterns)),
	pageErrors: issues.pageErrors.filter((issue) => !matchesIgnoredPattern(issue.message, ignorePagePatterns)),
	requestFailures: issues.requestFailures.filter(
		(issue) =>
			!matchesIgnoredPattern(`${issue.method} ${issue.url} ${issue.errorText}`, ignoreRequestPatterns)
	)
});

const ensureParentDirectory = async (filePath) => {
	await mkdir(path.dirname(filePath), { recursive: true });
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
	const attributeNames = getCsvArg('attribute', ['id', 'class', 'role', 'aria-label', 'data-node-id']);

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
					(value ?? '')
						.replace(/\s+/g, ' ')
						.trim()
						.slice(0, limit);
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
								resolvedStyleNames.map((name) => [name, computedStyle.getPropertyValue(name).trim()])
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

const main = async () => {
	if (command === 'smoke') {
		await runSmoke();
		return;
	}
	if (command === 'inspect') {
		await runInspect();
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