import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { windowCaptureSource } from './native-latency.mjs';

function edges(frame, width, height) {
	const red = [],
		blue = [];
	for (let x = 0; x < width; x++) {
		let image = 0,
			contour = 0;
		for (let y = 0; y < height; y++) {
			const offset = (y * width + x) * 4;
			const b = frame[offset],
				g = frame[offset + 1],
				r = frame[offset + 2];
			if (r > 160 && g < 80 && b < 100) image++;
			if (b > 180 && b - r > 85 && r > 70 && g > 110 && g < 215) contour++;
		}
		if (image >= height / 2) red.push(x);
		if (contour >= height / 2) blue.push(x);
	}
	if (!red.length || blue.length < 2) return null;
	const left = blue.filter((x) => x <= blue[0] + 3);
	const right = blue.filter((x) => x >= blue.at(-1) - 3);
	if (left.length + right.length !== blue.length || right[0] <= left.at(-1)) return null;
	const center = (values) => values.reduce((sum, value) => sum + value, 0) / values.length;
	return { image: [red[0], red.at(-1)], control: [center(left), center(right)] };
}

/** Follow the real red image and selected blue outline in the same composited window frames. */
export async function verifyNavigationPixels({ page, view, hwnd, label, navigate, interrupted }) {
	const viewport = await page.evaluate(() => ({
		width: innerWidth,
		height: innerHeight,
		dpr: devicePixelRatio
	}));
	const box = await view.boundingBox();
	assert(box);
	const { dpr } = viewport;
	const width = Math.floor(box.width * dpr) - 8,
		height = 6;
	const left = Math.round(box.x * dpr) + 4;
	// Fixture subjects are vertically centered. Sample beside the side handles.
	const top = Math.round((box.y + box.height / 2) * dpr) + 17;
	const source = windowCaptureSource({ hwnd, left, top, width, height, viewport, dpr });
	const capture = spawn(
		'ffmpeg',
		[
			'-hide_banner',
			'-loglevel',
			'error',
			'-filter_complex',
			`${source},hwdownload,format=bgra`,
			'-fps_mode',
			'passthrough',
			'-frames:v',
			'480',
			'-f',
			'rawvideo',
			'-pix_fmt',
			'bgra',
			'pipe:1'
		],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	const closed = new Promise((resolve) => capture.once('close', resolve));
	const frames = [];
	let buffer = Buffer.alloc(0),
		latest,
		recording = false,
		failure = '';
	capture.on('error', (error) => (failure = String(error)));
	capture.stdin.on('error', () => {});
	capture.stderr.on('data', (part) => (failure = (failure + part).slice(-8192)));
	const bytes = width * height * 4;
	capture.stdout.on('data', (part) => {
		buffer = Buffer.concat([buffer, part]);
		while (buffer.length >= bytes) {
			latest = edges(buffer, width, height);
			if (recording) frames.push(latest);
			buffer = buffer.subarray(bytes);
		}
	});
	let navigation;
	try {
		const until = Date.now() + 5000;
		while (!latest) {
			assert(
				capture.exitCode === null && Date.now() < until,
				`${label}: no paired red and blue edges. ${failure}`
			);
			await delay(10);
		}
		frames.push(latest);
		recording = true;
		navigation = await navigate();
		await delay(50);
		recording = false;
	} finally {
		if (capture.exitCode === null) capture.stdin.end('q\n');
		const timer = setTimeout(() => capture.kill(), 4000);
		try {
			assert.equal(await closed, 0, failure);
		} finally {
			clearTimeout(timer);
		}
	}
	assert(
		frames.length >= 4 && frames.every(Boolean),
		`${label}: missing edges in captured frames ${JSON.stringify(frames)}`
	);
	const widths = frames.map(({ image }) => image[1] - image[0]);
	const [low, high] = [Math.min(widths[0], widths.at(-1)), Math.max(widths[0], widths.at(-1))];
	// A resize changes the aperture, so the camera may pass outside the new endpoint widths.
	const intermediate = new Set(
		widths.filter((width) =>
			interrupted ? width !== low && width !== high : width > low && width < high
		)
	).size;
	assert(intermediate >= 2, `${label}: missing intermediate visible image sizes: ${widths}`);
	const maxSeparationPx = Math.max(
		...frames.flatMap(({ image, control }) =>
			image.map((edge, side) => Math.abs(edge - control[side]))
		)
	);
	assert(
		maxSeparationPx <= 4,
		`${label}: image and outline separated by ${maxSeparationPx} px; ${JSON.stringify(frames)}`
	);
	return { label, navigation, dpr, frames: frames.length, intermediate, maxSeparationPx };
}
