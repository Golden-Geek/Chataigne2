import assert from 'node:assert/strict';

export async function verifyNativeInput(browser, base) {
	const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	try {
		await page.goto(`${base}?mode=input`);
		await page.locator('[data-native-overlays="true"]').waitFor();
		// At the accepted native Left handle; far outside the DOM image/control.
		await page.mouse.click(130, 220);
		await page.waitForFunction(() =>
			window.__nativeInput.trace.some((event) => event.kind === 'left')
		);
		let trace = await page.evaluate(() => window.__nativeInput.trace);
		assert(trace.some((event) => event.kind === 'down' && Math.abs(event.x - 0.2) < 0.0001));
		assert(trace.some((event) => event.kind === 'up' && Math.abs(event.x - 0.2) < 0.0001));
		assert(!trace.some((event) => event.kind === 'right'));
		await page.mouse.move(130, 220);
		await page.mouse.down();
		await page.mouse.move(450, 400, { steps: 25 });
		await page.mouse.up();
		await page.waitForFunction(
			() => window.__nativeInput.trace.filter((event) => event.kind === 'up').length === 2
		);
		trace = await page.evaluate(() => window.__nativeInput.trace);
		const up = trace.filter((event) => event.kind === 'up').at(-1);
		assert(
			Math.abs(up.x - 1) < 0.0001 && up.y > 1,
			'captured drag keeps unclamped native image coordinates'
		);
		await page.mouse.move(130, 220);
		await page.mouse.down();
		await page.waitForFunction(
			() => window.__nativeInput.trace.filter((e) => e.kind === 'down').length === 3
		);
		const admitted = await page.evaluate(() => ({
			queries: window.__nativeInput.queries,
			moves: window.__nativeInput.trace.filter((e) => e.kind === 'move').length
		}));
		await page.evaluate(() => {
			window.__nativeInput.delay = 1000;
		});
		await page.mouse.move(450, 400, { steps: 25 });
		const immediate = await page.evaluate(() => ({
			queries: window.__nativeInput.queries,
			moves: window.__nativeInput.trace.filter((e) => e.kind === 'move').length
		}));
		assert.equal(
			immediate.queries,
			admitted.queries,
			'owned moves do not wait for another native IPC'
		);
		assert.equal(
			immediate.moves - admitted.moves,
			25,
			'every captured sample advances control feedback while native replies would take1s'
		);
		await page.mouse.up();
		await page.waitForFunction(
			() => window.__nativeInput.trace.filter((e) => e.kind === 'up').length === 3
		);
		assert.equal(
			await page.evaluate(() => window.__nativeInput.queries),
			admitted.queries + 1,
			'terminal hit/click still uses a native query'
		);
		await page.evaluate(() => {
			window.__nativeInput.delay = 30;
		});
		await page.mouse.move(130, 220);
		await page.mouse.down();
		await page.waitForFunction(
			() => window.__nativeInput.trace.filter((e) => e.kind === 'down').length === 4
		);
		const beforeGeometry = await page.evaluate(() => window.__nativeInput.trace.length);
		await page.evaluate(() => window.__nativeInput.moveImage());
		await page.mouse.move(150, 230);
		await page.mouse.up();
		const changedGeometry = await page.evaluate(
			(before) => window.__nativeInput.trace.slice(before),
			beforeGeometry
		);
		assert(
			changedGeometry.some((e) => e.kind === 'cancel'),
			'a changed DOM projection cancels capture'
		);
		assert(
			!changedGeometry.some((e) => e.kind === 'move' || e.kind === 'up'),
			'a changed projection never commits in a stale coordinate basis'
		);
		trace = await page.evaluate(() => window.__nativeInput.trace);
		await page.evaluate(() => {
			window.__nativeInput.delay = 150;
		});
		const before = trace.length;
		await page.mouse.move(130, 220);
		await page.mouse.down();
		await page.keyboard.press('Escape');
		await page.mouse.up();
		await page.waitForTimeout(220);
		trace = await page.evaluate(() => window.__nativeInput.trace);
		assert(
			!trace.slice(before).some((event) => event.kind === 'left' || event.kind === 'down'),
			'Escape fences delayed pointer admission'
		);
		await page.mouse.down();
		await page.evaluate(() => window.__nativeInput.changeSource());
		await page.mouse.up();
		await page.waitForTimeout(220);
		assert.equal(
			await page.evaluate(() => window.__nativeInput.trace.length),
			trace.length,
			'source replacement fences delayed native input'
		);
		assert.equal(await page.evaluate(() => window.__nativeInput.maximumQueries), 1);
		assert.deepEqual(errors, []);
		return {
			retargetedNativeHandle: true,
			projectedDownUp: true,
			coalescedDrag: true,
			capturedMovesIndependentOfNativeReplies: 25,
			geometryFence: true,
			outsideApertureDrag: true,
			escapeFence: true,
			sourceFence: true,
			maximumQueries: 1
		};
	} finally {
		await page.close();
	}
}
