import assert from 'node:assert/strict';

export async function feedbackChecks(page) {
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	const fixture = page.getByRole('region', { name: 'Feedback fixture' });
	const contour = page.getByRole('button', { name: 'Feedback contour', exact: true });
	const peer = page.getByRole('button', { name: 'Peer contour', exact: true });
	const handle = page.getByRole('button', { name: 'Feedback handle', exact: true });
	await fixture.scrollIntoViewIfNeeded();
	const peerPaint = await peer.evaluate((node) => getComputedStyle(node).borderColor);
	const bounds = await handle.boundingBox();
	assert(bounds);
	await page.evaluate(() => {
		window.__paintFrames = [];
		const until = performance.now() + 320;
		function sample() {
			const node = document.querySelector('[aria-label="Feedback contour"]');
			window.__paintFrames.push(getComputedStyle(node).borderColor);
			if (performance.now() < until) requestAnimationFrame(sample);
		}
		requestAnimationFrame(sample);
	});
	await page.getByRole('button', { name: 'Toggle feedback selection', exact: true }).click();
	await page.waitForTimeout(340);
	const frames = await page.evaluate(() => window.__paintFrames);
	assert(new Set(frames).size >= 3, 'selection paint must present intermediate colors');
	assert.equal(frames.at(-1), 'rgb(112, 170, 255)');
	assert.equal(await peer.evaluate((node) => getComputedStyle(node).borderColor), peerPaint);
	assert.deepEqual(await handle.boundingBox(), bounds, 'paint never changes handle bounds');
	await handle.focus();
	await page.keyboard.press('Shift+Tab');
	await page.keyboard.press('Tab');
	assert.equal(await handle.evaluate((node) => getComputedStyle(node).outlineStyle), 'solid');
	await page.getByRole('button', { name: 'Toggle feedback selection', exact: true }).click();
	await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
	await page.mouse.down();
	assert.equal(
		await page.getByLabel('Feedback presses').textContent(),
		'1',
		'press is admitted during paint'
	);
	await page.mouse.up();
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.getByRole('button', { name: 'Toggle feedback selection', exact: true }).click();
	assert.equal(
		await contour.evaluate((node) => getComputedStyle(node).borderColor),
		'rgb(112, 170, 255)'
	);
	assert.equal(await contour.evaluate((node) => getComputedStyle(node).transitionDuration), '0s');
	const property = await handle.evaluate((node) => getComputedStyle(node).transitionProperty);
	assert(!/transform|left|top|width|height/.test(property), 'geometry is never eased');
	return {
		intermediatePaint: true,
		immediateInput: true,
		constantHits: true,
		independentPeer: true,
		keyboardFocus: true,
		reducedMotion: true
	};
}
