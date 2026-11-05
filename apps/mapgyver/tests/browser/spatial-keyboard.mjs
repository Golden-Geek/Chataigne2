import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';

const directions = {
	TopLeft: [0, 0],
	Top: [0.5, 0],
	TopRight: [1, 0],
	Right: [1, 0.5],
	BottomRight: [1, 1],
	Bottom: [0.5, 1],
	BottomLeft: [0, 1],
	Left: [0, 0.5]
};
export function assertRectKeyboardResize(before, after, name, natural) {
	const size = before.size ?? natural,
		direction = directions[name];
	assert.deepEqual(after, { ...before, position: after.position, size: after.size });
	for (let axis = 0; axis < 2; axis++) {
		const key = axis === 0 ? 'width' : 'height',
			opposite = 1 - direction[axis];
		assert.equal(
			after.size[key],
			size[key] + (direction[axis] === 0.5 ? 0 : [3, 2][axis] * (2 * direction[axis] - 1))
		);
		assert(
			Math.abs(
				after.position[axis] +
					opposite * after.size[key] -
					before.position[axis] -
					opposite * size[key]
			) < 0.0001
		);
	}
}

/** Actual native keys only; backend reads establish exact grouped document history. */
export async function verifySpatialKeyboard({
	page,
	view,
	inspect,
	position,
	ready,
	resize,
	rotate
}) {
	const wait = async (test) => {
		const until = Date.now() + 9000;
		for (;;) {
			const snapshot = await inspect();
			if (test(snapshot)) return snapshot;
			assert(Date.now() < until, JSON.stringify(snapshot));
			await delay(25);
		}
	};
	const baseline = await inspect(),
		origin = position(baseline),
		reports = [];
	const at = (delta) => (value) =>
		position(value).every(
			(coordinate, axis) => Math.abs(coordinate - origin[axis] - delta[axis]) < 0.0001
		);
	const undo = async (expected) => {
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((value) => isDeepStrictEqual(value, expected));
		await ready();
	};
	const history = async (before, after) => {
		await undo(before);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait((value) => isDeepStrictEqual(value, after));
		await ready();
		await undo(before);
	};
	async function arrows(locator, delta) {
		await ready();
		const deadline = Date.now() + 9000;
		while (!(await locator.isEnabled())) {
			assert(Date.now() < deadline, 'keyboard handle admission did not settle');
			await delay(25);
		}
		await locator.focus();
		assert(
			await locator.evaluate((element) => document.activeElement === element),
			'keyboard resize/rotation handle must own focus'
		);
		for (let i = 0; i < delta[0]; i++) await page.keyboard.down('ArrowRight');
		for (let i = 0; i < delta[1]; i++) await page.keyboard.down('ArrowDown');
		await page.keyboard.up('ArrowRight');
		await page.keyboard.up('ArrowDown');
		await ready();
	}
	await ready();
	await view.focus();
	for (let i = 0; i < 12; i++) await page.keyboard.down('ArrowRight');
	await page.keyboard.down('Shift');
	await page.keyboard.down('ArrowDown');
	await page.keyboard.up('ArrowDown');
	await page.keyboard.up('Shift');
	await page.keyboard.up('ArrowRight');
	const held = await wait(at([12, 10]));
	await ready();
	await history(baseline, held);
	reports.push('held-repeat-Shift-one-history-group');
	await view.focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowRight');
	const tapped = await wait(at([2, 0]));
	await ready();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(at([1, 0]));
	await ready();
	await undo(baseline);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(at([1, 0]));
	await ready();
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait((value) => isDeepStrictEqual(value, tapped));
	await ready();
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(at([1, 0]));
	await ready();
	await undo(baseline);
	reports.push('rapid-taps-own-history-and-fresh-origin');
	await view.focus();
	await page.keyboard.press('Control+ArrowRight');
	assert.deepEqual(await inspect(), baseline);
	await page.keyboard.down('ArrowDown');
	const blurred = await wait(at([0, 1]));
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.focus();
	await ready();
	await view.focus();
	await page.keyboard.down('ArrowDown');
	await page.keyboard.up('ArrowDown');
	assert.deepEqual(
		await inspect(),
		blurred,
		'focus loss retires the group; stale repeats cannot restart it'
	);
	await history(baseline, blurred);
	reports.push('focus-loss-and-stale-repeat-fence');
	await view.focus();
	await page.keyboard.down('ArrowRight');
	await wait(at([1, 0]));
	await page.keyboard.press('Escape');
	await ready();
	const escaped = await inspect();
	await page.keyboard.down('ArrowRight');
	await page.keyboard.up('ArrowRight');
	assert.deepEqual(await inspect(), escaped, 'Escape retires queued/repeated keyboard input');
	await history(baseline, escaped);
	reports.push('Escape-retirement');
	if (resize) {
		await ready();
		const first = resize.locator('TopLeft');
		const deadline = Date.now() + 9000;
		while (!(await first.isEnabled()) || (await first.getAttribute('tabindex')) !== '0') {
			assert(Date.now() < deadline, 'Tab admission did not settle');
			await delay(25);
		}
		await view.focus();
		for (
			let step = 0;
			step < 16 && !(await first.evaluate((element) => document.activeElement === element));
			step++
		)
			await page.keyboard.press('Tab');
		assert(
			await first.evaluate((element) => document.activeElement === element),
			'resize handle must be reachable with ordinary Tab navigation'
		);
		reports.push('Tab-reachable-resize-handles');
		for (const name of Object.keys(directions)) {
			console.log(JSON.stringify({ keyboardResize: name }));
			await arrows(resize.locator(name), [3, 2]);
			const after = await wait((value) => !isDeepStrictEqual(value, baseline));
			await ready();
			resize.validate(baseline, after, name);
			await history(baseline, after);
			reports.push(`keyboard-resize-${name}`);
		}
	}
	if (rotate)
		for (const name of ['TopLeft', 'TopRight', 'BottomRight', 'BottomLeft']) {
			await arrows(rotate.locator(name), [3, 2]);
			const after = await wait((value) => !isDeepStrictEqual(value, baseline));
			await ready();
			rotate.validate(baseline, after, name);
			await history(baseline, after);
			reports.push(`keyboard-rotate-${name}`);
		}
	assert.deepEqual(await inspect(), baseline);
	return reports;
}
