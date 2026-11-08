// Read-only DOM/native hit metadata plus guarded physical desktop pixels of one owned cohort.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]),
	stage = process.argv[3];
assert(/^[a-z][a-z0-9-]{0,40}$/.test(stage));
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
const browser = await chromium.connectOverCDP(ready.cdp);
const pages = browser
	.contexts()
	.flatMap((context) => context.pages())
	.filter((page) => page.url().startsWith(ready.base));
assert.equal(pages.length, 1);
const page = pages[0];
const geometry = () =>
	page.evaluate(() =>
		[...document.querySelectorAll('[role=application]')].map((view) => {
			const r = view.querySelector('.active-zone')?.getBoundingClientRect();
			return r ? [r.x, r.y, r.width, r.height, devicePixelRatio] : null;
		})
	);
let previous = JSON.stringify(await geometry()),
	stable = Date.now();
const deadline = stable + 5000;
while (Date.now() - stable < 250) {
	assert(Date.now() < deadline, 'Native viewport did not settle');
	await delay(50);
	const next = JSON.stringify(await geometry());
	if (next !== previous) {
		previous = next;
		stable = Date.now();
	}
}
const metadata = await page.evaluate(async () => {
	const view = [...document.querySelectorAll('[role=application]')].find(
		(element) => element.getBoundingClientRect().width > 0
	);
	if (!view) return { empty: true, title: document.title, dpr: devicePixelRatio };
	const rect = (element) => {
		const value = element.getBoundingClientRect();
		return { x: value.x, y: value.y, width: value.width, height: value.height };
	};
	const preview = view.querySelector('.native-preview'),
		active = view.querySelector('.active-zone');
	const identity = preview && {
		session: preview.dataset.previewSession,
		view: preview.dataset.previewView,
		source: preview.dataset.previewSource
	};
	const hits = [];
	if (identity?.session && identity.view && identity.source) {
		for (const element of view.querySelectorAll('[data-native-hit-id]')) {
			const bounds = rect(element);
			if (!bounds.width || !bounds.height) continue;
			const point = [bounds.x + bounds.width / 2, bounds.y + bounds.height / 2];
			const native = JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'Pointer', ...identity, point })
				})
			);
			const domHit = document
				.elementFromPoint(...point)
				?.closest('[data-native-hit-id]')
				?.getAttribute('data-native-hit-id');
			hits.push({ id: element.dataset.nativeHitId, bounds, point, native, domHit });
		}
	}
	const token =
		getComputedStyle(document.documentElement).getPropertyValue('--gc-color-panel').trim() ||
		'#252525';
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 1;
	const context = canvas.getContext('2d');
	context.fillStyle = token;
	context.fillRect(0, 0, 1, 1);
	return {
		title: document.title,
		label: view.getAttribute('aria-label'),
		dpr: devicePixelRatio,
		viewport: { width: innerWidth, height: innerHeight },
		rootFont: getComputedStyle(document.documentElement).fontSize,
		view: rect(view),
		active: active && rect(active),
		toolbar: view.querySelector('[role=toolbar]') && rect(view.querySelector('[role=toolbar]')),
		preview: preview && { bounds: rect(preview), dataset: { ...preview.dataset } },
		theme: { token, rgba: [...context.getImageData(0, 0, 1, 1).data] },
		hits
	};
});
await writeFile(path.join(directory, `${stage}-dom.json`), JSON.stringify(metadata, null, 2));
const output = path.join(directory, stage);
let capture;
try {
	capture = execFileSync(
		'python',
		[
			'apps/mapgyver/tests/native/window_capture.py',
			'--visible',
			'--stationary-cursor',
			'--hwnd',
			ready.hwnd,
			'--pid',
			String(ready.pid),
			output
		],
		{ encoding: 'utf8', windowsHide: true }
	);
} catch (error) {
	if (error.status !== 2) throw error;
	capture = error.stdout.toString(); // Missing/empty scenes intentionally contain no red landmark.
}
console.log(JSON.stringify({ stage, metadata, capture: JSON.parse(capture) }));
assert.equal(
	JSON.stringify(await geometry()),
	previous,
	'Projection changed during physical capture'
);
// Disconnect this read-only client without closing the fixture's native WebView.
process.exit(0);
