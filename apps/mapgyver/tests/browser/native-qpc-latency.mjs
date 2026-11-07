// Conservative input-to-pixel-copy bounds on one native QPC clock; no scanout inference.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { edges } from './native-latency.mjs';
const run = promisify(execFile);

const percentile = (values, rank) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * rank) - 1];

export async function measureNativeQpcLatency({ native, view, directory, hwnd }) {
	const adapter = process.env.MAPGYVER_CAPTURE_GPU;
	assert(/^[01]$/.test(adapter ?? ''), 'Explicit desktop capture adapter required');
	const observed = await run(
		'python',
		[
			'apps/mapgyver/tests/native/window_capture.py',
			'--geometry-only',
			'--hwnd',
			String(hwnd),
			'--pid',
			String(native.app.pid)
		],
		{ windowsHide: true, encoding: 'utf8', timeout: 5000 }
	);
	const geometry = JSON.parse(observed.stdout);
	const box = await view.locator('.active-zone').boundingBox();
	const dpr = await native.page.evaluate(() => devicePixelRatio);
	assert(box && geometry.window_dpi === dpr * 96);
	const width = 128,
		height = 8;
	const left = Math.floor(geometry.client_desktop_origin[0] + (box.x + box.width * 0.2) * dpr - 16);
	const top = Math.floor(
		geometry.client_desktop_origin[1] + (box.y + box.height / 2) * dpr - height / 2
	);
	await native.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await native.page.evaluate(() => {
		window.__qpcWheel = [];
		window.addEventListener(
			'wheel',
			(event) => {
				if (window.__qpcWheel.length < 64)
					window.__qpcWheel.push({ trusted: event.isTrusted, deltaY: event.deltaY });
			},
			{ capture: true, passive: true }
		);
	});
	const executable = path.resolve('target/debug/deps/native_qualification-desktop-qpc.exe');
	const args = [
		'--role',
		'desktop-capture',
		'--adapter',
		adapter,
		'--output',
		'0',
		'--hwnd',
		String(hwnd),
		'--pid',
		String(native.app.pid),
		'--left',
		String(left),
		'--top',
		String(top),
		'--width',
		String(width),
		'--height',
		String(height),
		'--seconds',
		'120'
	];
	const capture = spawn(executable, args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
	let failure,
		exit,
		stopped = false,
		ready,
		latest,
		trial,
		id = 0;
	let errors = '';
	const stamps = new Map();
	const recent = [],
		observations = [],
		witnesses = [];
	const closed = new Promise((resolve) =>
		capture.once('close', (code, signal) => {
			exit = { code, signal };
			resolve(exit);
		})
	);
	capture.on('error', (error) => {
		failure = error;
	});
	capture.stdin.on('error', () => {});
	capture.stderr.on('data', (data) => {
		errors = (errors + data).slice(-16384);
	});
	const lines = createInterface({ input: capture.stdout });
	lines.on('line', (line) => {
		try {
			assert(line.length <= 65536, 'Bounded capture packet required');
			const event = JSON.parse(line);
			if (event.kind === 'Ready') {
				assert(!ready);
				ready = event;
			} else if (event.kind === 'Stamped') stamps.set(event.id, event.qpc);
			else if (event.kind === 'Stopped') stopped = true;
			else if (event.kind === 'Frame') {
				assert.equal(typeof event.protected_content_masked_out, 'boolean');
				assert.equal(event.bgra.length, width * height * 4);
				assert(BigInt(event.acquire_begin_qpc) <= BigInt(event.readback_end_qpc));
				assert(BigInt(event.desktop_update_qpc) <= BigInt(event.readback_end_qpc));
				if (latest) assert(event.serial > latest.serial);
				event.raw = Buffer.from(event.bgra);
				event.position = edges(event.raw, width, height);
				delete event.bgra;
				latest = event;
				recent.push(event);
				if (recent.length > 10) recent.shift();
				if (
					trial &&
					!trial.response &&
					event.position &&
					BigInt(event.desktop_update_qpc) > BigInt(trial.begin) &&
					['red', 'blue'].every(
						(key) => (event.position[key] - trial.baseline.position[key]) * trial.direction >= 2
					)
				) {
					trial.response = event;
				}
			} else assert.fail(`Unknown native capture event ${event.kind}`);
		} catch (error) {
			failure = error;
		}
	});
	async function waitFor(test, label, timeout = 10000) {
		const deadline = Date.now() + timeout;
		while (!test()) {
			assert(!failure && !exit, `${label}: ${failure ?? errors ?? JSON.stringify(exit)}`);
			assert(Date.now() < deadline, label);
			await delay(2);
		}
	}
	async function stamp() {
		const key = ++id;
		capture.stdin.write(JSON.stringify({ kind: 'Stamp', id: key }) + '\n');
		await waitFor(() => stamps.has(key), 'Native stamp acknowledgement required', 2000);
		const value = stamps.get(key);
		stamps.delete(key);
		return value;
	}
	let result;
	try {
		await waitFor(
			() => ready && recent.length === 10 && latest.position,
			'Owned native image/overlay warmup required'
		);
		const frequency = BigInt(ready.frequency);
		assert(frequency > 0n);
		assert.equal(ready.geometry.dpi, geometry.window_dpi);
		assert.deepEqual(ready.geometry.window, geometry.desktop_bounds);
		assert.deepEqual(ready.region, [left, top, left + width, top + height]);
		for (let index = 0; index < 30; index++) {
			await delay(500);
			const baseline = latest;
			const begin = await stamp(); // Native acknowledgement received before CDP dispatch.
			assert(baseline.position && BigInt(baseline.readback_end_qpc) <= BigInt(begin));
			assert(
				(Number(BigInt(begin) - BigInt(baseline.readback_end_qpc)) * 1000) / Number(frequency) <=
					25,
				'Fresh native baseline required'
			);
			assert(
				recent.every(
					(sample) =>
						sample.position &&
						['red', 'blue'].every(
							(key) => Math.abs(sample.position[key] - baseline.position[key]) <= 1
						)
				),
				'Previous image motion must settle'
			);
			const direction = index % 2 === 0 ? 1 : -1;
			trial = { begin, baseline, direction };
			await native.page.mouse.wheel(0, direction * 120);
			await waitFor(() => trial.response, 'Both native image and overlay must respond', 1500);
			const response = trial.response;
			trial = null;
			const after = await stamp(); // Revalidates ownership/geometry after the captured response.
			assert(BigInt(after) >= BigInt(response.readback_end_qpc));
			observations.push({
				index,
				deltaY: direction * 120,
				begin_qpc: begin,
				before_qpc: baseline.readback_end_qpc,
				acquire_begin_qpc: response.acquire_begin_qpc,
				response_qpc: response.readback_end_qpc,
				after_qpc: after,
				upper_bound_ms:
					(Number(BigInt(response.readback_end_qpc) - BigInt(begin)) * 1000) / Number(frequency),
				before: baseline.position,
				response: response.position,
				before_serial: baseline.serial,
				response_serial: response.serial,
				desktop_update_qpc: response.desktop_update_qpc,
				accumulated_frames: response.accumulated_frames,
				before_protected_content_masked_out: baseline.protected_content_masked_out,
				response_protected_content_masked_out: response.protected_content_masked_out
			});
			witnesses.push(baseline.raw, response.raw);
		}
		const events = await native.page.evaluate(() => window.__qpcWheel);
		assert(
			events.length === 30 &&
				events.every((event, index) => event.trusted && event.deltaY === observations[index].deltaY)
		);
		const values = observations.map((value) => value.upper_bound_ms);
		result = {
			pid: native.app.pid,
			hwnd,
			geometry,
			region: { left, top, width, height },
			frequency: ready.frequency,
			observations,
			events,
			p95_upper_bound_ms: percentile(values, 0.95),
			p99_upper_bound_ms: percentile(values, 0.99),
			maximum_upper_bound_ms: Math.max(...values),
			within_50_ms: percentile(values, 0.95) <= 50,
			scope:
				'Native pre-input acknowledgement to owned-pixel-copy completion on one QPC clock; desktop-update metadata is freshness only. Desktop-wide masked-content flags are preserved; actual red-image and blue-overlay witnesses remain mandatory. Conservative composited-desktop bound, no scanout inference.'
		};
		await writeFile(path.join(directory, 'qpc-latency.json'), JSON.stringify(result, null, 2));
		await writeFile(path.join(directory, 'qpc-witnesses.bgra'), Buffer.concat(witnesses));
		assert(
			result.within_50_ms,
			`Native QPC visible-response bound ${result.p95_upper_bound_ms}ms exceeds50ms`
		);
	} finally {
		trial = null;
		capture.stdin.end(JSON.stringify({ kind: 'Stop' }) + '\n');
		const done = await Promise.race([closed, delay(3000).then(() => null)]);
		if (!done) {
			capture.kill();
			await closed;
		}
		lines.close();
		await writeFile(path.join(directory, 'qpc-witnesses.bgra'), Buffer.concat(witnesses));
		await writeFile(
			path.join(directory, 'qpc-capture-state.json'),
			JSON.stringify(
				{
					ready,
					exit,
					errors,
					failure: failure ? String(failure) : null,
					stopped,
					observations,
					complete: !!result,
					command: [executable, ...args]
				},
				null,
				2
			)
		);
		if (result) {
			assert(!failure, String(failure));
			assert(stopped, 'Native stop acknowledgement required');
			assert.equal(exit?.code, 0, errors);
		}
	}
}
