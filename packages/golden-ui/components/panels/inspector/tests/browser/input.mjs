import assert from 'node:assert/strict';

export async function checkCurveInput(page, canvas, selected, points) {
	await canvas.evaluate((element) =>
		element.addEventListener(
			'pointerdown',
			(event) => {
				if (event.isTrusted) element.dataset.inputPointer = String(event.pointerId);
			},
			{ capture: true }
		)
	);
	const writes = () => page.getByLabel('Curve writes').textContent().then(Number);
	const settle = () =>
		page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
	const key = page.getByRole('button', { name: 'Select Curve A key 0', exact: true });
	const handles = () =>
		canvas.evaluate((element) => {
			const pixels = element
				.getContext('2d')
				.getImageData(0, 0, element.width, element.height).data;
			const dots = [];
			for (let i = 0; i < pixels.length; i += 4)
				if (
					(pixels[i] === 253 && pixels[i + 1] === 0 && pixels[i + 2] === 253) ||
					(pixels[i] === 0 && pixels[i + 1] === 253 && pixels[i + 2] === 253)
				)
					dots.push({
						x: (i / 4) % element.width,
						y: Math.floor(i / 4 / element.width),
						out: pixels[i] === 253
					});
			return dots;
		});
	const box = await canvas.boundingBox();
	const background = { x: box.x + box.width * 0.7, y: box.y + box.height * 0.85 };
	await key.click();
	await settle();
	assert((await handles()).length > 5, 'selected Bezier keys expose their real controls');
	const initial = await writes();
	await page.mouse.move(background.x, background.y);
	await page.mouse.down();
	assert.equal(await selected.textContent(), 'Curve A', 'background inspects owner before release');
	await page.mouse.up();
	assert.equal(await selected.textContent(), 'Curve A');
	await settle();
	assert.equal((await handles()).length, 0, 'owner selection withdraws Bezier handles');
	const middle = { x: (points[0].x + points[1].x) / 2, y: (points[0].y + points[1].y) / 2 };
	const paint = (point) =>
		canvas.evaluate((element, point) => {
			const rect = element.getBoundingClientRect();
			const x = Math.round(((point.x - rect.x) * element.width) / rect.width);
			const y = Math.round(((point.y - rect.y) * element.height) / rect.height);
			return [...element.getContext('2d').getImageData(x - 3, y - 3, 7, 7).data];
		}, point);
	const idleContour = await paint(middle),
		idleFill = await paint(background);
	await page.mouse.move(middle.x, middle.y);
	await settle();
	assert.notDeepEqual(await paint(middle), idleContour, 'unselected hover highlights the contour');
	assert.deepEqual(await paint(background), idleFill, 'contour hover preserves the area fill');
	assert.equal((await handles()).length, 0, 'unselected segment hover never exposes handles');
	assert.equal(await selected.textContent(), 'Curve A', 'hover does not select');
	await page.mouse.down();
	assert.equal(
		await selected.textContent(),
		'Easing',
		'segment selects the real easing node before release'
	);
	await canvas.evaluate((element) => {
		const rect = element.getBoundingClientRect();
		element.dispatchEvent(
			new PointerEvent('pointermove', {
				pointerId: Number(element.dataset.inputPointer),
				clientX: rect.x + rect.width * 0.6,
				clientY: rect.y + rect.height * 0.3,
				buttons: 1,
				bubbles: true
			})
		);
		element.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
	});
	await page.mouse.up();
	assert.equal(await writes(), initial, 'segment cancellation cannot flush sculpt edits');
	await page.getByRole('button', { name: 'Select key value', exact: true }).click();
	await canvas.focus();
	await page.keyboard.press('Escape');
	assert.equal(
		await selected.textContent(),
		'Curve A',
		'idle Escape clears actual field selection'
	);
	await key.click();
	await page.keyboard.down('Shift');
	await page.mouse.move(background.x, background.y);
	await page.mouse.down();
	assert.equal(
		await selected.textContent(),
		'Curve A key 0',
		'additive background keeps selection'
	);
	await page.mouse.move(background.x - 50, background.y - 50);
	await page.keyboard.press('Escape');
	await page.mouse.up();
	await page.keyboard.up('Shift');
	assert.equal(
		await selected.textContent(),
		'Curve A key 0',
		'marquee Escape cancels without selecting hits'
	);

	const beginKey = async () => {
		await key.click();
		await page.mouse.move(points[0].x, points[0].y);
		await page.mouse.down();
		assert.equal(await selected.textContent(), 'Curve A key 0');
		assert(
			await canvas.evaluate((element) =>
				element.hasPointerCapture(Number(element.dataset.inputPointer))
			)
		);
	};
	await beginKey();
	const out = (await handles()).filter((point) => point.out);
	assert(out.length > 0);
	await page.keyboard.press('Escape');
	await page.mouse.up();
	const outPoint = {
		x:
			box.x +
			((out.reduce((sum, point) => sum + point.x, 0) / out.length) * box.width) /
				(await canvas.evaluate((element) => element.width)),
		y:
			box.y +
			((out.reduce((sum, point) => sum + point.y, 0) / out.length) * box.height) /
				(await canvas.evaluate((element) => element.height))
	};
	await page.mouse.move(outPoint.x, outPoint.y);
	await page.mouse.down();
	await canvas.evaluate((element) => {
		const pointerId = Number(element.dataset.inputPointer),
			rect = element.getBoundingClientRect();
		element.dispatchEvent(
			new PointerEvent('pointermove', {
				pointerId,
				clientX: rect.x + 200,
				clientY: rect.y + 70,
				buttons: 1,
				bubbles: true
			})
		);
		element.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
	});
	await page.mouse.up();
	await settle();
	assert.equal(await writes(), initial, 'Bezier handle cancellation discards queued control edits');
	await beginKey();
	await canvas.evaluate((element) => {
		const pointerId = Number(element.dataset.inputPointer),
			rect = element.getBoundingClientRect();
		element.dispatchEvent(new PointerEvent('pointercancel', { pointerId: pointerId + 100 }));
		if (!element.hasPointerCapture(pointerId)) throw Error('foreign pointer canceled capture');
		element.dispatchEvent(
			new PointerEvent('pointerdown', {
				pointerId: pointerId + 100,
				button: 0,
				clientX: rect.x + 200,
				clientY: rect.y + 200,
				bubbles: true
			})
		);
		if (!element.hasPointerCapture(pointerId)) throw Error('second pointer replaced capture');
		// Queue movement and cancel within one JS task, before its RAF can submit a write.
		element.dispatchEvent(
			new PointerEvent('pointermove', {
				pointerId,
				clientX: rect.x + 100,
				clientY: rect.y + 100,
				buttons: 1,
				bubbles: true
			})
		);
		element.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
		if (element.hasPointerCapture(pointerId)) throw Error('Escape retained capture');
	});
	await settle();
	assert.equal(await writes(), initial, 'Escape discards queued input');
	assert.equal(await selected.textContent(), 'Curve A key 0', 'first Escape cancels the tool');
	await page.mouse.up();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'Curve A', 'next Escape inspects owner');
	assert.equal(await writes(), initial, 'late release cannot submit canceled movement');

	for (const boundary of ['pointercancel', 'lostcapture', 'blur', 'windowblur']) {
		await beginKey();
		await canvas.evaluate((element, boundary) => {
			const pointerId = Number(element.dataset.inputPointer);
			if (boundary === 'pointercancel')
				element.dispatchEvent(new PointerEvent('pointercancel', { pointerId }));
			if (boundary === 'lostcapture') element.releasePointerCapture(pointerId);
			if (boundary === 'blur') element.blur();
			if (boundary === 'windowblur') window.dispatchEvent(new Event('blur'));
		}, boundary);
		await page.mouse.move(points[0].x + 15, points[0].y - 15);
		await page.mouse.up();
		await settle();
		assert.equal(await writes(), initial, `${boundary} cannot commit after cancellation`);
	}
	await beginKey();
	await page
		.getByRole('button', { name: 'Replace Curve snapshot', exact: true })
		.dispatchEvent('click');
	await settle();
	assert.equal(await selected.textContent(), 'Curve A');
	assert(
		!(await canvas.evaluate((element) =>
			element.hasPointerCapture(Number(element.dataset.inputPointer))
		)),
		'snapshot/owner replacement retires capture'
	);
	await page.mouse.move(points[0].x + 5, points[0].y - 5);
	await page.mouse.up();
	await settle();
	assert.equal(await writes(), initial, 'replaced scope cannot receive old input');
	await beginKey();
	await page.mouse.move(points[0].x + 10, points[0].y - 10);
	await settle();
	const firstWrite = await writes();
	assert(firstWrite > initial, 'normal drag submits live typed Parameter changes');
	assert(
		await canvas.evaluate((element) =>
			element.hasPointerCapture(Number(element.dataset.inputPointer))
		),
		'live graph updates do not cancel the gesture'
	);
	await page.mouse.move(points[0].x + 20, points[0].y - 20);
	await settle();
	assert(
		(await writes()) > firstWrite,
		'one captured gesture continues across live graph publications'
	);
	await page.mouse.up();
	await settle();
	await key.click();
	const beforeDouble = Number(await page.getByLabel('Curve removal intents').textContent());
	await page.mouse.dblclick(points[0].x + 20, points[0].y - 20, { delay: 40 });
	await settle();
	assert.equal(
		Number(await page.getByLabel('Curve removal intents').textContent()),
		beforeDouble + 1,
		'key double activation retains its existing removal intent'
	);
}
