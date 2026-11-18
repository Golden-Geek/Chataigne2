import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { windowCaptureSource } from './native-latency.mjs';

const run = promisify(execFile);

/** Compare an independently captured native image edge to the snapped DOM guide. */
export async function verifyAlignmentPixels(native, view, axis) {
	const hwnd = Number(ownedWindowHandle(native.app.pid));
	assert(Number.isSafeInteger(hwnd) && hwnd > 0);
	const projection = () =>
		view.evaluate((node, axis) => {
			const svg = node.querySelector('[data-alignment-guides]');
			const line =
				svg &&
				[...svg.querySelectorAll('line')].find((entry) =>
					axis === 'x'
						? entry.x1.baseVal.value === entry.x2.baseVal.value
						: entry.y1.baseVal.value === entry.y2.baseVal.value
				);
			if (!line) return null;
			const point = new DOMPoint(line.x1.baseVal.value, line.y1.baseVal.value).matrixTransform(
				line.getScreenCTM()
			);
			return { guide: point[axis], dpr: devicePixelRatio, width: innerWidth, height: innerHeight };
		}, axis);
	const before = await projection();
	assert(before);
	const expected = Math.round(before.guide * before.dpr);
	const width = axis === 'x' ? 64 : 8,
		height = axis === 'x' ? 8 : 64;
	// Sample through the moving red rectangle, away from its selected handles and its peer.
	const zone = await view.locator('.active-zone').boundingBox();
	assert(zone);
	const options = {
		hwnd,
		width,
		height,
		left: axis === 'x' ? expected - 32 : Math.round((zone.x + zone.width * 0.6) * before.dpr),
		top: axis === 'y' ? expected - 32 : Math.round((zone.y + zone.height * 0.415) * before.dpr),
		viewport: before,
		dpr: before.dpr
	};
	const handles = await view
		.locator('[data-instance-resize]')
		.evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().toJSON()));
	assert(
		handles.length > 0 &&
			handles.every(
				(box) =>
					options.left + width <= box.left * before.dpr ||
					options.top + height <= box.top * before.dpr ||
					options.left >= box.right * before.dpr ||
					options.top >= box.bottom * before.dpr
			),
		'The capture strip must not intersect a resize handle'
	);
	const { stdout } = await run(
		'ffmpeg',
		[
			'-hide_banner',
			'-v',
			'error',
			'-init_hw_device',
			'd3d11va=owned:0',
			'-filter_hw_device',
			'owned',
			'-filter_complex',
			`${windowCaptureSource(options)},hwdownload,format=bgra`,
			'-frames:v',
			'1',
			'-f',
			'rawvideo',
			'-pix_fmt',
			'bgra',
			'pipe:1'
		],
		{ windowsHide: true, timeout: 8000, encoding: 'buffer' }
	);
	assert.deepEqual(
		await projection(),
		before,
		'Held guide and viewport remain stationary during capture'
	);
	assert.equal(stdout.length, width * height * 4);
	const reds = [],
		cyan = [];
	for (let offset = 0; offset < 64; offset++) {
		let red = 0,
			guide = 0;
		for (let across = 0; across < 8; across++) {
			const index = (axis === 'x' ? across * width + offset : offset * width + across) * 4;
			const [b, g, r] = stdout.subarray(index, index + 3);
			if (r > 160 && g < 110 && b < 125) red++;
			if (g > 125 && b > 150 && r < 115 && g - r > 45) guide++;
		}
		if (red >= 5) reds.push(offset);
		if (guide >= 2) cyan.push(offset);
	}
	assert(
		reds.includes(0) && !reds.includes(63),
		'The sample must straddle a real red-to-background edge'
	);
	const edge = reds.at(-1) + 1;
	assert(
		cyan.length > 0 && cyan.length <= 5,
		`Expected a thin painted guide in the same native frame: ${JSON.stringify({ reds, cyan })}`
	);
	const guide = cyan.reduce((sum, value) => sum + value, 0) / cyan.length;
	assert(Math.abs(guide - 32) <= 16, 'Captured guide must be near the CSS projection');
	assert(
		Math.abs(edge - guide) <= 4,
		`Native ${axis} edge differs from the same-frame guide by ${edge - guide} physical pixels`
	);
	return {
		axis,
		expected,
		errorPixels: edge - guide,
		captureOffsetPixels: guide - 32,
		dpr: before.dpr
	};
}
