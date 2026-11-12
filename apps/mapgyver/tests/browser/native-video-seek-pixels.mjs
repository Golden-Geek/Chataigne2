// Physical Media Preview witness using the existing guarded desktop/QPC collector.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';

class Capture {
	constructor(native, region, hwnd) {
		this.frames = [];
		this.stamps = new Map();
		this.id = 0;
		const args = [
			'--role',
			'desktop-capture',
			'--adapter',
			process.env.MAPGYVER_CAPTURE_GPU ?? '0',
			'--output',
			'0',
			'--hwnd',
			String(hwnd),
			'--pid',
			String(native.app.pid),
			'--left',
			String(region[0]),
			'--top',
			String(region[1]),
			'--width',
			'256',
			'--height',
			'8',
			'--seconds',
			'120'
		];
		this.child = spawn(path.resolve('target/debug/native_qualification.exe'), args, {
			windowsHide: true,
			stdio: ['pipe', 'pipe', 'pipe']
		});
		this.closed = new Promise((resolve) =>
			this.child.once('close', (code, signal) => {
				this.exit = { code, signal };
				resolve();
			})
		);
		this.child.on('error', (error) => {
			this.failure = String(error);
		});
		this.child.stdin.on('error', () => {});
		this.stderr = '';
		this.child.stderr.on('data', (data) => {
			this.stderr = (this.stderr + data).slice(-8192);
		});
		createInterface({ input: this.child.stdout }).on('line', (line) => {
			try {
				assert(line.length < 65536, 'bounded physical capture row');
				const event = JSON.parse(line);
				if (event.kind === 'Ready') this.ready = event;
				else if (event.kind === 'Stamped') this.stamps.set(event.id, event.qpc);
				else if (event.kind === 'Stopped') this.stopped = true;
				else if (event.kind === 'Frame') {
					assert.equal(event.bgra.length, 256 * 8 * 4);
					// This flag is desktop-wide; other apps can be masked independently of our ROI.
					assert.equal(typeof event.protected_content_masked_out, 'boolean');
					assert(BigInt(event.desktop_update_qpc) <= BigInt(event.readback_end_qpc));
					const bytes = Buffer.from(event.bgra);
					event.non_black_pixels = 0;
					for (let i = 0; i < bytes.length; i += 4) {
						if (bytes[i] || bytes[i + 1] || bytes[i + 2]) event.non_black_pixels++;
					}
					this.latestRaw = bytes;
					event.hash = createHash('sha256').update(bytes).digest('hex');
					delete event.bgra;
					if (this.latest?.hash === event.hash) event.stableSince = this.latest.stableSince;
					else event.stableSince = performance.now();
					this.latest = event;
					this.frames.push(event);
					assert(this.frames.length <= 30000, 'bounded witness lifetime');
				} else assert.fail(`Unknown capture event: ${event.kind}`);
			} catch (error) {
				this.failure = String(error);
			}
		});
	}
	async wait(test, label, seconds = 5) {
		const end = performance.now() + seconds * 1000;
		while (!test()) {
			assert(!this.failure && !this.exit, `${label}: ${this.failure ?? this.stderr}`);
			assert(performance.now() < end, label);
			await delay(2);
		}
	}
	async stamp() {
		const id = ++this.id;
		assert(id <= 128);
		this.child.stdin.write(JSON.stringify({ kind: 'Stamp', id }) + '\n');
		await this.wait(() => this.stamps.has(id), 'native pre-intent stamp', 2);
		const qpc = this.stamps.get(id);
		this.stamps.delete(id);
		return qpc;
	}
	async close(directory, label) {
		if (!this.exit) this.child.stdin.write('{"kind":"Stop"}\n');
		await Promise.race([this.closed, delay(5000)]);
		if (!this.exit) this.child.kill();
		await this.closed;
		await writeFile(
			path.join(directory, `${label}-capture.json`),
			JSON.stringify(
				{
					ready: this.ready,
					frames: this.frames,
					failure: this.failure,
					exit: this.exit,
					stopped: this.stopped,
					stderr: this.stderr
				},
				null,
				2
			)
		);
		assert(
			!this.failure && this.exit.code === 0 && this.stopped,
			`capture failed: ${this.failure ?? this.stderr}`
		);
	}
}

const percentile = (values, rank) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * rank) - 1];
const framePosition = (index) => (index * 1000) / 24;
// Seek inside a frame, avoiding floating-point ambiguity at its exact boundary.
const intentPosition = (index) => ((index + 0.25) * 1000) / 24;

export async function measureSeekPixels({ native, directory, state, seek, mode, viewport }) {
	const main = JSON.parse(
		execFileSync(
			'python',
			['apps/mapgyver/tests/native_output/window_state.py', String(native.app.pid), '--main'],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
	assert.equal(main.length, 1);
	const geometry = JSON.parse(
		execFileSync(
			'python',
			[
				'apps/mapgyver/tests/native/window_capture.py',
				'--geometry-only',
				'--hwnd',
				String(main[0].hwnd),
				'--pid',
				String(native.app.pid)
			],
			{ encoding: 'utf8', windowsHide: true }
		)
	);
	const dpr = await native.page.evaluate(() => devicePixelRatio);
	assert.equal(geometry.window_dpi, dpr * 96);
	const box = await viewport.boundingBox();
	assert(box && box.width * dpr > 280);
	const region = [
		Math.floor(geometry.client_desktop_origin[0] + (box.x + box.width / 2) * dpr - 128),
		Math.floor(geometry.client_desktop_origin[1] + (box.y + box.height / 2) * dpr - 4)
	];
	const references = [],
		phases = [];
	await mode(false);
	for (let start = 0; start < 199; start += 100) {
		const capture = new Capture(native, region, main[0].hwnd);
		try {
			await capture.wait(() => capture.ready && capture.latest, 'calibration capture ready');
			for (let index = start; index < Math.min(199, start + 100); index++) {
				const previous = capture.latest.hash;
				const begin = await capture.stamp();
				await seek(intentPosition(index));
				const deadline = performance.now() + 5000;
				while (true) {
					const value = await state();
					assert(!value.source.error && !value.transport.playing, JSON.stringify(value));
					if (Math.abs(value.frame?.position_ms - framePosition(index)) < 0.002) break;
					assert(performance.now() < deadline, `calibration frame${index}`);
					await delay(5);
				}
				await capture.wait(
					() =>
						capture.latest &&
						BigInt(capture.latest.desktop_update_qpc) > BigInt(begin) &&
						capture.latest.hash !== previous &&
						performance.now() - capture.latest.stableSince >= 80,
					`stable physical frame${index}`
				);
				references.push({
					index,
					hash: capture.latest.hash,
					qpc: capture.latest.desktop_update_qpc
				});
				assert(capture.latest.non_black_pixels > 0, 'owned source region must not be blacked out');
				if (index === 0)
					await writeFile(path.join(directory, 'reference-strip.bgra'), capture.latestRaw);
			}
		} finally {
			await capture.close(directory, `reference-${start}`);
		}
	}
	await writeFile(
		path.join(directory, 'physical-references.json'),
		JSON.stringify({ geometry, box, region, references }, null, 2)
	);
	assert.equal(
		new Set(references.map((ref) => ref.hash)).size,
		199,
		'unique physical reference images required'
	);
	const byHash = new Map(references.map((ref) => [ref.hash, ref.index]));
	for (const playing of [false, true]) {
		await mode(false);
		await seek(intentPosition(0));
		await delay(150);
		const capture = new Capture(native, region, main[0].hwnd);
		const requests = [];
		try {
			await capture.wait(
				() => capture.ready && capture.latest?.hash === references[0].hash,
				'physical phase baseline'
			);
			await mode(playing);
			const start = performance.now();
			for (let index = 0; index < 120; index++) {
				await delay(Math.max(0, start + index * 25 - performance.now()));
				const frame = ((index + 1) * 37) % 180;
				const qpc = await capture.stamp();
				requests.push({ index, frame, qpc });
				await seek(intentPosition(frame));
			}
			await delay(200);
			const value = await state();
			assert(!value.source.error);
			assert.equal(value.transport.settings.mode, playing ? 'Playing' : 'Paused');
			const frequency = Number(capture.ready.frequency),
				matched = [],
				unknown = [],
				used = new Set();
			let prior,
				lastMatched = -1;
			for (const image of capture.frames) {
				if (image.hash === prior) continue;
				prior = image.hash;
				if (BigInt(image.desktop_update_qpc) <= BigInt(requests[0].qpc)) continue;
				const frame = byHash.get(image.hash);
				if (frame === undefined) {
					unknown.push(image.serial);
					continue;
				}
				const candidates = requests.filter(
					(request) =>
						request.index > lastMatched &&
						!used.has(request.index) &&
						BigInt(request.qpc) < BigInt(image.desktop_update_qpc) &&
						frame >= request.frame &&
						frame <= request.frame + (playing ? 2 : 0)
				);
				assert(candidates.length <= 1, 'ambiguous seek/frame identity');
				if (!candidates.length) continue;
				const request = candidates[0];
				used.add(request.index);
				lastMatched = request.index;
				matched.push({
					request: request.index,
					requested_frame: request.frame,
					frame,
					qpc: image.desktop_update_qpc,
					serial: image.serial,
					copy_bound_ms:
						(Number(BigInt(image.readback_end_qpc) - BigInt(request.qpc)) * 1000) / frequency
				});
			}
			const span = Number(BigInt(requests.at(-1).qpc) - BigInt(requests[0].qpc)) / frequency;
			const inWindow = matched.filter((image) => BigInt(image.qpc) <= BigInt(requests.at(-1).qpc));
			const latencies = matched.map((image) => image.copy_bound_ms);
			phases.push({
				playing,
				requests,
				matched,
				unknown,
				final: value,
				input_rate: 119 / span,
				distinct_presented_seek_updates_per_second: inWindow.length / span,
				latency_copy_bound_ms: {
					p50: percentile(latencies, 0.5),
					p95: percentile(latencies, 0.95),
					max: Math.max(...latencies)
				}
			});
		} finally {
			await capture.close(directory, playing ? 'playing' : 'paused');
		}
	}
	await mode(false);
	await seek(2000);
	const result = {
		geometry,
		box,
		region,
		references,
		phases,
		scope:
			'Physical foreground Media Preview pixels; exact reference signatures and unique seek intents. Playing allows at most two source frames of clock advance. Latency is a conservative pre-intent-to-pixel-copy bound, not scanout timing.'
	};
	await writeFile(path.join(directory, 'physical.json'), JSON.stringify(result, null, 2));
	return result;
}
