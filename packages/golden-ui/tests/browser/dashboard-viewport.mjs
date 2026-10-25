import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, mkdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { cpus, platform, release, totalmem } from 'node:os';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';
import { denseChecks } from './dashboard-dense.mjs';
import { hierarchyChecks } from './dashboard-hierarchy.mjs';
import { lifecycleChecks } from './dashboard-lifecycle.mjs';
import { editLifecycleChecks } from './dashboard-edit-lifecycle.mjs';
import { creationChecks } from './dashboard-creation.mjs';
import { gestureChecks } from './dashboard-gestures.mjs';
import { confirmedChecks } from './dashboard-confirmed.mjs';
import { orderedChecks } from './dashboard-ordered.mjs';
import { orderedMoveChecks } from './dashboard-ordered-move.mjs';
import { compositionChecks } from './dashboard-composition.mjs';
import { barrierChecks } from './dashboard-barriers.mjs';

const repository = fileURLToPath(new URL('../../../../', import.meta.url));
const root = fileURLToPath(new URL('./fixture/', import.meta.url));
const generated = resolve(repository, 'target/qualification');
const baseline = '654a5f15';
const denseOnly = process.argv.includes('--dense-only');
const hierarchyOnly = process.argv.includes('--hierarchy-only');
const lifecycleOnly = process.argv.includes('--lifecycle-only');
const editOnly = process.argv.includes('--edit-only');
const creationOnly = process.argv.includes('--creation-only');
const gesturesOnly = process.argv.includes('--gestures-only');
const confirmedOnly = process.argv.includes('--confirmed-only');
const orderedOnly = process.argv.includes('--ordered-only');
const orderedMoveOnly = process.argv.includes('--ordered-move-only');
const compositionOnly = process.argv.includes('--composition-only');
const barrierBaseline = process.argv.includes('--barriers-baseline');
const barriersOnly = process.argv.includes('--barriers-only') || barrierBaseline;
const oldSavePath = 'packages/golden-ui/store/project-files.svelte.ts';
const oldSave = barrierBaseline
	? execFileSync('git', ['show', `85c34e6f:${oldSavePath}`], { cwd: repository, encoding: 'utf8' })
	: null;
const fullRun = !(
	denseOnly ||
	hierarchyOnly ||
	lifecycleOnly ||
	editOnly ||
	creationOnly ||
	gesturesOnly ||
	confirmedOnly ||
	orderedOnly ||
	orderedMoveOnly ||
	compositionOnly ||
	barriersOnly
);
const canvasPrefix = 'packages/golden-ui/components/panels/dashboard/';
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const filesBelow = (directory) =>
	readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = resolve(directory, entry.name);
		return entry.isDirectory() ? filesBelow(path) : [path];
	});
const sources = [
	...filesBelow(resolve(repository, canvasPrefix)),
	...filesBelow(root).filter((path) =>
		/[\\/](dashboard[^\\/]*|Dashboard[^\\/]*|session\.ts)$/.test(path)
	),
	resolve(repository, 'packages/golden-ui/store/graph-index.ts'),
	resolve(repository, 'packages/golden-ui/css-value.ts'),
	resolve(repository, 'packages/golden-ui/components/common/ContextMenu.svelte'),
	...filesBelow(resolve(repository, 'packages/golden-ui/components/common/context-menu')),
	...filesBelow(resolve(repository, 'packages/golden-ui/components/common/virtual-rows')),
	resolve(repository, 'packages/golden-ui/tests/css-value-units.test.ts'),
	resolve(repository, 'packages/golden-graph-ui/spatial-index.ts'),
	resolve(repository, 'packages/golden-ui/package.json'),
	resolve(repository, 'package-lock.json'),
	fileURLToPath(new URL('./dashboard-dense.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-hierarchy.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-lifecycle.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-edit-lifecycle.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-creation.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-gestures.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-confirmed.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-ordered.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-ordered-move.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-composition.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-composition-lifetimes.mjs', import.meta.url)),
	fileURLToPath(new URL('./dashboard-barriers.mjs', import.meta.url)),
	resolve(repository, 'packages/golden-ui/store/ui-intents.ts'),
	resolve(repository, 'packages/golden-ui/types.ts'),
	resolve(repository, 'packages/golden-ui/generated/rust_protocol/UiEditIntent.ts'),
	resolve(repository, 'packages/golden-ui/generated/rust_protocol/protocol-version.ts'),
	resolve(repository, 'crates/golden_core/services/protocol/src/types/intent.rs'),
	resolve(repository, 'crates/golden_core/services/protocol/src/types/message.rs'),
	resolve(repository, 'crates/golden_core/services/protocol/tests/public_protocol.rs'),
	resolve(repository, 'crates/golden_core/engine/src/ui_sync.rs'),
	resolve(repository, 'crates/golden_core/engine/src/ui_sync/dashboard_layout.rs'),
	resolve(repository, 'crates/golden_core/engine/src/ui_sync/tests/dashboard_layout.rs'),
	resolve(repository, 'crates/golden_core/hosts/transport/src/ui_server/mod.rs'),
	resolve(repository, 'crates/golden_core/support/codegen/src/lib.rs'),
	resolve(repository, 'packages/golden-ui/store/workbench.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/graph.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/session/action-activity.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/session/edit-operations.ts'),
	resolve(repository, 'packages/golden-ui/store/session/edit-operations.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/session/event-time.ts'),
	resolve(repository, 'packages/golden-ui/store/session/intent-sender.ts'),
	resolve(repository, 'packages/golden-ui/store/session/tests/edit-operations.test.ts'),
	resolve(repository, 'packages/golden-ui/store/session/edit-session.ts'),
	resolve(repository, 'packages/golden-ui/store/session/tests/edit-session.test.ts'),
	resolve(repository, 'packages/golden-ui/store/session/intent-pipeline.ts'),
	resolve(repository, 'packages/golden-ui/store/session/history.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/project-files.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/session/project-graph-transition.svelte.ts'),
	resolve(repository, 'packages/golden-ui/store/session/tests/edit-barrier.test.ts'),
	resolve(repository, 'packages/golden-ui/store/session/tests/intent-barrier.test.ts'),
	resolve(repository, 'apps/chataigne/ui/src/lib/tests/workbenchProjectTransition.test.ts'),
	fileURLToPath(import.meta.url)
];
const fingerprints = () =>
	Object.fromEntries(
		sources.map((path) => [
			relative(repository, path).replaceAll('\\', '/'),
			hash(readFileSync(path))
		])
	);
mkdirSync(generated, { recursive: true });
const report = resolve(
	generated,
	`p11-dashboard-viewport-${new Date().toISOString().replaceAll(/[:.]/g, '-')}.json`
);
const result = {
	scope: barrierBaseline
		? 'barriers-save-baseline'
		: barriersOnly
			? 'barriers-only'
			: compositionOnly
				? 'composition-only'
				: orderedMoveOnly
					? 'ordered-move-only'
					: orderedOnly
						? 'ordered-only'
						: confirmedOnly
							? 'confirmed-only'
							: gesturesOnly
								? 'gestures-only'
								: creationOnly
									? 'creation-only'
									: editOnly
										? 'edit-only'
										: lifecycleOnly
											? 'lifecycle-only'
											: hierarchyOnly
												? 'hierarchy-only'
												: denseOnly
													? 'dense-only'
													: 'dashboard-correctness',
	baseline,
	browser: '',
	machine: {
		platform: platform(),
		release: release(),
		cpu: cpus()[0]?.model,
		logicalCpus: cpus().length,
		memoryBytes: totalmem(),
		node: process.version
	},
	source: fingerprints(),
	...(oldSave
		? { overrides: { revision: '85c34e6f', source: { [oldSavePath]: hash(oldSave) } } }
		: {}),
	cases: [],
	tiers: [],
	errors: [],
	assets: {},
	passed: false
};
let browser;
const record = (label, details = {}) => {
	result.cases.push({ label, ...details, passed: true });
};

async function scene(version, work) {
	const outDir = resolve(generated, `p11-dashboard-viewport-${version}`);
	await build({
		configFile: false,
		root,
		logLevel: 'warn',
		plugins: [
			...(oldSave
				? [
						{
							name: 'save-before-edit-barrier',
							enforce: 'pre',
							load(id) {
								if (relative(repository, id).replaceAll('\\', '/') === oldSavePath) return oldSave;
							}
						}
					]
				: []),
			...(version === 'baseline'
				? [
						{
							name: 'delivered-dashboard',
							enforce: 'pre',
							load(id) {
								if (id.includes('?')) return;
								const path = relative(repository, id).replaceAll('\\', '/');
								if (!path.startsWith(canvasPrefix)) return;
								return execFileSync('git', ['show', `${baseline}:${path}`], {
									cwd: repository,
									encoding: 'utf8',
									maxBuffer: 2_000_000
								});
							}
						}
					]
				: []),
			svelte({ configFile: false })
		],
		build: { outDir, emptyOutDir: true, rollupOptions: { input: resolve(root, 'dashboard.html') } }
	});
	result.assets[version] = Object.fromEntries(
		filesBelow(outDir).map((path) => [
			relative(outDir, path).replaceAll('\\', '/'),
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
	page.on('pageerror', (error) =>
		result.errors.push({ version, message: error.message, stack: error.stack })
	);
	try {
		await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dashboard.html`, {
			waitUntil: 'networkidle'
		});
		await page.waitForFunction(() => Boolean(window.dashboardFixture));
		return await work(page);
	} finally {
		await page.close();
		await server.close();
	}
}

async function paintedWidgets(page) {
	return page.locator('.dashboard-widget-shell').evaluateAll((shells) =>
		shells.map((shell) => {
			const bounds = shell.getBoundingClientRect();
			const slot = shell.closest('.dashboard-slot').getBoundingClientRect();
			const textNodes = document.createTreeWalker(shell, NodeFilter.SHOW_TEXT);
			let ownText = '';
			while (textNodes.nextNode()) {
				if (textNodes.currentNode.parentElement.closest('.dashboard-widget-shell') === shell)
					ownText += textNodes.currentNode.textContent;
			}
			const viewport = shell.closest('.dashboard-page-viewport').getBoundingClientRect();
			let left = Math.max(bounds.left, viewport.left),
				top = Math.max(bounds.top, viewport.top);
			let right = Math.min(bounds.right, viewport.right),
				bottom = Math.min(bounds.bottom, viewport.bottom);
			for (let ancestor = shell.parentElement; ancestor; ancestor = ancestor.parentElement) {
				const style = getComputedStyle(ancestor),
					rect = ancestor.getBoundingClientRect();
				if (style.overflowX !== 'visible') {
					left = Math.max(left, rect.left);
					right = Math.min(right, rect.right);
				}
				if (style.overflowY !== 'visible') {
					top = Math.max(top, rect.top);
					bottom = Math.min(bottom, rect.bottom);
				}
			}
			return {
				id: shell.dataset.nodeId,
				box: [slot.x, slot.y, slot.width, slot.height],
				shellBox: [bounds.x, bounds.y, bounds.width, bounds.height],
				text: ownText.replaceAll(/\s+/g, ' ').trim(),
				visible: right > left && bottom > top
			};
		})
	);
}

async function viewportChecks(page, references) {
	for (const editing of [false, true]) {
		await page.evaluate(
			(editing) => window.dashboardFixture.setup('free', editing, 1000, 'grid'),
			editing
		);
		const actual = await paintedWidgets(page);
		const expected = references.get(editing).filter((widget) => widget.visible);
		assert(actual.length <= 96, `Sparse mounting bound: ${actual.length}`);
		for (const widget of expected) {
			const candidate = actual.find((item) => item.id === widget.id);
			assert(candidate, `Missing painted widget ${widget.id}`);
			assert.equal(candidate.text, widget.text);
			for (let index = 0; index < 4; index++)
				assert(
					Math.abs(candidate.shellBox[index] - candidate.box[index]) <= 0.5,
					`Widget must fit its authored slot: ${candidate.shellBox} != ${candidate.box}`
				);
			for (let index = 0; index < 4; index++)
				assert(
					Math.abs(candidate.box[index] - widget.box[index]) <= 0.5,
					`Paint geometry changed for ${widget.id}: ${candidate.box} != ${widget.box}`
				);
		}
		record(`visible-preservation-${editing ? 'edit' : 'run'}`, {
			mounted: actual.length,
			painted: expected.length,
			paintedIds: expected.map((widget) => widget.id),
			mountedIds: actual.map((widget) => widget.id)
		});
	}
	await page.evaluate(() => window.dashboardFixture.setup('free', true, 1000, 'grid', true));
	const nested = await paintedWidgets(page);
	assert(nested.length > 1 && nested.length <= 40);
	for (const expected of references.get('nested').filter((widget) => widget.visible)) {
		const actual = nested.find((widget) => widget.id === expected.id);
		assert(actual, `Missing painted nested widget ${expected.id}`);
		assert.equal(actual.text, expected.text);
		for (let axis = 0; axis < 4; axis++)
			assert(Math.abs(actual.box[axis] - expected.box[axis]) <= 0.5);
	}
	record('nested-free-surface-clipping', { mounted: nested.length });
	for (const layout of ['free', 'horizontal', 'vertical', 'grid', 'accordion', 'tabs']) {
		for (const editing of [false, true]) {
			await page.evaluate(({ layout, editing }) => window.dashboardFixture.setup(layout, editing), {
				layout,
				editing
			});
			assert((await page.locator('.dashboard-widget-shell').count()) > 0);
			assert.deepEqual(result.errors, []);
		}
	}
	record('six-layout-edit-run-mounted-smoke');
	for (const target of process.argv.includes('--smoke')
		? [1000, 10000]
		: [1000, 10000, 20000, 50000, 100000]) {
		const started = performance.now();
		const setup = await page.evaluate((size) => window.dashboardFixture.setupTier(size), target);
		const readyMs = performance.now() - started;
		const far = setup.widgets - 1;
		const farId = setup.ids[far];
		const initial = await page.locator('.dashboard-widget-shell').count();
		assert(initial > 0 && initial <= 96);
		assert.equal(
			await page.locator(`[data-node-id="${farId}"]`).count(),
			setup.widgets <= 40 ? 1 : 0
		);
		await page.evaluate((index) => window.dashboardFixture.select(index), far);
		const outcome = await page.evaluate(() => window.dashboardFixture.command('view.frame'));
		assert.equal(outcome.handled, true);
		const selected = page.locator(`.dashboard-widget-shell[data-node-id="${farId}"]`);
		assert.equal(await selected.count(), 1, 'Frame must reach an unmounted selection');
		assert.equal(
			await selected.evaluate((element) => element.classList.contains('selected')),
			true
		);
		const framed = await selected.evaluate((element) => {
			const rect = element.getBoundingClientRect(),
				viewport = element.closest('.dashboard-page-viewport').getBoundingClientRect();
			return {
				dx: Math.abs(rect.x + rect.width / 2 - viewport.x - viewport.width / 2),
				dy: Math.abs(rect.y + rect.height / 2 - viewport.y - viewport.height / 2),
				boxes: [element, element.parentElement, element.parentElement.parentElement].map((item) => {
					const box = item.getBoundingClientRect();
					const style = getComputedStyle(item);
					return {
						classes: item.className,
						width: box.width,
						height: box.height,
						top: box.top,
						blockSize: style.blockSize,
						boxSizing: style.boxSizing
					};
				})
			};
		});
		assert(framed.dx <= 0.5 && framed.dy <= 0.5, `Frame center: ${JSON.stringify(framed)}`);
		await page.evaluate(() => window.dashboardFixture.command('view.home'));
		assert.equal(await selected.count(), 0);
		const region = await page.evaluate((index) => {
			const layout = document.querySelector('.dashboard-page > .dashboard-layout');
			const bounds = layout.getBoundingClientRect(),
				scale = bounds.width / layout.clientWidth;
			return {
				left: bounds.left + ((index % 40) * 240 + 20) * scale,
				top: bounds.top + (Math.floor(index / 40) * 112 + 20) * scale,
				width: 5 * scale,
				height: 5 * scale
			};
		}, far);
		assert.deepEqual(
			await page.evaluate((rect) => window.dashboardFixture.selectRegion(rect), region),
			[farId]
		);
		assert.deepEqual(
			await page.evaluate((rect) => window.dashboardFixture.selectRegion(rect, 'TOGGLE'), region),
			[]
		);
		await page.evaluate((index) => window.dashboardFixture.reveal(index), far);
		assert.equal(await selected.count(), 1);
		assert((await page.locator('.dashboard-widget-shell').count()) <= 96);
		result.tiers.push({
			targetAuthoredNodes: target,
			authoredNodes: setup.authoredNodes,
			widgets: setup.widgets,
			readyMs,
			initialMounted: initial,
			framed,
			passed: true
		});
		console.log(JSON.stringify(result.tiers.at(-1)));
	}
	await page.evaluate(() => window.dashboardFixture.setup('free', false, 1000, 'grid'));
	const ids = await page.evaluate(() => window.dashboardFixture.focus(4));
	const focused = await page.locator(`[data-node-id="${ids}"] input`).elementHandle();
	await page.evaluate(() => window.dashboardFixture.reorder());
	assert.equal(await focused.evaluate((element) => element === document.activeElement), true);
	await page.evaluate(() => window.dashboardFixture.view({ panX: 0, panY: 0, zoom: 4 }));
	await page.evaluate(() => window.dashboardFixture.reveal(0));
	assert.equal(await focused.evaluate((element) => element === document.activeElement), true);
	assert.equal(await page.locator(`[data-node-id="${ids}"]`).count(), 1);
	await page.evaluate(() => window.dashboardFixture.rootFont(20));
	assert.equal(await focused.evaluate((element) => element === document.activeElement), true);
	await page.evaluate(() => window.dashboardFixture.rootFont(16));
	assert((await page.locator('.dashboard-widget-shell').count()) <= 97);
	record('focused-control-retention-and-root-font');
	const beforeRetirement = await focused.evaluate((element) => ({
		focused: element === document.activeElement,
		connected: element.isConnected
	}));
	await page.evaluate(() => window.dashboardFixture.remove(4));
	assert.equal(await page.locator(`[data-node-id="${ids}"]`).count(), 0);
	const retiredFocus = await page.evaluate(() => ({
		tag: document.activeElement?.tagName,
		classes: document.activeElement?.className
	}));
	assert(
		retiredFocus.classes?.includes('dashboard-page-viewport'),
		JSON.stringify({ beforeRetirement, retiredFocus, errors: result.errors })
	);
	record('retired-focus-fallback');
	await page.evaluate(() => window.dashboardFixture.setup('free', true, 1000, 'grid'));
	const native = await page
		.locator('.generic-text-display-value')
		.first()
		.evaluate((element) => {
			const range = document.createRange();
			range.selectNodeContents(element);
			const selection = window.getSelection();
			selection.removeAllRanges();
			selection.addRange(range);
			return { text: selection.toString(), id: element.closest('[data-node-id]').dataset.nodeId };
		});
	await page.evaluate(() => window.dashboardFixture.reveal(999));
	assert.equal(await page.locator(`[data-node-id="${native.id}"]`).count(), 1);
	assert.equal(await page.evaluate(() => window.getSelection().toString()), native.text);
	await page.evaluate(async () => {
		window.getSelection().removeAllRanges();
		await window.dashboardFixture.reveal(998);
	});
	assert.equal(await page.locator(`[data-node-id="${native.id}"]`).count(), 0);
	record('native-range-retention-and-release');
	await page.evaluate(() => window.dashboardFixture.setup('free', true, 1000, 'grid'));
	const movedId = await page
		.locator('.dashboard-widget-shell')
		.first()
		.getAttribute('data-node-id');
	await page.evaluate(() =>
		window.dashboardFixture.update(0, 'position', { kind: 'vec2', value: [9000, 9000] })
	);
	assert.equal(await page.locator(`[data-node-id="${movedId}"]`).count(), 0);
	await page.evaluate(() =>
		window.dashboardFixture.update(0, 'position', { kind: 'vec2', value: [0, 0] })
	);
	assert.equal(await page.locator(`[data-node-id="${movedId}"]`).count(), 1);
	record('live-position-leaves-and-enters-viewport');
	await page.evaluate(() =>
		window.dashboardFixture.update(0, 'width', { kind: 'css_value', value: 10, unit: 'em' })
	);
	await page.evaluate(() => window.dashboardFixture.localFont(12));
	const emBoundary = await page.evaluate(() => {
		const layout = document.querySelector('.dashboard-page > .dashboard-layout');
		const bounds = layout.getBoundingClientRect(),
			scale = bounds.width / layout.clientWidth;
		const slot = layout.querySelector('.dashboard-slot').getBoundingClientRect();
		return {
			width: slot.width / scale,
			rect: {
				left: bounds.left + 130 * scale,
				top: bounds.top + 20 * scale,
				width: 2 * scale,
				height: 2 * scale
			}
		};
	});
	assert(Math.abs(emBoundary.width - 120) <= 0.5);
	assert.deepEqual(
		await page.evaluate((rect) => window.dashboardFixture.selectRegion(rect), emBoundary.rect),
		[]
	);
	await page.evaluate(() => window.dashboardFixture.localFont(20));
	assert.deepEqual(
		await page.evaluate((rect) => window.dashboardFixture.selectRegion(rect), emBoundary.rect),
		[Number(movedId)]
	);
	await page.evaluate(() => window.dashboardFixture.localFont(16));
	record('inherited-em-font-and-logical-boundary');
	const hierarchy = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 6, 'grid', true)
	);
	await page.evaluate(() =>
		window.dashboardFixture.update(-2, 'position', { kind: 'vec2', value: [6000, 6000] })
	);
	assert.equal(await page.locator(`[data-node-id="${hierarchy.containerId}"]`).count(), 0);
	await page.evaluate(() => window.dashboardFixture.select(0));
	assert.equal(
		(await page.evaluate(() => window.dashboardFixture.command('view.frame'))).handled,
		true
	);
	const framedChild = page.locator(`.dashboard-widget-shell[data-node-id="${hierarchy.ids[0]}"]`);
	assert.equal(await framedChild.count(), 1, 'Frame must materialize an offscreen ancestry');
	const hierarchyCenter = await framedChild.evaluate((element) => {
		const rect = element.getBoundingClientRect(),
			viewport = element.closest('.dashboard-page-viewport').getBoundingClientRect();
		return {
			dx: Math.abs(rect.x + rect.width / 2 - viewport.x - viewport.width / 2),
			dy: Math.abs(rect.y + rect.height / 2 - viewport.y - viewport.height / 2)
		};
	});
	assert(hierarchyCenter.dx <= 0.5 && hierarchyCenter.dy <= 0.5, JSON.stringify(hierarchyCenter));
	record('frame-beneath-unmounted-container', hierarchyCenter);
	const forest = await page.evaluate(() => window.dashboardFixture.setupForest(100));
	assert.equal(await page.locator('.dashboard-container').count(), 0);
	const peak = await page.evaluate(async () => {
		let maximum = 0;
		const observer = new MutationObserver(() => {
			maximum = Math.max(maximum, document.querySelectorAll('.dashboard-widget-shell').length);
		});
		observer.observe(document.querySelector('main'), { childList: true, subtree: true });
		try {
			await window.dashboardFixture.selectAllWidgets();
			await window.dashboardFixture.command('view.frame');
		} finally {
			observer.disconnect();
		}
		return maximum;
	});
	assert(peak > 0 && peak <= 96, `Ancestry preparation mounted ${peak} shells`);
	const forestCenter = await page.evaluate((count) => {
		const frame = document.querySelector('.dashboard-page-frame').getBoundingClientRect();
		const rootLayout = document
			.querySelector('.dashboard-page > .dashboard-layout')
			.getBoundingClientRect();
		const container = document.querySelector('.dashboard-container');
		const containerRect = container.getBoundingClientRect();
		const childLayout = container.querySelector('.dashboard-layout').getBoundingClientRect();
		const viewport = container.closest('.dashboard-page-viewport').getBoundingClientRect();
		const scale = frame.width / 1200;
		const x = 6000 + (count - 1) * 500 + 96;
		const y = 6000 + 40;
		return {
			dx: Math.abs(
				rootLayout.left +
					x * scale +
					childLayout.left -
					containerRect.left -
					viewport.left -
					viewport.width / 2
			),
			dy: Math.abs(
				rootLayout.top +
					y * scale +
					childLayout.top -
					containerRect.top -
					viewport.top -
					viewport.height / 2
			)
		};
	}, forest.ids.length);
	assert(forestCenter.dx <= 0.5 && forestCenter.dy <= 0.5, JSON.stringify(forestCenter));
	record('batched-frame-across-100-offscreen-containers', { peakMounted: peak, ...forestCenter });
	const canceledFrame = await page.evaluate(() => window.dashboardFixture.cancelFrameWithView());
	assert.equal(canceledFrame.handled, true);
	assert.match(canceledFrame.transform, /^translate\(17px, 19px\)/);
	record('pending-frame-yields-to-new-navigation');
	assert.deepEqual(result.errors, []);
}

try {
	browser = await chromium.launch({
		headless: true,
		...(process.env.GOLDEN_UI_BROWSER_PATH
			? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
			: { channel: process.platform === 'win32' ? 'msedge' : undefined })
	});
	result.browser = await browser.version();
	const references = !fullRun
		? null
		: await scene('baseline', async (page) => {
				const scenes = new Map();
				for (const editing of [false, true]) {
					await page.evaluate(
						(editing) => window.dashboardFixture.setup('free', editing, 1000, 'grid'),
						editing
					);
					const widgets = await paintedWidgets(page);
					assert.equal(widgets.length, 1000);
					scenes.set(editing, widgets);
				}
				await page.evaluate(() => window.dashboardFixture.setup('free', true, 1000, 'grid', true));
				const nested = await paintedWidgets(page);
				assert.equal(nested.length, 1001);
				scenes.set('nested', nested);
				return scenes;
			});
	await scene('current', async (page) => {
		if (references) await viewportChecks(page, references);
		if (fullRun || denseOnly) await denseChecks(page, record);
		if (fullRun || hierarchyOnly) await hierarchyChecks(page, record);
		if (fullRun || lifecycleOnly) await lifecycleChecks(page, record);
		if (fullRun || editOnly) await editLifecycleChecks(page, record);
		if (fullRun || creationOnly) await creationChecks(page, record);
		if (fullRun || gesturesOnly) await gestureChecks(page, record);
		if (fullRun || confirmedOnly) await confirmedChecks(page, record);
		if (fullRun || orderedOnly) await orderedChecks(page, record);
		if (fullRun || orderedOnly || orderedMoveOnly) await orderedMoveChecks(page, record);
		if (fullRun || compositionOnly) await compositionChecks(page, record);
		if (fullRun || barriersOnly) await barrierChecks(page, record);
		assert.deepEqual(result.errors, []);
	});
	assert.deepEqual(fingerprints(), result.source, 'Qualified sources changed');
	result.passed = true;
	console.log(
		JSON.stringify({ passed: true, cases: result.cases.length, tiers: result.tiers.length, report })
	);
} catch (error) {
	result.failure = String(error);
	throw error;
} finally {
	writeFileSync(report, JSON.stringify(result, null, 2) + '\n');
	await browser?.close();
}
