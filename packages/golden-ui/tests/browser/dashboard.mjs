import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, readFileSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { cpus, platform, release, totalmem } from 'node:os';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';

const repository = fileURLToPath(new URL('../../../../', import.meta.url));
const root = fileURLToPath(new URL('./fixture/', import.meta.url));
const generated = resolve(repository, 'target/qualification');
const baseline = 'c1816a5c';
const canvasPath = 'packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte';
const original = execFileSync('git', ['show', `${baseline}:${canvasPath}`], {
	cwd: repository,
	encoding: 'utf8',
	maxBuffer: 2_000_000
});
const errors = [];
mkdirSync(generated, { recursive: true });
const report = resolve(
	generated,
	`p11-dashboard-split-${new Date().toISOString().replaceAll(/[:.]/g, '-')}.json`
);
const hash = (value) => createHash('sha256').update(value).digest('hex');
const filesBelow = (directory) =>
	readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = resolve(directory, entry.name);
		return entry.isDirectory() ? filesBelow(path) : [path];
	});
const sourceFiles = [
	...filesBelow(resolve(repository, 'packages/golden-ui/components/panels/dashboard')),
	...filesBelow(root).filter((path) =>
		/[\\/](dashboard[^\\/]*|Dashboard[^\\/]*|session\.ts)$/.test(path)
	),
	fileURLToPath(import.meta.url)
];
const fingerprints = () =>
	Object.fromEntries(
		sourceFiles.map((path) => [
			relative(repository, path).replaceAll('\\', '/'),
			hash(readFileSync(path))
		])
	);
const result = {
	baseline,
	browser: '',
	cases: [],
	errors,
	passed: false,
	machine: {
		platform: platform(),
		release: release(),
		cpu: cpus()[0]?.model,
		logicalCpus: cpus().length,
		memoryBytes: totalmem(),
		node: process.version
	},
	source: fingerprints(),
	productionAssets: {}
};
let browser;

// Compare actual production components. No synthetic input or desktop control is used.
async function capture(page) {
	return page.locator('main').evaluate((root) => {
		const properties = [
			'display',
			'position',
			'width',
			'height',
			'minWidth',
			'minHeight',
			'flexDirection',
			'flexWrap',
			'flexGrow',
			'flexShrink',
			'alignItems',
			'justifyContent',
			'gridTemplateColumns',
			'gridTemplateRows',
			'gap',
			'padding',
			'margin',
			'overflow',
			'fontSize',
			'lineHeight',
			'color',
			'backgroundColor',
			'border',
			'borderRadius',
			'boxShadow',
			'opacity',
			'transform',
			'pointerEvents',
			'zIndex'
		];
		return [...root.querySelectorAll('*')].map((element) => {
			const style = getComputedStyle(element);
			const rect = element.getBoundingClientRect();
			return {
				tag: element.tagName,
				classes: [...element.classList]
					.filter((name) => !name.startsWith('svelte-') && name !== 'gc-dashboard-canvas')
					.sort(),
				node: element.getAttribute('data-node-id'),
				surface: element.getAttribute('data-dashboard-surface-id'),
				text: element.children.length ? '' : element.textContent,
				box: [rect.x, rect.y, rect.width, rect.height],
				style: Object.fromEntries(properties.map((key) => [key, style[key]])),
				value: 'value' in element ? element.value : undefined,
				disabled: 'disabled' in element ? element.disabled : undefined,
				checked: 'checked' in element ? element.checked : undefined,
				inert: element.hasAttribute('inert')
			};
		});
	});
}

function compare(expected, actual, label) {
	assert.equal(actual.length, expected.length, `${label}: DOM element count`);
	for (let index = 0; index < actual.length; index++) {
		const { box: expectedBox, ...expectedOther } = expected[index];
		const { box: actualBox, ...actualOther } = actual[index];
		assert.deepEqual(actualOther, expectedOther, `${label}: element ${index}`);
		for (let coordinate = 0; coordinate < 4; coordinate++)
			assert(
				Math.abs(actualBox[coordinate] - expectedBox[coordinate]) <= 0.5,
				`${label}: element ${index} geometry ${actualBox} != ${expectedBox}`
			);
	}
}

async function run(version) {
	const outDir = resolve(generated, `p11-dashboard-${version}`);
	await build({
		configFile: false,
		root,
		plugins: [
			...(version === 'baseline'
				? [
						{
							name: 'delivered-dashboard-source',
							enforce: 'pre',
							load(id) {
								if (id.replaceAll('\\', '/').endsWith('/' + canvasPath)) return original;
							}
						}
					]
				: []),
			svelte({ configFile: false })
		],
		logLevel: 'warn',
		build: { outDir, emptyOutDir: true, rollupOptions: { input: resolve(root, 'dashboard.html') } }
	});
	result.productionAssets[version] = Object.fromEntries(
		filesBelow(outDir).map((path) => [
			path.slice(outDir.length + 1).replaceAll('\\', '/'),
			hash(readFileSync(path))
		])
	);
	const server = await preview({
		configFile: false,
		root,
		build: { outDir },
		logLevel: 'warn',
		preview: { host: '127.0.0.1', port: 0, strictPort: true }
	});
	const page = await browser.newPage({ viewport: { width: 1680, height: 1200 } });
	page.on('pageerror', (error) => errors.push({ version, message: error.message }));
	const captures = new Map();
	const record = async (name) => {
		captures.set(name, await capture(page));
		assert.equal(errors.length, 0, JSON.stringify(errors));
	};
	try {
		await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dashboard.html`, {
			waitUntil: 'networkidle'
		});
		await page.waitForFunction(() => Boolean(window.dashboardFixture));
		for (const layout of ['free', 'horizontal', 'vertical', 'grid', 'accordion', 'tabs']) {
			for (const editing of [false, true]) {
				const label = `${layout}-${editing ? 'edit' : 'run'}`;
				await page.evaluate(
					({ layout, editing }) => window.dashboardFixture.setup(layout, editing),
					{ layout, editing }
				);
				await record(label);
				await page.evaluate(() => window.dashboardFixture.select(2));
				await record(label + '-selection');
				await page.evaluate(() => window.dashboardFixture.view({ panX: 65, panY: -25, zoom: 1.3 }));
				await record(label + '-camera');
				for (const command of ['view.frame', 'view.home']) {
					const outcome = await page.evaluate(
						(command) => window.dashboardFixture.command(command),
						command
					);
					assert.equal(outcome.handled, true, `${label}: ${command}`);
					await record(label + '-' + command);
				}
				await page.evaluate(() =>
					window.dashboardFixture.update(2, 'width', {
						kind: 'css_value',
						value: 23,
						unit: 'percent'
					})
				);
				await record(label + '-relative-size');
				await page.evaluate(() => window.dashboardFixture.reorder());
				await record(label + '-reorder');
				await page.setViewportSize({ width: 1200, height: 900 });
				await page.evaluate(() => window.dashboardFixture.select(0));
				await record(label + '-responsive');
				await page.setViewportSize({ width: 1680, height: 1200 });
			}
		}
		await page.evaluate(() => window.dashboardFixture.setup('free', false));
		for (const anchor of [
			'top-left',
			'top-center',
			'top-right',
			'center-left',
			'center',
			'center-right',
			'bottom-left',
			'bottom-center',
			'bottom-right'
		]) {
			await page.evaluate(
				(anchor) => window.dashboardFixture.update(0, 'anchor', { kind: 'enum', value: anchor }),
				anchor
			);
			await record('anchor-' + anchor);
		}
		if (version === 'current') {
			await page.evaluate(() => window.dashboardFixture.setup('free', true));
			const position = await page.evaluate(() => window.dashboardFixture.placement(0));
			await page.evaluate(() =>
				window.dashboardFixture.update(0, 'anchor', { kind: 'enum', value: 'bottom-right' })
			);
			assert.deepEqual(
				await page.evaluate(() => window.dashboardFixture.placement(0)),
				position,
				'Rendering an anchor change must not rewrite authored offsets'
			);
			assert.deepEqual(errors, [], 'Rendering must not send any domain repair intent');
			result.authoredAnchorOffsetsPreserved = true;
		}
		await page.evaluate(() => window.dashboardFixture.setup('free', true, 1000));
		assert.equal(
			await page.locator('.dashboard-widget-shell').count(),
			1000,
			'The refactor preserves the full pre-culling sparse page'
		);
		await record('sparse-1000');
		console.log(JSON.stringify({ version, cases: captures.size, errors }));
		return captures;
	} finally {
		await page.close();
		await server.close();
	}
}

try {
	browser = await chromium.launch({
		headless: true,
		...(process.env.GOLDEN_UI_BROWSER_PATH
			? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
			: { channel: process.platform === 'win32' ? 'msedge' : undefined })
	});
	result.browser = await browser.version();
	const expected = await run('baseline');
	const actual = await run('current');
	for (const [label, before] of expected) {
		compare(before, actual.get(label), label);
		result.cases.push({
			label,
			elements: before.length,
			expectedSha256: hash(JSON.stringify(before)),
			actualSha256: hash(JSON.stringify(actual.get(label))),
			passed: true
		});
	}
	assert.deepEqual(fingerprints(), result.source, 'Qualified sources changed during the run');
	result.passed = true;
	console.log(JSON.stringify({ passed: true, cases: result.cases.length, report }));
} catch (error) {
	result.failure = String(error);
	throw error;
} finally {
	writeFileSync(report, JSON.stringify(result, null, 2) + '\n');
	await browser?.close();
}
