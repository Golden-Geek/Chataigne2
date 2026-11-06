// Operate only a running owned native-alignment fixture, then release its normal shutdown.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright-core';
import { captureAlignment, ownedWindowGeometry, ownedWindowHandle } from './capture-alignment.mjs';

const directory = path.resolve(process.argv[2]);
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
assert(ready.landmark?.independent);
const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
const pages = browser
	.contexts()
	.flatMap((context) => context.pages())
	.filter((page) => page.url().startsWith(ready.base + '/'));
assert.equal(pages.length, 1);
const page = pages[0];
page.setDefaultTimeout(8000);
const hwnd = ownedWindowHandle(ready.pid);
const originalWindow = ownedWindowGeometry(ready.pid, hwnd);
const view = page.getByRole('application', {
	name: ready.editor === 'screen' ? 'Screen view' : 'Surface source view',
	exact: true
});
const editorTab = () =>
	page.getByRole('tab').filter({
		hasText: ready.editor === 'screen' ? /^Screen Warping(?::|$)/ : /^Surface Source(?::|$)/
	});
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const stages = [];
const actions = [];
async function capture(stage) {
	const result = await captureAlignment({ directory, stage, page });
	assert.equal(result.hwnd, Number(hwnd));
	assert.deepEqual(ownedWindowGeometry(ready.pid, hwnd), originalWindow);
	stages.push(result);
	return result;
}
async function dragSash(delta) {
	const before = await view.locator('.active-zone').boundingBox();
	const group = view.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	const box = await group.boundingBox();
	assert(box && before);
	const sashes = await page
		.locator('.dv-sash.dv-enabled')
		.evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().toJSON()));
	const candidates = sashes.filter(
		(rect) => rect.height > 100 && Math.abs(rect.x + rect.width / 2 - box.x - box.width) < 12
	);
	assert.equal(candidates.length, 1, 'Exactly one real right-hand dock sash required');
	const sash = candidates[0];
	const x = sash.x + sash.width / 2,
		y = sash.y + sash.height / 2;
	await page.mouse.move(x, y);
	await page.mouse.down();
	await page.mouse.move(x + delta, y, { steps: 12 });
	await page.mouse.up();
	await page.waitForFunction(
		({ name, before }) => {
			const active = document.querySelector(
				`[role="application"][aria-label="${name}"] .active-zone`
			);
			return active && Math.abs(active.getBoundingClientRect().width - before) > 20;
		},
		{ name: await view.getAttribute('aria-label'), before: before.width }
	);
	const after = await view.locator('.active-zone').boundingBox();
	actions.push({ action: 'dock-sash-resize', delta, before, after });
}
try {
	await page.evaluate(() => {
		window.__editorLayoutInputs = [];
		for (const type of ['pointerdown', 'pointerup', 'wheel', 'click'])
			window.addEventListener(
				type,
				(event) => {
					if (window.__editorLayoutInputs.length < 128)
						window.__editorLayoutInputs.push({ type, trusted: event.isTrusted });
				},
				{ capture: true, passive: true }
			);
	});
	await capture('initial');
	await dragSash(-80);
	await capture('dock-narrow');
	await dragSash(80);
	const restored = await capture('dock-restored');
	const inspector = page.locator('.inspector-content').filter({ visible: true });
	assert.equal(await inspector.count(), 1);
	const scroll = await inspector.evaluate((element) => ({
		top: element.scrollTop,
		maximum: element.scrollHeight - element.clientHeight
	}));
	assert(scroll.maximum > 50, 'Real overflow required for Inspector wheel check');
	await inspector.hover();
	await page.mouse.wheel(0, 600);
	await page.waitForFunction(() =>
		[...document.querySelectorAll('.inspector-content')].some((element) => element.scrollTop > 50)
	);
	actions.push({
		action: 'inspector-scroll-down',
		before: scroll.top,
		after: await inspector.evaluate((element) => element.scrollTop)
	});
	await capture('inspector-scrolled');
	await inspector.hover();
	await page.mouse.wheel(0, -1000);
	await page.waitForFunction(() =>
		[...document.querySelectorAll('.inspector-content')].every((element) => element.scrollTop === 0)
	);
	await capture('inspector-top');
	const group = editorTab().locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Logger', { exact: true }) })
		.click();
	await group
		.getByRole('tab')
		.filter({ hasText: /^Logger$/ })
		.waitFor();
	await page.waitForFunction(
		() =>
			![...document.querySelectorAll('.native-preview')].some(
				(element) =>
					element.getBoundingClientRect().width > 0 &&
					element.getBoundingClientRect().height > 0 &&
					element.dataset.previewInstalled === 'true'
			) && !document.querySelector('[data-native-preview-retirement]')
	);
	const output = path.join(directory, 'editor-hidden');
	const hiddenCapture = spawnSync(
		'python',
		[
			'apps/mapgyver/tests/native/window_capture.py',
			'--visible',
			'--hwnd',
			hwnd,
			'--pid',
			String(ready.pid),
			output
		],
		{ windowsHide: true, encoding: 'utf8' }
	);
	// Exit 2 means the image lacks the red/blue alignment fixture, as expected here.
	// Ownership/capture failures still reject; the independent absence oracle follows.
	assert([0, 2].includes(hiddenCapture.status), hiddenCapture.stderr);
	const edges = restored.alignment.red_edges;
	const roi = [
		Math.ceil(edges[0] + 4),
		Math.ceil(edges[2] + 4),
		Math.floor(edges[1] - 4),
		Math.floor(edges[3] - 4)
	];
	const script = [
		'import json,sys',
		'from PIL import Image',
		'roi=json.loads(sys.argv[2])',
		'image=Image.open(sys.argv[1]).convert("RGB").crop(roi)',
		'count=sum(r>160 and g<80 and b<100 for r,g,b in image.getdata())',
		'print(json.dumps({"roi":roi,"pixels":image.width*image.height,"native_red_pixels":count}))',
		'assert count==0,"Hidden native landmark must retire from its former physical region"'
	].join('\n');
	const hidden = JSON.parse(
		execFileSync('python', ['-c', script, path.join(output, 'baseline.png'), JSON.stringify(roi)], {
			encoding: 'utf8',
			windowsHide: true
		})
	);
	hidden.alignment_capture_exit = hiddenCapture.status;
	await writeFile(path.join(output, 'retirement.json'), JSON.stringify(hidden, null, 2));
	assert.deepEqual(ownedWindowGeometry(ready.pid, hwnd), originalWindow);
	await editorTab().click();
	await capture('editor-shown');
	const inputs = await page.evaluate(() => window.__editorLayoutInputs);
	assert(inputs.length > 10 && inputs.length < 128 && inputs.every((event) => event.trusted));
	for (const type of ['pointerdown', 'pointerup', 'wheel', 'click'])
		assert(
			inputs.some((event) => event.type === type),
			'Missing trusted ' + type
		);
	assert.deepEqual(errors, []);
	const result = {
		result: 'pass',
		pid: ready.pid,
		hwnd: Number(hwnd),
		editor: ready.editor,
		stages,
		actions,
		hidden,
		inputs,
		page_errors: errors,
		scope:
			'Settled native dock-sash resize, Inspector wheel scroll and tab hide/show. No every-frame, mixed-monitor or latency acceptance.'
	};
	await writeFile(path.join(directory, 'editor-layout.json'), JSON.stringify(result, null, 2));
	console.log(
		JSON.stringify({
			result: 'editor-layout pass',
			pid: ready.pid,
			maximum_error_px: Math.max(
				...stages.flatMap(({ alignment }) => [
					alignment.image_to_projection_max_px,
					alignment.overlay_to_projection_max_px,
					alignment.image_to_overlay_max_px
				])
			),
			hidden
		})
	);
	// Existing fixture consumes these markers and follows its ordinary owned-app shutdown.
	await writeFile(path.join(directory, 'finish'), 'finish\n');
	await writeFile(path.join(directory, 'capture'), 'capture\n');
	process.exit(0);
} catch (error) {
	await writeFile(
		path.join(directory, 'editor-layout-failure.json'),
		JSON.stringify({ error: String(error), stages, actions, page_errors: errors }, null, 2)
	);
	console.error(error);
	await editorTab()
		.click()
		.catch(() => {});
	// Release the existing fixture even on failure; this observer's nonzero exit remains authoritative.
	await writeFile(path.join(directory, 'finish'), 'finish\n');
	await writeFile(path.join(directory, 'capture'), 'capture\n');
	process.exit(1);
}
