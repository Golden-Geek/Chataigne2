// Owned native scene for same-frame image/overlay acceptance. Capture it with native window pixels,
// never Playwright screenshots (which omit the underlay). No production instrumentation.
// Use a fresh output directory; write its `finish` marker after capture to stop normally.
import assert from 'node:assert/strict';
import { execFile, execFileSync, spawn } from 'node:child_process';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { crc32, deflateSync } from 'node:zlib';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { measureVisibleLatency } from './native-latency.mjs';
import { verifyNativeWindowLayout } from './native-window-layout.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-native-alignment-${Date.now()}`);
await mkdir(directory, { recursive: true });
const exists = async (name) => !!(await stat(path.join(directory, name)).catch(() => null));
assert(!(await exists('ready.json')), 'Alignment fixture requires a fresh output directory');
const sourceEditor = process.argv.includes('--source');
const independentLandmark = process.argv.includes('--independent-landmark');
assert(
	!independentLandmark || !process.argv.includes('--zoom'),
	'Independent landmarks require their projected geometry oracle, not the coincident-edge zoom oracle'
);
const pictureFixture = sourceEditor || independentLandmark;
const redRect = independentLandmark ? [80, 60, 640, 480] : [160, 120, 480, 360];
const holdArgument = process.argv.find((value) => value.startsWith('--hold-seconds='));
const holdSeconds = Number(holdArgument?.slice('--hold-seconds='.length) ?? 300);
assert(
	Number.isInteger(holdSeconds) && holdSeconds >= 30 && holdSeconds <= 480,
	'Alignment hold must be 30–480 seconds, below the owned host watchdog'
);
const windowStateArgument = process.argv.find((value) => value.startsWith('--window-state='));
const windowState = windowStateArgument
	? JSON.parse(
			await readFile(path.resolve(windowStateArgument.slice('--window-state='.length)), 'utf8')
		)
	: undefined;
const gpu = process.argv.find((value) => value.startsWith('--gpu='))?.slice('--gpu='.length);
assert(!gpu || /^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu), 'Exact GPU LUID required');
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const hardware = gpu
	? JSON.parse(
			execFileSync(observer, ['--role', 'hardware'], { encoding: 'utf8', windowsHide: true })
		)
	: undefined;
const native = await openNativeEditor({
	windowState,
	prepareProfile: gpu
		? async (root) => {
				const preferences = path.join(root, 'MapGyver');
				await mkdir(preferences, { recursive: true });
				execFileSync(
					observer,
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(preferences, 'preferences.json'),
						'--selected',
						gpu
					],
					{ windowsHide: true }
				);
			}
		: undefined
});
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
		media_kind: pictureFixture ? 'Picture' : 'SolidColor',
		label: 'Alignment red'
	});
	if (pictureFixture) {
		// A red internal landmark surrounded by black defeats the full-image aperture false
		// positive. Independent mode keeps image edges clear of the authored overlay stroke.
		const width = 800,
			height = 600;
		const raw = Buffer.alloc(height * (1 + width * 3));
		for (let y = redRect[1]; y < redRect[1] + redRect[3]; y++)
			for (let x = redRect[0]; x < redRect[0] + redRect[2]; x++)
				raw[y * (1 + width * 3) + 1 + x * 3] = 255;
		function chunk(name, data) {
			const type = Buffer.from(name);
			const length = Buffer.alloc(4);
			length.writeUInt32BE(data.length);
			const crc = Buffer.alloc(4);
			crc.writeUInt32BE(crc32(Buffer.concat([type, data])));
			return Buffer.concat([length, type, data, crc]);
		}
		const header = Buffer.alloc(13);
		header.writeUInt32BE(width);
		header.writeUInt32BE(height, 4);
		header[8] = 8;
		header[9] = 2;
		const picture = path.join(directory, 'internal-landmark.png');
		await writeFile(
			picture,
			Buffer.concat([
				Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
				chunk('IHDR', header),
				chunk('IDAT', deflateSync(raw)),
				chunk('IEND', Buffer.alloc(0))
			])
		);
		await edit('media', {
			kind: 'ImportPicture',
			target: source,
			locator: { kind: 'absolute', path: picture },
			label: null
		});
	} else
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
			...(sourceEditor ? { crop: { x: 0.2, y: 0.2, width: 0.6, height: 0.6 } } : {}),
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
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: snapshot.label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screen Warping(?::|$)/ })
		.click();
	if (sourceEditor)
		await page
			.getByRole('tab')
			.filter({ hasText: /^Surface Source(?::|$)/ })
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
	// Install only after startup navigation and native preview readiness. Observe actual native
	// arrow delivery without capturing text or changing input behavior.
	await page.evaluate(() => {
		window.__alignmentArrows = { events: [], overflow: false };
		for (const phase of ['keydown', 'keyup'])
			window.addEventListener(
				phase,
				(event) => {
					if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
					const journal = window.__alignmentArrows;
					if (journal.events.length >= 256) {
						journal.overflow = true;
						return;
					}
					journal.events.push({
						phase,
						key: event.key,
						trusted: event.isTrusted,
						repeat: event.repeat,
						shift: event.shiftKey,
						timestamp: event.timeStamp
					});
				},
				{ capture: true, passive: true }
			);
	});
	const bounds = await view.locator('.active-zone').boundingBox();
	assert(bounds);
	const metadata = {
		editor: sourceEditor ? 'source' : 'screen',
		landmark: pictureFixture
			? {
					extent: [800, 600],
					internalRedRect: redRect,
					crop: sourceEditor ? [0.2, 0.2, 0.6, 0.6] : [0, 0, 1, 1],
					independent: independentLandmark
				}
			: null,
		pid: native.app.pid,
		holdSeconds,
		directory,
		base: native.base,
		cdp: native.cdp,
		reference,
		adapter: hardware ? await observeNativeAdapter(page, hardware) : undefined,
		viewport: await page.evaluate(() => ({
			width: innerWidth,
			height: innerHeight,
			dpr: devicePixelRatio
		})),
		active: bounds
	};
	if (gpu) assert.equal(metadata.adapter.key, gpu);
	await writeFile(path.join(directory, 'ready.json'), JSON.stringify(metadata, null, 2));
	console.log(JSON.stringify({ ready: metadata }));
	if (process.argv.includes('--await-capture')) {
		// Let supported desktop control activate this owned window before visible capture.
		const deadline = Date.now() + holdSeconds * 1000;
		while (!(await exists('capture'))) {
			assert(Date.now() < deadline, 'alignment capture activation timeout');
			await delay(50);
		}
	}
	if (process.argv.includes('--hit')) {
		const expectedHit = sourceEditor ? 'crop:0' : `${reference.surface}:0`;
		const preview = view.locator('.native-preview');
		const identity = await preview.evaluate((element) => ({
			session: element.dataset.previewSession,
			view: element.dataset.previewView,
			source: element.dataset.previewSource
		}));
		assert(identity.session && identity.view && identity.source);
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
		await page.mouse.wheel(0, 1200);
		const samples = await page.evaluate(
			async ({ identity, expectedHit }) => {
				const samples = [];
				const query = async (point) =>
					JSON.parse(
						await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
							request: JSON.stringify({ kind: 'Pointer', ...identity, point })
						})
					);
				for (let index = 0; index < 16; index++) {
					const initial = (await query([0, 0])).sample;
					if (initial) {
						const point = [
							initial.rect.x + initial.rect.width * 0.2,
							initial.rect.y + initial.rect.height * 0.2
						];
						const hit = (await query(point)).sample;
						const rect = document
							.querySelector(`[data-native-hit-id="${CSS.escape(expectedHit)}"]`)
							.getBoundingClientRect();
						samples.push({
							point,
							hit,
							dom: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }
						});
					}
					await new Promise(requestAnimationFrame);
				}
				return samples;
			},
			{ identity, expectedHit }
		);
		const hits = samples.filter((sample) => sample.hit?.hit === expectedHit);
		assert(
			hits.length >= 3,
			'native accepted frame must resolve its corner throughout sampled motion'
		);
		assert(new Set(samples.map((sample) => Math.round(sample.hit?.rect.width ?? 0))).size >= 3);
		const separation = Math.max(
			...hits.map((sample) =>
				Math.hypot(sample.point[0] - sample.dom.x, sample.point[1] - sample.dom.y)
			)
		);
		const report = {
			editor: sourceEditor ? 'source' : 'screen',
			expectedHit,
			samples,
			correctNativeHits: hits.length,
			maxNativeDomSeparationCss: separation,
			scope:
				'actual native accepted-frame point/hit queries during animation; DOM event retargeting verified by offset browser fixture and real native editor suites'
		};
		await writeFile(path.join(directory, 'native-hit.json'), JSON.stringify(report, null, 2));
		console.log(
			JSON.stringify({
				nativeMovingHit: 'pass',
				editor: report.editor,
				correctNativeHits: hits.length,
				separationCss: separation
			})
		);
	} else if (process.argv.includes('--window-layout') || process.argv.includes('--editor-layout')) {
		const editorLayout = process.argv.includes('--editor-layout');
		if (process.argv.includes('--window-layout'))
			await verifyNativeWindowLayout({ native, directory, prefix: editorLayout ? 'window-' : '' });
		if (editorLayout) {
			const result = await promisify(execFile)(
				process.execPath,
				['apps/mapgyver/tests/browser/native-editor-layout.mjs', directory],
				{ windowsHide: true, timeout: 120000, maxBuffer: 1024 * 1024 }
			);
			console.log(result.stdout.trim());
		}
	} else if (process.argv.includes('--latency')) {
		await measureVisibleLatency({ native, view, directory });
	} else if (process.argv.includes('--zoom')) {
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
		const deadline = Date.now() + holdSeconds * 1000;
		while (!(await exists('finish'))) {
			assert(Date.now() < deadline, 'alignment fixture timeout');
			await delay(50);
		}
	}
	await writeFile(
		path.join(directory, 'native-arrows.json'),
		JSON.stringify(await page.evaluate(() => window.__alignmentArrows), null, 2)
	);
	// A mixed-DPI move changes DOM/DPR independently of the startup ready receipt.
	// Record the settled final projection after the caller's last capture, before normal close.
	await writeFile(
		path.join(directory, 'settled.json'),
		JSON.stringify(
			await view.evaluate((element) => {
				const active = element.querySelector('.active-zone').getBoundingClientRect();
				return {
					viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
					active: { x: active.x, y: active.y, width: active.width, height: active.height }
				};
			}),
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			result: 'fixture complete',
			project: (await edit('screens', { kind: 'InspectSurface', reference })).snapshot
		})
	);
} catch (error) {
	const state = await native.page
		.evaluate(() => ({
			pointers: window.__nativeHit?.pointers,
			previews: [...document.querySelectorAll('.native-preview')].map((element) => ({
				state: element.dataset.previewState,
				painted: element.dataset.nativeOverlays,
				hits: [...element.querySelectorAll('[data-native-hit-id]')].map((control) =>
					control.getAttribute('data-native-hit-id')
				)
			}))
		}))
		.catch(() => null);
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), state, log: native.log() }, null, 2)
	);
	console.error(JSON.stringify(state));
	throw error;
} finally {
	await native.close();
}
