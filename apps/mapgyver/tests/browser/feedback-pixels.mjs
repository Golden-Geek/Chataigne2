import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { captureOwnedWindow } from './navigation-pixels.mjs';

/** Sample native contour stroke and the red image beneath its selection fill. */
export async function verifyFeedbackPixels({ page, view, hwnd, contour, peer, label, hoverOnly }) {
	const preview = view.locator('.native-preview');
	const installed = () =>
		view
			.locator('.native-preview[data-preview-installed="true"][data-native-overlays="true"]')
			.waitFor();
	await installed();
	const viewport = await page.evaluate(() => ({
		width: innerWidth,
		height: innerHeight,
		dpr: devicePixelRatio
	}));
	const { dpr } = viewport;
	const viewportBox = await view.boundingBox();
	const box = await contour.boundingBox();
	assert(viewportBox && box && box.width * dpr > 65 && box.height * dpr > 45, label);
	const blank = { x: viewportBox.x + 4, y: viewportBox.y + 4 };
	const inside = { x: box.x + 0.32 * box.width, y: box.y + 0.32 * box.height };
	const selectedState = `contains(concat(' ', normalize-space(@class), ' '), ' selected ') or @aria-pressed = 'true'`;
	const selected = (target, value) =>
		target.locator(`xpath=self::*[${value ? selectedState : `not(${selectedState})`}]`).waitFor();
	const clear = async () => {
		await page.mouse.move(blank.x, blank.y);
		await view.press('Escape');
		await selected(contour, false);
	};
	const select = async () => {
		await contour.press('Enter');
		await selected(contour, true);
	};
	if (hoverOnly) await page.mouse.move(blank.x, blank.y);
	else await clear();
	await installed();
	const lease = await preview.getAttribute('data-preview-view');
	const paintOwner = await preview.evaluateHandle((node) => {
		let lost = false;
		let pending = false;
		const observer = new MutationObserver((records) => {
			for (const record of records) {
				if (record.attributeName === 'data-native-overlays') lost ||= record.oldValue !== 'true';
				else pending ||= record.oldValue === 'true';
			}
		});
		observer.observe(node, {
			attributes: true,
			attributeOldValue: true,
			attributeFilter: ['data-native-overlays', 'data-preview-installed']
		});
		return () => {
			observer.disconnect();
			return {
				stayedNative: !lost && node.isConnected && node.dataset.nativeOverlays === 'true',
				pending
			};
		};
	});
	const read = (frame, width, height) => {
		let stroke = 0;
		let strokeBlue = 140;
		let fill = 0;
		for (let y = 0; y < height; y++) {
			for (let x = 1; x < 12; x++) {
				const index = (y * width + x) * 4;
				if (frame[index] > strokeBlue) {
					strokeBlue = frame[index];
					stroke = frame[index + 2];
				}
			}
			for (let x = 20; x < width; x++) {
				const index = (y * width + x) * 4;
				if (!hoverOnly && frame[index + 2] < 160) return null;
				fill += frame[index];
			}
		}
		return stroke ? { stroke, fill: Math.round(fill / (height * (width - 20))) } : null;
	};
	let capture;
	let ownership;
	try {
		capture = await captureOwnedWindow({
			hwnd,
			viewport,
			bounds: {
				left: Math.round(box.x * dpr) - 6,
				top: Math.round(inside.y * dpr),
				width: 32,
				height: 6
			},
			label,
			read,
			action: async (length) => {
				const ranges = {};
				async function step(name, change) {
					const start = length() - 1;
					await change();
					await delay(195);
					ranges[name] = [start, length()];
				}
				await step('hover', () => page.mouse.move(inside.x, inside.y));
				await step('leave', () => page.mouse.move(blank.x, blank.y));
				if (hoverOnly) return ranges;
				await step('select', select);
				assert.equal(await contour.getAttribute('data-viewport-focused'), 'true');
				await step('rapidRetarget', async () => {
					await clear();
					await select();
					await clear();
				});
				if (peer) {
					await peer.press('Enter');
					await selected(peer, true);
					await delay(195);
					await step('peerSelect', select);
					await clear();
					await delay(195);
				}
				await page.emulateMedia({ reducedMotion: 'reduce' });
				await step('reduced', select);
				return ranges;
			}
		});
	} finally {
		await page.emulateMedia({ reducedMotion: 'no-preference' });
		ownership = await paintOwner.evaluate((finish) => finish());
		await paintOwner.dispose();
	}
	assert(
		ownership.stayedNative,
		`${label}: completed paint was presented by the same native owner`
	);
	assert(
		ownership.pending,
		`${label}: changed paint must receive a fresh installation acknowledgement`
	);
	await installed();
	assert.equal(await preview.getAttribute('data-preview-view'), lease);
	assert(capture.frames.every(Boolean), `${label}: source image and contour stay visible`);
	function colors(phase, key) {
		const [start, end] = capture.result[phase];
		return capture.frames.slice(start, end).map((frame) => frame[key]);
	}
	function intermediate(values) {
		const low = Math.min(values[0], values.at(-1));
		const high = Math.max(values[0], values.at(-1));
		return new Set(values.filter((value) => value > low && value < high)).size;
	}
	const hover = colors('hover', 'stroke');
	// Stroke width changes immediately. Its subpixel coverage can jump once, while the paint
	// should still pass through multiple distinct colors before settling.
	assert(new Set(hover).size >= 4, `${label} hover pixels: ${hover}`);
	assert.equal(colors('leave', 'stroke').at(-1), hover[0], `${label} restores the idle contour`);
	if (hoverOnly) return { label, hover, installed: true };
	const selection = colors('select', 'fill');
	const rapid = colors('rapidRetarget', 'fill');
	const reduced = colors('reduced', 'fill');
	assert(
		selection.at(-1) > selection[0] && intermediate(selection) >= 2,
		`${label} selection pixels: ${selection}`
	);
	assert.equal(rapid.at(-1), selection[0], `${label} clears after rapid retargeting: ${rapid}`);
	assert.equal(reduced.at(-1), selection.at(-1));
	assert.equal(intermediate(reduced), 0, `${label} reduced motion pixels: ${reduced}`);
	const peerSelection = peer ? colors('peerSelect', 'fill') : null;
	if (peerSelection)
		assert(
			peerSelection.at(-1) === selection.at(-1) && intermediate(peerSelection) >= 2,
			`${label} paint must stay with its contour when a peer was selected: ${peerSelection}`
		);
	return { label, hover, selection, rapid, reduced, peerSelection, installed: true };
}
