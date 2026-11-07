// One bounded Composition acceptance cohort through the established WGC window source.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { stat, writeFile, readFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { ownedWindowHandle, ownedWindowGeometry } from './capture-alignment.mjs';
import { edges, windowCaptureSource } from './native-latency.mjs';

const now = () => process.hrtime.bigint();
const ms = (ticks) => Number(ticks) / 1e6;

export async function measureCompositionCadence({ native, directory, shape, control }) {
	const { page } = native;
	const hwnd = Number(ownedWindowHandle(native.app.pid));
	await writeFile(
		path.join(directory, 'cadence-ready.json'),
		JSON.stringify({
			pid: native.app.pid,
			hwnd,
			binary: process.env.MAPGYVER_TEST_BINARY
		})
	);
	const deadline = Date.now() + 180000;
	while (!(await stat(path.join(directory, 'cadence-activated')).catch(() => null))) {
		assert.equal(native.app.exitCode, null, 'Owned cadence host closed before activation');
		assert(Date.now() < deadline, 'Owned cadence window activation expired');
		await delay(100);
	}
	assert.equal(native.app.exitCode, null, 'Owned cadence host closed before activation');
	const geometry = ownedWindowGeometry(native.app.pid, String(hwnd));
	const projection = await shape.evaluate((element) => {
		const matrix = element.ownerSVGElement.getScreenCTM();
		const corners = [...element.points].map((point) => point.matrixTransform(matrix));
		const at = (u, v) => ({
			x: corners[0].x + (corners[1].x - corners[0].x) * u + (corners[3].x - corners[0].x) * v,
			y: corners[0].y + (corners[1].y - corners[0].y) * u + (corners[3].y - corners[0].y) * v
		});
		const inner = [at(0.1, 0.1), at(0.9, 0.1), at(0.9, 0.9), at(0.1, 0.9)];
		const y = (corners[0].y + corners[2].y) / 2;
		const left = (quad) =>
			Math.min(
				...quad.flatMap((a, index) => {
					const b = quad[(index + 1) % 4];
					return y >= Math.min(a.y, b.y) && y < Math.max(a.y, b.y)
						? [a.x + ((b.x - a.x) * (y - a.y)) / (b.y - a.y)]
						: [];
				})
			);
		return {
			outer_left: left(corners),
			inner_left: left(inner),
			y,
			viewport: { width: innerWidth, height: innerHeight },
			dpr: devicePixelRatio,
			corners: corners.map(({ x, y }) => [x, y])
		};
	});
	assert.equal(geometry.window_dpi, projection.dpr * 96);
	const width = 192,
		height = 8,
		bytes = width * height * 4;
	const region = {
		left: Math.floor(projection.outer_left * projection.dpr) - 64,
		top: Math.floor(projection.y * projection.dpr) - 4,
		width,
		height
	};
	assert(region.left >= 0 && region.top >= 0);
	const source = windowCaptureSource({ hwnd, ...region, ...projection });
	const args = [
		'-hide_banner',
		'-v',
		'info',
		'-filter_complex',
		`${source},hwdownload,format=bgra,showinfo`,
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
	const child = spawn('ffmpeg', args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
	const closed = new Promise((resolve) =>
		child.once('close', (code, signal) => resolve({ code, signal }))
	);
	let problem = null,
		failure = null,
		log = '',
		line = '',
		buffer = Buffer.alloc(0);
	let begin = null,
		end = null,
		requested = 0;
	const frames = [],
		pixels = [],
		timestamps = [];
	child.on('error', (error) => {
		problem = String(error);
	});
	child.stdin.on('error', () => {});
	child.stderr.on('data', (chunk) => {
		log = (log + chunk).slice(-65536);
		line += chunk;
		const lines = line.split('\n');
		line = lines.pop();
		for (const row of lines) {
			const match = row.match(/\bn:\s*(\d+)\s+pts:\s*(-?\d+)/);
			if (match && timestamps.length < 3601)
				timestamps.push({ serial: Number(match[1]), ticks: match[2] });
		}
	});
	child.stdout.on('data', (chunk) => {
		buffer = Buffer.concat([buffer, chunk]);
		while (buffer.length >= bytes) {
			const raw = Buffer.from(buffer.subarray(0, bytes));
			buffer = buffer.subarray(bytes);
			if (frames.length >= 3600) {
				problem = 'Bounded cadence cohort overflow';
				continue;
			}
			frames.push({ serial: frames.length, arrival_ns: now().toString() });
			pixels.push(raw);
		}
	});
	async function wait(test, label) {
		const until = Date.now() + 10000;
		while (!test()) {
			assert(!problem && child.exitCode === null, `${label}: ${problem ?? log}`);
			assert(Date.now() < until, label);
			await delay(10);
		}
	}
	let events = [];
	try {
		await wait(
			() => frames.length >= 10 && edges(pixels.at(-1), width, height),
			'Window capture needs actual independent image/control pixels'
		);
		const box = await control.boundingBox();
		assert(box);
		const point = { x: box.x + box.width * 0.43, y: box.y + box.height * 0.43 };
		await page.mouse.move(point.x, point.y);
		await page.evaluate(() => {
			window.__cadenceEvents = [];
			window.__cadenceListener = (event) => {
				if (window.__cadenceEvents.length < 1600)
					window.__cadenceEvents.push({
						type: event.type,
						trusted: event.isTrusted,
						buttons: event.buttons,
						x: event.clientX,
						y: event.clientY,
						at: performance.now()
					});
			};
			for (const type of ['pointerdown', 'pointermove', 'pointerup'])
				document.addEventListener(type, window.__cadenceListener, true);
		});
		begin = now();
		await page.mouse.down();
		try {
			while (ms(now() - begin) < 8000) {
				assert(!problem && child.exitCode === null, 'Capture stopped during input');
				const time = ms(now() - begin);
				const dx = 40 * (2 / Math.PI) * Math.asin(Math.sin((time * Math.PI * 2) / 1000));
				await page.mouse.move(point.x + dx, point.y);
				requested++;
				assert(requested < 1500, 'Bounded input cohort');
			}
		} finally {
			end = now();
			await page.mouse.up();
		}
		events = await page.evaluate(() => {
			for (const type of ['pointerdown', 'pointermove', 'pointerup'])
				document.removeEventListener(type, window.__cadenceListener, true);
			return window.__cadenceEvents;
		});
		await delay(200);
	} catch (error) {
		failure = String(error);
	} finally {
		child.stdin.end('q');
		let exit = await Promise.race([closed, delay(3000, null)]);
		if (!exit) {
			child.kill();
			exit = await closed;
		}
		let after = null;
		try {
			after = ownedWindowGeometry(native.app.pid, String(hwnd));
		} catch (error) {
			failure ??= String(error);
		}
		const result = {
			pid: native.app.pid,
			hwnd,
			geometry,
			after,
			projection,
			region,
			expected_separation_px: (projection.inner_left - projection.outer_left) * projection.dpr,
			command: ['ffmpeg', ...args],
			begin_ns: begin?.toString(),
			end_ns: end?.toString(),
			requested,
			events,
			frames,
			timestamps,
			exit,
			failure,
			problem
		};
		await writeFile(path.join(directory, 'cadence.json'), JSON.stringify(result, null, 2));
		await writeFile(path.join(directory, 'cadence.bgra'), Buffer.concat(pixels));
		await writeFile(path.join(directory, 'cadence.log'), log);
		assert.deepEqual(after, geometry, 'Owned foreground geometry/DPI retained');
		assert.deepEqual(exit, { code: 0, signal: null });
		assert.equal(failure, null);
		assert.equal(problem, null);
	}
	execFileSync('python', ['apps/mapgyver/tests/native/motion_cadence.py', directory], {
		windowsHide: true
	});
	return JSON.parse(await readFile(path.join(directory, 'cadence-audit.json'), 'utf8'));
}
