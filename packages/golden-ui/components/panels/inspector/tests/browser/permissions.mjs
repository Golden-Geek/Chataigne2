import assert from 'node:assert/strict';

export async function checkCurvePermissions(page, canvas, selected, points) {
	const primary = page.getByLabel('Primary curve Inspector', { exact: true });
	const writes = () => page.getByLabel('Curve writes').textContent().then(Number);
	const edits = () => page.getByLabel('Curve edits').textContent().then(Number);
	const settle = () =>
		page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
	const choose = async (mode) => {
		const revision = Number(await page.getByLabel('Curve access revision').textContent());
		await page.getByRole('button', { name: `Curve access ${mode}`, exact: true }).click();
		await page.waitForFunction(
			({ mode, revision }) =>
				document.querySelector('[aria-label="Curve access mode"]').textContent === mode &&
				Number(document.querySelector('[aria-label="Curve access revision"]').textContent) >
					revision,
			{ mode, revision }
		);
		await settle();
	};
	const selectKey = () =>
		page.getByRole('button', { name: 'Select Curve A key 0', exact: true }).click();
	await canvas.evaluate((element) =>
		element.addEventListener(
			'pointerdown',
			(event) => {
				if (event.isTrusted) element.dataset.accessPointer = String(event.pointerId);
			},
			{ capture: true }
		)
	);
	const captured = () =>
		canvas.evaluate((element) => element.hasPointerCapture(Number(element.dataset.accessPointer)));
	const pressKey = async () => {
		await page.mouse.move(points[0].x, points[0].y);
		await page.mouse.down();
	};
	for (const mode of ['readonly', 'disabled', 'controlled']) {
		await choose(mode);
		const beforeEdits = await edits(),
			beforeWrites = await writes();
		await pressKey();
		assert.equal(
			await selected.textContent(),
			'Curve A key 0',
			`${mode} key selects before release`
		);
		assert.equal(await captured(), false, `${mode} key refuses edit capture`);
		const fields = await primary
			.locator('.selected-key-node-inspector input.number-field')
			.evaluateAll((inputs) => inputs.map((input) => input.readOnly || input.disabled));
		assert(
			fields.length >= 2 && fields.every(Boolean),
			`${mode} agrees with the actual typed Inspector`
		);
		await page.mouse.move(points[0].x + 20, points[0].y - 20);
		await page.mouse.up();
		const middle = { x: (points[0].x + points[1].x) / 2, y: (points[0].y + points[1].y) / 2 };
		await page.mouse.move(middle.x, middle.y);
		await page.mouse.down();
		assert.equal(await selected.textContent(), 'Easing', `${mode} segment stays selectable`);
		assert.equal(await captured(), false, `${mode} sculpt refuses capture`);
		await page.mouse.up();
		await settle();
		assert.equal(await writes(), beforeWrites);
		assert.equal(await edits(), beforeEdits, `${mode} refuses before opening a history group`);
	}
	await choose('mixed');
	await page.getByRole('button', { name: 'Select both keys', exact: true }).click();
	const mixedWrites = await writes();
	await pressKey();
	assert.equal(await captured(), false, 'one read-only axis refuses the entire multi-key gesture');
	assert.equal((await selected.textContent()).split('|').length, 2);
	await page.mouse.move(points[0].x + 10, points[0].y - 10);
	await page.mouse.up();
	assert.equal(await writes(), mixedWrites);

	for (const boundary of ['Disable active Curve value', 'Control active Curve value']) {
		await choose('manual');
		await pressKey();
		assert(await captured());
		const before = await writes();
		await canvas.evaluate((element, boundary) => {
			const pointerId = Number(element.dataset.accessPointer),
				rect = element.getBoundingClientRect();
			element.dispatchEvent(
				new PointerEvent('pointermove', {
					pointerId,
					buttons: 1,
					bubbles: true,
					clientX: rect.x + 150,
					clientY: rect.y + 100
				})
			);
			[...document.querySelectorAll('button')]
				.find((button) => button.textContent === boundary)
				.click();
		}, boundary);
		await settle();
		assert.equal(await captured(), false, `${boundary} retires active capture`);
		await page.mouse.up();
		await settle();
		assert.equal(await writes(), before, `${boundary} drops unsent input`);
	}
	await choose('protected');
	const removals = Number(await page.getByLabel('Curve removal intents').textContent());
	await page.mouse.dblclick(points[0].x, points[0].y, { delay: 40 });
	await settle();
	assert.equal(
		Number(await page.getByLabel('Curve removal intents').textContent()),
		removals,
		'double activation respects document remove permission'
	);
	await choose('manual');
	await selectKey();
	const selectedPixels = () =>
		canvas.evaluate((element) => {
			const data = element.getContext('2d').getImageData(0, 0, element.width, element.height).data;
			let count = 0;
			for (let i = 0; i < data.length; i += 4)
				if (data[i] === 241 && data[i + 1] === 0 && data[i + 2] === 241) count++;
			return count;
		});
	await settle();
	assert((await selectedPixels()) > 0);
	await page.mouse.move(points[0].x, points[0].y);
	await page.mouse.wheel(4000, 0);
	await settle();
	assert.equal(await selectedPixels(), 0, 'selected key is outside the canvas view');
	assert.equal(await selected.textContent(), 'Curve A key 0');
	const field = primary.locator('.selected-key-node-inspector input.number-field').first();
	const beforeField = await writes();
	await field.fill('0.25');
	await field.press('Enter');
	await settle();
	assert(
		(await writes()) > beforeField,
		'offscreen selected key remains editable through the actual Inspector'
	);
	assert.equal(await selected.textContent(), 'Curve A key 0');
	await choose('manual');
}
