// Owned native scene for same-frame image/overlay acceptance. Capture it with native window pixels,
// never Playwright screenshots (which omit the underlay). No production instrumentation.
// Use a fresh output directory; write its `finish` marker after capture to stop normally.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-native-alignment-${Date.now()}`);
await mkdir(directory, { recursive: true });
const exists = async (name) => !!(await stat(path.join(directory, name)).catch(() => null));
assert(!(await exists('ready.json')), 'Alignment fixture requires a fresh output directory');
const native = await openNativeEditor();
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
}
async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit('media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Alignment red'
	});
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const { screen, surface: reference } = await edit('screens', {
		kind: 'CreateScreen',
		label: 'Alignment screen'
	});
	await edit('screens', {
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Alignment-${screen}`, extent: { width: 800, height: 600 } }
		}
	});
	const { snapshot } = await edit('screens', { kind: 'InspectSurface', reference });
	await edit('screens', {
		kind: 'ConfigureSurface',
		reference,
		definition: {
			...snapshot.definition,
			source,
			quad: {
				corners: [
					{ x: 0.2, y: 0.2 },
					{ x: 0.8, y: 0.2 },
					{ x: 0.8, y: 0.8 },
					{ x: 0.2, y: 0.8 }
				]
			}
		}
	});
	const page = native.page;
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Manager$/ })
		.click();
	await page
		.locator('section[aria-label="Screen Manager"]')
		.getByRole('button', { name: snapshot.label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Warping$/ })
		.click();
	const sourceEditor = process.argv.includes('--source');
	if (sourceEditor)
		await page
			.getByRole('tab')
			.filter({ hasText: /^Surface Source$/ })
			.click();
	const view = page.getByRole('application', {
		name: sourceEditor ? 'Surface source view' : 'Screen view',
		exact: true
	});
	await view
		.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
		.waitFor();
	await view.getByRole('button', { name: 'Home view', exact: true }).click();
	await delay(250);
	const bounds = await view.locator('.active-zone').boundingBox();
	assert(bounds);
	const metadata = {
		editor: sourceEditor ? 'source' : 'screen',
		pid: native.app.pid,
		directory,
		base: native.base,
		reference,
		viewport: await page.evaluate(() => ({
			width: innerWidth,
			height: innerHeight,
			dpr: devicePixelRatio
		})),
		active: bounds
	};
	await writeFile(path.join(directory, 'ready.json'), JSON.stringify(metadata, null, 2));
	console.log(JSON.stringify({ ready: metadata }));
	if (process.argv.includes('--zoom')) {
		const hwnd = Number(
			execFileSync(
				'powershell.exe',
				[
					'-NoProfile',
					'-Command',
					`(Get-Process -Id ${native.app.pid}).MainWindowHandle.ToInt64()`
				],
				{ windowsHide: true, encoding: 'utf8' }
			).trim()
		);
		assert(Number.isSafeInteger(hwnd) && hwnd > 0);
		const capture = spawn(
			'python',
			[
				'apps/mapgyver/tests/native/visible_zoom.py',
				String(hwnd),
				String(native.app.pid),
				path.join(directory, 'zoom')
			],
			{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }
		);
		const captureTimeout = setTimeout(() => capture.kill(), 10000);
		let output = '',
			errors = '';
		const complete = new Promise((resolve, reject) => {
			capture.once('error', reject);
			capture.once('close', (code) =>
				code === 0 ? resolve() : reject(new Error(`capture ${code}: ${errors}`))
			);
		});
		capture.stderr.on('data', (data) => {
			errors += data;
		});
		try {
			await new Promise((resolve, reject) => {
				capture.stdout.on('data', (data) => {
					output += data;
					if (output.includes('"capture_ready": true')) resolve();
				});
				complete.then(() => reject(new Error('capture ended before readiness')), reject);
			});
			await view.evaluate((element) => {
				window.__alignmentZoom = [];
				const until = performance.now() + 800;
				const sample = () => {
					window.__alignmentZoom.push({
						at: performance.now(),
						zoom: Number(element.querySelector('output[data-zoom]').dataset.zoom)
					});
					if (performance.now() < until) requestAnimationFrame(sample);
				};
				requestAnimationFrame(sample);
			});
			const box = await view.locator('.active-zone').boundingBox();
			await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			await page.mouse.wheel(0, 120);
			await complete;
			const pixels = JSON.parse(
				await readFile(path.join(directory, 'zoom', 'zoom-pixels.json'), 'utf8')
			);
			assert(
				new Set(pixels.samples.map((sample) => sample.edges[1] - sample.edges[0])).size >= 3,
				'captured pixels must include an intermediate native zoom width'
			);
			assert(
				pixels.baseline_offsets_px.every((value) => value >= 0.5 && value <= 2.5),
				'visible native edges must start immediately inside the blue stroke'
			);
			assert(
				pixels.max_edge_offset_change_px <= 1,
				'native and blue edge offsets must stay aligned during sampled zoom'
			);
			await writeFile(
				path.join(directory, 'zoom-dom.json'),
				JSON.stringify(await page.evaluate(() => window.__alignmentZoom), null, 2)
			);
			console.log(output.trim());
		} finally {
			clearTimeout(captureTimeout);
			if (capture.exitCode === null) capture.kill();
		}
	} else {
		const deadline = Date.now() + 5 * 60_000;
		while (!(await exists('finish'))) {
			assert(Date.now() < deadline, 'alignment fixture timeout');
			await delay(50);
		}
	}
	console.log(
		JSON.stringify({
			result: 'fixture complete',
			project: (await edit('screens', { kind: 'InspectSurface', reference })).snapshot
		})
	);
} finally {
	await native.close();
}
