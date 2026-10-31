// Run from the workspace root after building MapGyver and native_qualification with
// --features native-qualification. Uses the ordinary app, public authoring HTTP API and an
// existing independent Spout receiver. No editor is loaded or registered in this flow.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { createServer } from 'node:net';
import { createWriteStream } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const fullscreen = process.argv.includes('--fullscreen');
const catalogArg = process.argv.indexOf('--catalog');
assert(
	!fullscreen || (catalogArg >= 0 && process.argv[catalogArg + 1]),
	'fullscreen requires --catalog <fixture.json>'
);
const displayCatalog = fullscreen
	? JSON.parse(await readFile(process.argv[catalogArg + 1], 'utf8'))
	: [];
assert(
	!fullscreen || (catalogArg >= 0 && displayCatalog.length > 0),
	'fullscreen requires --catalog with native fixture metadata'
);
const selectedDisplay = displayCatalog[0];

// Never attach this authoring fixture to an existing user session.
const portCheck = createServer();
await new Promise((resolve, reject) => {
	portCheck.once('error', reject);
	portCheck.listen(7023, '127.0.0.1', resolve);
});
await new Promise((resolve) => portCheck.close(resolve));
const directory = path.resolve('target', `m05-spout-${Date.now()}`);
await mkdir(directory, { recursive: true });
const stop = path.join(directory, 'app.stop');
const base = 'http://127.0.0.1:7023';
const sender = `MapGyver-M05-${randomUUID()}`;
const rectangle = process.argv.includes('--rectangle');
let rectangleScreen;
let fullscreenScreen;
const executable = path.resolve('target/debug/MapGyver.exe');
const receiver = path.resolve('target/debug/native_qualification.exe');
const appLog = createWriteStream(path.join(directory, 'app.log'));
const app = spawn(executable, ['--no-frontend', '--automation-shutdown-file', stop], {
	env: { ...process.env, GC_UI_BIND: '127.0.0.1:7023', GC_UI_FRONTEND_URL: base },
	windowsHide: true,
	stdio: ['ignore', 'pipe', 'pipe']
});
app.stdout.pipe(appLog, { end: false });
app.stderr.pipe(appLog, { end: false });
const exited = new Promise((resolve, reject) => {
	app.once('error', reject);
	app.once('close', (code, signal) => resolve({ code, signal }));
});
let token;
async function post(route, payload) {
	const response = await fetch(`${base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(payload)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function edit(namespace, payload) {
	const acknowledgement = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(acknowledgement.success, JSON.stringify(acknowledgement));
	return acknowledgement.result.value;
}
const screens = (payload) => edit('mapgyver.screens', payload);
async function waitForAppLog(text, after = 0) {
	const deadline = Date.now() + 15000;
	while (true) {
		const log = await readFile(path.join(directory, 'app.log'), 'utf8');
		if (log.slice(after).includes(text)) return;
		assert(Date.now() < deadline, `app did not report ${text}`);
		await delay(50);
	}
}
let ordinal = 0;
async function receive(width, label, expected) {
	const output = path.join(directory, `${++ordinal}-${label}`);
	await mkdir(output);
	const child = spawn(
		receiver,
		[
			'--role',
			'consumer',
			'--name',
			sender,
			'--width',
			`${width}`,
			'--height',
			'64',
			'--instance',
			randomUUID(),
			'--directory',
			output,
			'--max-seconds',
			'3',
			'--max-frames',
			'256',
			'--trace-capacity',
			'4096'
		],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	let errors = '';
	child.stderr.on('data', (chunk) => {
		errors += chunk;
	});
	child.stdout.resume();
	const code = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	assert.equal(code, 0, errors);
	const rows = (await readFile(path.join(output, 'consumer.jsonl'), 'utf8'))
		.trim()
		.split('\n')
		.map(JSON.parse);
	const final = rows.find((row) => row.kind === 'consumer_final').consumer;
	assert.equal(final.failed_receives, 0, label);
	if (expected) {
		assert(final.receive_copies >= 10, `${label}: only ${final.receive_copies} copies`);
		assert.equal(final.descriptor.extent.width, width);
		assert.equal(final.mismatch, 0, label);
	} else {
		assert.equal(final.receive_copies, 0, label);
		assert(final.absent > 0, `${label}: sender absence was not observed`);
	}
	console.log(
		JSON.stringify({ label, receive_copies: final.receive_copies, absent: final.absent })
	);
	// Marker errors are expected: calibration/solid colors are not qualification marker cards.
	// This flow proves actual independent SDK delivery, never pixel/color or cadence acceptance.
}

try {
	const deadline = Date.now() + 15000;
	while (true) {
		try {
			if ((await fetch(`${base}/api/ui/health`, { signal: AbortSignal.timeout(1000) })).ok) break;
		} catch {
			/* Startup is bounded below; no process is restarted. */
		}
		assert(Date.now() < deadline, 'app did not start');
		await delay(100);
	}
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit('mapgyver.media', {
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Output source'
	});
	const { screen, surface } = await screens({
		kind: 'CreateScreen',
		label: 'Virtual output check'
	});
	const definition = {
		source: reference,
		crop: { x: 0, y: 0, width: 1, height: 1 },
		quad: {
			corners: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 1, y: 1 },
				{ x: 0, y: 1 }
			]
		},
		opacity: 1,
		enabled: true,
		calibration: 'ColorBars'
	};
	const target = (enabled, width = 96) =>
		screens({
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled,
				target: { kind: 'Spout', sender, extent: { width, height: 64 } }
			}
		});
	const fullscreenTarget = (enabled, display = selectedDisplay.id) =>
		screens({
			kind: 'ConfigureScreen',
			screen: fullscreenScreen,
			definition: { enabled, target: { kind: 'Fullscreen', display } }
		});
	await screens({ kind: 'ConfigureSurface', reference: surface, definition });
	await target(true);
	await waitForAppLog('state=Ready');
	if (rectangle) {
		const { screen: createdRectangle, surface: rectangleSurface } = await screens({
			kind: 'CreateScreen',
			label: 'Rectangle output check'
		});
		rectangleScreen = createdRectangle;
		await screens({
			kind: 'ConfigureSurface',
			reference: rectangleSurface,
			definition: { ...definition, source: null, calibration: 'ColorBars' }
		});
		await screens({
			kind: 'ConfigureScreen',
			screen: rectangleScreen,
			definition: {
				enabled: true,
				target: {
					kind: 'Rectangle',
					bounds: {
						left: 80,
						top: 120,
						extent: { width: 640, height: 360 }
					}
				}
			}
		});
		await waitForAppLog(
			'physical=NativeOutputBounds { left: 80, top: 120, width: 640, height: 360 }'
		);
		await waitForAppLog('screens=2 outputs=2');
	}
	if (fullscreen) {
		assert(rectangle, 'fullscreen flow includes rectangle concurrency');
		const created = await screens({ kind: 'CreateScreen', label: 'Fullscreen output check' });
		fullscreenScreen = created.screen;
		await screens({
			kind: 'ConfigureSurface',
			reference: created.surface,
			definition: { ...definition, source: null, calibration: 'ColorBars' }
		});
		await fullscreenTarget(true);
		const { left, top, extent } = selectedDisplay.bounds;
		await waitForAppLog(
			`window id=4 physical=NativeOutputBounds { left: ${left}, top: ${top}, width: ${extent.width}, height: ${extent.height} }`
		);
		await waitForAppLog('screens=3 outputs=3');
		await waitForAppLog('id=4 first API acceptance');
		await waitForAppLog('id=3 first API acceptance');
		console.log(
			JSON.stringify({ fullscreen_active: selectedDisplay.name, bounds: selectedDisplay.bounds })
		);
	}
	await receive(96, 'bars-without-editor', true);
	if (fullscreen) {
		const beforeMissing = (await readFile(path.join(directory, 'app.log'), 'utf8')).length;
		await fullscreenTarget(true, 'missing-display-fixture');
		await waitForAppLog('screens=2 outputs=2', beforeMissing);
		console.log(JSON.stringify({ missing_display_withdrawn: true }));
	}
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...definition, calibration: 'Grid' }
	});
	await receive(96, 'grid-without-editor', true);
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: { ...definition, calibration: 'Off' }
	});
	await receive(96, 'restored-source-without-editor', true);
	const beforeDisable = (await readFile(path.join(directory, 'app.log'), 'utf8')).length;
	await target(false);
	await waitForAppLog('[mapgyver-output] native session retired', beforeDisable);
	await receive(96, 'disabled', false);
	await target(true, 128);
	if (fullscreen) {
		const beforeRestore = (await readFile(path.join(directory, 'app.log'), 'utf8')).length;
		await fullscreenTarget(true);
		await waitForAppLog('id=4 first API acceptance', beforeRestore);
		const { snapshot } = await screens({ kind: 'InspectManager' });
		assert.equal(
			snapshot.screens.find((entry) => entry.id === fullscreenScreen).definition.target.display,
			selectedDisplay.id
		);
	}
	await receive(128, 'reenabled-and-resized', true);
	if (fullscreen) await fullscreenTarget(false);
	await target(false, 128);
	if (rectangle) {
		await screens({
			kind: 'ConfigureScreen',
			screen: rectangleScreen,
			definition: {
				enabled: true,
				target: {
					kind: 'Rectangle',
					bounds: {
						left: 120,
						top: 180,
						extent: { width: 480, height: 270 }
					}
				}
			}
		});
		await waitForAppLog(
			'physical=NativeOutputBounds { left: 120, top: 180, width: 480, height: 270 }'
		);
		console.log(JSON.stringify({ inspection_ready: true, directory }));
		await delay(fullscreen ? 1000 : 60000); // Fullscreen flow closes promptly; rectangle mode allows finite inspection.
	}
	console.log(JSON.stringify({ result: 'pass', directory }));
} finally {
	await writeFile(stop, 'stop\n');
	const result = await Promise.race([
		exited,
		delay(15000, undefined, { ref: false }).then(() => {
			throw new Error(`app retirement did not finish; PID ${app.pid}`);
		})
	]);
	await new Promise((resolve) => appLog.end(resolve));
	assert.equal(result.code, 0, JSON.stringify(result));
}
const log = await readFile(path.join(directory, 'app.log'), 'utf8');
assert(
	log.includes('channels=0 screens=1 outputs=1') || log.includes('channels=0 screens=2 outputs=2'),
	'calibration demanded a media channel'
);
assert(
	log.includes(rectangle ? 'channels=1 screens=2 outputs=2' : 'channels=1 screens=1 outputs=1'),
	'Off did not restore source demand'
);
assert(
	!log.includes('window teardown failed') && !log.includes('close failed'),
	'output window teardown failed'
);
