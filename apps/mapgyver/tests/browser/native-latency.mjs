// Conservative CDP-input to observed composited-desktop response bounds. No scanout claim.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';

const run = promisify(execFile);
const clock = () => process.hrtime.bigint();
const milliseconds = (value) => Number(value) / 1e6;
const percentile = (values, rank) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * rank) - 1];

export function edges(frame, width, height) {
	const red = [],
		blue = [];
	for (let x = 0; x < width; x++) {
		const offset = (Math.floor(height / 2) * width + x) * 4;
		const [b, g, r] = frame.subarray(offset, offset + 3);
		if (r > 160 && g < 80 && b < 100) red.push(x);
		if (b > 180 && r > 70 && r < 190 && g > 110 && g < 215) blue.push(x);
	}
	return red.length && blue.length
		? { red: red[0], blue: blue.reduce((sum, x) => sum + x, 0) / blue.length }
		: null;
}

export function windowCaptureSource({ hwnd, left, top, width, height, viewport, dpr }) {
	return [
		`gfxcapture=hwnd=${hwnd}`,
		'capture_cursor=0',
		'max_framerate=120',
		`crop_left=${left}`,
		`crop_top=${top}`,
		`crop_right=${Math.round(viewport.width * dpr) - left - width}`,
		`crop_bottom=${Math.round(viewport.height * dpr) - top - height}`
	].join(':');
}

export async function measureVisibleLatency({
	native,
	view,
	directory,
	hwnd,
	captureKind = 'desktop'
}) {
	assert(['desktop', 'window'].includes(captureKind));
	const gpu = process.env.MAPGYVER_CAPTURE_GPU;
	assert(/^[01]$/.test(gpu ?? ''), 'Explicit capture adapter index 0 or 1 required');
	// Resolve identity during fixture preparation, before supported foreground activation.
	// Every geometry guard below still authenticates this exact HWND/PID and foreground.
	assert(Number.isSafeInteger(hwnd) && hwnd > 0);
	async function guard() {
		const { stdout } = await run(
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
		return JSON.parse(stdout);
	}
	const geometry = await guard();
	const box = await view.locator('.active-zone').boundingBox();
	const dpr = await native.page.evaluate(() => devicePixelRatio);
	assert(box && geometry.window_dpi === dpr * 96);
	const width = 128,
		height = 8,
		bytes = width * height * 4;
	const left = Math.floor(geometry.client_desktop_origin[0] + (box.x + box.width * 0.2) * dpr - 16);
	const top = Math.floor(
		geometry.client_desktop_origin[1] + (box.y + box.height / 2) * dpr - height / 2
	);
	const [monitorX, monitorY, monitorRight, monitorBottom] = geometry.monitor_bounds;
	assert(
		left >= monitorX &&
			top >= monitorY &&
			left + width <= monitorRight &&
			top + height <= monitorBottom
	);
	await native.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await native.page.evaluate(() => {
		window.__latencyWheel = [];
		window.addEventListener(
			'wheel',
			(event) => {
				if (window.__latencyWheel.length < 64)
					window.__latencyWheel.push({ trusted: event.isTrusted, deltaY: event.deltaY });
			},
			{ capture: true, passive: true }
		);
	});
	const cropLeft = left - geometry.client_desktop_origin[0];
	const cropTop = top - geometry.client_desktop_origin[1];
	const viewport = await native.page.evaluate(() => ({ width: innerWidth, height: innerHeight }));
	const source =
		captureKind === 'window'
			? windowCaptureSource({ hwnd, left: cropLeft, top: cropTop, width, height, viewport, dpr })
			: [
					'ddagrab=output_idx=0',
					'framerate=120',
					`video_size=${width}x${height}`,
					`offset_x=${left - monitorX}`,
					`offset_y=${top - monitorY}`,
					'draw_mouse=0',
					'dup_frames=1'
				].join(':');
	const args = [
		'-hide_banner',
		'-v',
		captureKind === 'window' ? 'info' : 'warning',
		'-init_hw_device',
		`d3d11va=desktop:${gpu}`,
		'-filter_hw_device',
		'desktop',
		// A CLI filter graph receives filter_hw_device; the lavfi input owns a separate graph.
		'-filter_complex',
		`${source},hwdownload,format=bgra${captureKind === 'window' ? ',showinfo' : ',setpts=N/(120*TB)'}`,
		'-fps_mode',
		'passthrough',
		'-f',
		'rawvideo',
		'-pix_fmt',
		'bgra',
		'-flush_packets',
		'1',
		'-blocksize',
		String(bytes),
		'pipe:1'
	];
	const capture = spawn('ffmpeg', args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
	let errors = '',
		failure = null,
		streamError,
		buffer = Buffer.alloc(0),
		frames = 0,
		latest,
		trial,
		previousAt;
	const recent = [],
		gaps = [],
		observations = [],
		witnesses = [],
		captureTimes = [];
	let logLine = '';
	const closed = new Promise((resolve) =>
		capture.once('close', (code, signal) => resolve({ code, signal }))
	);
	capture.once('error', (error) => {
		streamError = error;
	});
	capture.stdin.on('error', () => {});
	capture.stderr.on('data', (data) => {
		errors = (errors + data).slice(-65536);
		if (captureKind === 'window') {
			logLine += data;
			const lines = logLine.split('\n');
			logLine = lines.pop();
			for (const line of lines) {
				const match = line.match(/\bn:\s*(\d+)\s+pts:\s*(-?\d+)/);
				if (match && captureTimes.length < 12000)
					captureTimes.push({ frame: Number(match[1]) + 1, ticks_100ns: match[2] });
			}
		}
	});
	capture.stdout.on('data', (chunk) => {
		buffer = Buffer.concat([buffer, chunk]);
		while (buffer.length >= bytes) {
			const raw = Buffer.from(buffer.subarray(0, bytes));
			buffer = buffer.subarray(bytes);
			const at = clock();
			if (previousAt) gaps.push(milliseconds(at - previousAt));
			previousAt = at;
			frames++;
			const position = edges(raw, width, height);
			latest = { frame: frames, at, position, raw };
			recent.push(latest);
			if (recent.length > 64) recent.shift();
			if (
				trial &&
				!trial.response &&
				position &&
				at >= trial.begin &&
				(position.red - trial.baseline.position.red) * trial.direction >= 2 &&
				(position.blue - trial.baseline.position.blue) * trial.direction >= 2
			) {
				trial.response = latest;
			}
		}
	});
	async function waitFor(predicate, timeout, label) {
		const deadline = Date.now() + timeout;
		while (!predicate()) {
			assert(
				!streamError && capture.exitCode === null,
				`${label}: capture stopped: ${streamError ?? errors}`
			);
			assert(Date.now() < deadline, `${label}: ${errors}`);
			await delay(2);
		}
	}
	try {
		await waitFor(
			() => recent.length >= 10 && latest.position,
			10000,
			'Warm capture needs actual red/blue edge pixels'
		);
		for (let index = 0; index < 30; index++) {
			assert.deepEqual(
				await guard(),
				geometry,
				'Owned window foreground/bounds/DPI must stay fixed'
			);
			await delay(500);
			await waitFor(
				() => latest.position && milliseconds(clock() - latest.at) <= 25,
				1500,
				'Fresh edge observation required'
			);
			const baseline = latest;
			assert(
				baseline.position && milliseconds(clock() - baseline.at) <= 25,
				'Fresh edge observation required'
			);
			assert(
				recent
					.slice(-10)
					.every(
						(item) =>
							item.position &&
							Math.abs(item.position.red - baseline.position.red) <= 1 &&
							Math.abs(item.position.blue - baseline.position.blue) <= 1
					),
				'Previous motion must settle before another input'
			);
			const direction = index % 2 === 0 ? 1 : -1;
			trial = { begin: clock(), baseline, direction };
			await native.page.mouse.wheel(0, direction * 120);
			await waitFor(() => trial.response, 1500, 'Both image and overlay must visibly respond');
			const { begin, response } = trial;
			trial = null;
			observations.push({
				index,
				deltaY: direction * 120,
				dispatch_ns: begin.toString(),
				response_arrival_ns: response.at.toString(),
				upper_bound_ms: milliseconds(response.at - begin),
				before: baseline.position,
				after: response.position,
				before_frame: baseline.frame,
				response_frame: response.frame
			});
			witnesses.push(baseline.raw, response.raw);
			assert.deepEqual(
				await guard(),
				geometry,
				'Owned window must retain foreground after response'
			);
		}
		const events = await native.page.evaluate(() => window.__latencyWheel);
		assert(
			events.length === 30 &&
				events.every(
					(event, index) => event.trusted && event.deltaY === observations[index].deltaY
				),
			'Every measured action needs its trusted wheel event'
		);
		const values = observations.map((item) => item.upper_bound_ms);
		const result = {
			pid: native.app.pid,
			hwnd,
			geometry,
			dpr,
			region: { left, top, width, height },
			capture_adapter_index: Number(gpu),
			capture_kind: captureKind,
			command: ['ffmpeg', ...args],
			frames,
			events,
			observations,
			p95_upper_bound_ms: percentile(values, 0.95),
			p99_upper_bound_ms: percentile(values, 0.99),
			maximum_upper_bound_ms: Math.max(...values),
			capture_gap_p95_ms: percentile(gaps, 0.95),
			capture_gap_maximum_ms: Math.max(...gaps),
			within_50_ms: percentile(values, 0.95) <= 50,
			scope: `Same Node monotonic clock before documented CDP dispatch and after complete ${captureKind === 'window' ? 'Windows Graphics Capture' : 'Desktop Duplication'} frame arrival. Both actual native-image and overlay edges move >=2 pixels in the expected direction. Conservative composited-window response upper bound; no physical scanout or pointer/keyboard latency acceptance.`
		};
		await writeFile(path.join(directory, 'latency.json'), JSON.stringify(result, null, 2));
		assert(
			result.within_50_ms,
			`Native zoom response p95 upper bound ${result.p95_upper_bound_ms}ms exceeds 50ms`
		);
	} catch (error) {
		failure = String(error);
		throw error;
	} finally {
		capture.stdin.end('q');
		const exited = await Promise.race([closed, delay(3000).then(() => null)]);
		if (!exited) {
			capture.kill();
			await closed;
		}
		const captureExit = await closed;
		await writeFile(
			path.join(directory, 'capture-state.json'),
			JSON.stringify(
				{
					geometry,
					region: { left, top, width, height },
					frames,
					command: ['ffmpeg', ...args],
					last_edges: latest?.position,
					observations,
					cohort_observations_complete: observations.length === 30,
					capture_kind: captureKind,
					capture_times: captureTimes,
					capture_exit: captureExit,
					failure,
					clock_scope:
						captureKind === 'window'
							? 'Capture PTS preserves WGC SystemRelativeTime differences, rebased by FFmpeg; response upper bounds use Node hrtime only, without offset fitting or subtraction'
							: 'Muxer PTS is frame-index metadata; response bounds use Node hrtime only'
				},
				null,
				2
			)
		);
		// Keep failed/partial cohorts replayable rather than losing their pixel witnesses.
		await writeFile(path.join(directory, 'latency-witnesses.bgra'), Buffer.concat(witnesses));
		if (latest) await writeFile(path.join(directory, 'capture-last.bgra'), latest.raw);
		await writeFile(path.join(directory, 'capture.log'), errors);
		if (failure === null)
			assert.deepEqual(captureExit, { code: 0, signal: null }, 'Capture must close normally');
	}
}
