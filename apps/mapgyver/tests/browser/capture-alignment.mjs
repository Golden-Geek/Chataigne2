// Read-only CDP geometry and physical pixels of one caller-owned alignment fixture.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export function ownedWindowHandle(pid) {
	const hwnd = execFileSync(
		'powershell.exe',
		['-NoProfile', '-Command', '(Get-Process -Id ' + pid + ').MainWindowHandle.ToInt64()'],
		{ encoding: 'utf8', windowsHide: true }
	).trim();
	assert(/^[1-9]\d*$/.test(hwnd));
	return hwnd;
}

export function ownedWindowGeometry(pid, hwnd) {
	return JSON.parse(
		execFileSync(
			'python',
			[
				'apps/mapgyver/tests/native/window_capture.py',
				'--geometry-only',
				'--hwnd',
				hwnd,
				'--pid',
				String(pid)
			],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
}

export async function captureAlignment({ directory, stage, maximized = false, page }) {
	assert(/^[a-z][a-z0-9-]{0,40}$/.test(stage ?? ''), 'Explicit bounded capture stage required');
	const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
	assert(ready.landmark?.independent, 'Independent landmark fixture required');
	assert(['screen', 'source'].includes(ready.editor));
	if (!page) {
		assert(/^http:\/\/127\.0\.0\.1:\d+$/.test(ready.cdp), 'Owned loopback CDP endpoint required');
		const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
		const pages = browser
			.contexts()
			.flatMap((context) => context.pages())
			.filter((candidate) => candidate.url().startsWith(ready.base + '/'));
		assert.equal(pages.length, 1, 'Exactly one fixture page must match its owned endpoint');
		page = pages[0];
	}
	assert(page.url().startsWith(ready.base + '/'));
	const view = page.getByRole('application', {
		name: ready.editor === 'screen' ? 'Screen view' : 'Surface source view',
		exact: true
	});
	await view
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	const projection = () =>
		view.evaluate((element) => {
			const rect = element.querySelector('.active-zone').getBoundingClientRect();
			return {
				viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
				active: { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
			};
		});
	// ResizeObserver updates the editor after the WebView viewport changes. Require a
	// quiet settled projection before capture; the exact post-capture guard still applies.
	let geometry = await projection();
	let stableSince = Date.now();
	const deadline = stableSince + 5000;
	while (Date.now() - stableSince < 250) {
		assert(Date.now() < deadline, 'Owned editor projection did not settle before capture');
		await delay(50);
		const observed = await projection();
		if (JSON.stringify(observed) !== JSON.stringify(geometry)) {
			geometry = observed;
			stableSince = Date.now();
		}
	}
	const hwnd = ownedWindowHandle(ready.pid);
	const output = path.join(directory, stage);
	execFileSync(
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
		{ windowsHide: true }
	);
	const captured = JSON.parse(await readFile(path.join(output, 'capture.json'), 'utf8'));
	assert.equal(captured.window_maximized, maximized, 'Actual OS maximized state must match stage');
	assert.deepEqual(
		await projection(),
		geometry,
		'DOM projection must stay fixed throughout capture'
	);
	await writeFile(path.join(output, 'geometry.json'), JSON.stringify(geometry, null, 2));
	const script = [
		'import sys,json',
		'from pathlib import Path',
		'from PIL import Image',
		"sys.path.insert(0,'apps/mapgyver/tests/native')",
		'from visible_zoom import independent_landmark_alignment',
		'p=Path(sys.argv[1])',
		"v=independent_landmark_alignment(Image.open(p/'baseline.png'),json.loads((p/'geometry.json').read_text(encoding='utf8')),json.loads((p/'capture.json').read_text(encoding='utf8')),sys.argv[2])",
		"(p/'alignment.json').write_text(json.dumps(v,indent=2)+chr(10),encoding='utf8')",
		"assert v['settled_alignment_verified'],v",
		'print(json.dumps(v))'
	].join('\n');
	const alignment = JSON.parse(
		execFileSync('python', ['-c', script, output, ready.editor], {
			encoding: 'utf8',
			windowsHide: true
		})
	);
	return {
		stage,
		pid: ready.pid,
		hwnd: Number(hwnd),
		dpr: geometry.viewport.dpr,
		capture: captured,
		geometry,
		alignment
	};
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
	const result = await captureAlignment({
		directory: path.resolve(process.argv[2]),
		stage: process.argv[3],
		maximized: process.argv.includes('--maximized')
	});
	console.log(JSON.stringify(result));
	// Disconnect this observer without sending Browser.close to the WebView.
	process.exit(0);
}
