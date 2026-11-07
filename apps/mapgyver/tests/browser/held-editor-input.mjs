// Passive DOM/input witnesses avoid adding a CDP read to each pointer sample.
import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';

export async function heldEditorInput({ page, name, control, shape, inspector = false, preview }) {
	await control.scrollIntoViewIfNeeded();
	const box = await control.boundingBox();
	assert(box);
	const point = { x: box.x + box.width * 0.43, y: box.y + box.height * 0.43 };
	await page.mouse.move(point.x, point.y);
	const view = preview ? await preview.getAttribute('data-preview-view') : null;
	await shape.evaluate((element) => {
		const value = () =>
			element.tagName.toLowerCase() === 'rect'
				? ['x', 'y', 'width', 'height'].map((key) => element.getAttribute(key)).join(',')
				: element.getAttribute('points');
		const trace = {
			initial: value(),
			last: value(),
			held: false,
			changes: [],
			moves: [],
			up: null
		};
		const down = (event) => {
			if (event.isTrusted) trace.held = true;
		};
		const up = (event) => {
			if (event.isTrusted) {
				trace.held = false;
				trace.up = performance.now();
			}
		};
		const move = (event) => {
			if (event.isTrusted && trace.held)
				trace.moves.push({ at: performance.now(), x: event.clientX, y: event.clientY });
		};
		const observer = new MutationObserver(() => {
			const next = value();
			if (next !== trace.last) {
				if (trace.held) trace.changes.push({ at: performance.now(), value: next });
				trace.last = next;
			}
		});
		observer.observe(element, {
			attributes: true,
			attributeFilter: ['x', 'y', 'width', 'height', 'points']
		});
		document.addEventListener('pointerdown', down, true);
		document.addEventListener('pointerup', up, true);
		document.addEventListener('pointermove', move, true);
		element.__heldEditorTrace = {
			trace,
			stop() {
				observer.disconnect();
				document.removeEventListener('pointerdown', down, true);
				document.removeEventListener('pointerup', up, true);
				document.removeEventListener('pointermove', move, true);
			}
		};
	});
	if (inspector) await page.keyboard.down('Shift');
	await page.mouse.down();
	try {
		for (let index = 1; index <= 72; index++) {
			const progress = inspector ? (12 - Math.abs(12 - (index % 24))) / 12 : index / 72;
			await page.mouse.move(
				point.x + (inspector ? 12 : 32) * progress,
				point.y + (inspector ? 0 : 10) * progress
			);
			await delay(8);
		}
	} finally {
		await page.mouse.up();
		if (inspector) await page.keyboard.up('Shift');
	}
	const trace = await shape.evaluate((element) => {
		element.__heldEditorTrace.stop();
		return element.__heldEditorTrace.trace;
	});
	assert(
		trace.up !== null && trace.changes.length > 2,
		`${name}: geometry must advance before release`
	);
	assert(
		trace.moves.length > 60 && trace.moves.length <= 74,
		`${name}: unexpected input ${trace.moves.length}`
	);
	assert(trace.changes.every((change) => change.at <= trace.up));
	await delay(180);
	if (preview) {
		assert.equal(
			await preview.getAttribute('data-preview-view'),
			view,
			'independent preview lease retained'
		);
		assert.equal(await preview.getAttribute('data-preview-state'), 'available');
	}
	return {
		name,
		held_dom_changes: trace.changes.length,
		trace,
		independent_preview: view,
		limit:
			'Trusted pointer/held DOM geometry and native preview identity only; no presented video FPS/latency claim.'
	};
}
