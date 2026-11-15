import assert from 'node:assert/strict';

export async function checkGradient(page, url) {
	await page.goto(`${url}/?gradient`);
	const primary = page.getByLabel('Primary Gradient Inspector', { exact: true });
	const editor = primary.getByRole('application', { name: 'Gradient editor', exact: true });
	const selected = page.getByLabel('Selected Gradient nodes');
	const count = async (label) => Number(await page.getByLabel(`Gradient ${label}`).textContent());
	const waitSelected = (value) =>
		page.waitForFunction(
			(value) =>
				document.querySelector('[aria-label="Selected Gradient nodes"]')?.textContent === value,
			value
		);
	await editor.waitFor();
	await waitSelected('Gradient A');
	assert.equal(
		await editor.locator('.gradient-stop-handle.selected').count(),
		0,
		'owner has no implicit first stop'
	);
	const marker = editor.locator('.gradient-stop-handle[data-node-id="2"]');
	const idleOutline = await marker.evaluate((element) => getComputedStyle(element).boxShadow);
	await marker.hover();
	assert.notEqual(
		await marker.evaluate((element) => getComputedStyle(element).boxShadow),
		idleOutline,
		'hover changes only the unselected contour'
	);
	await waitSelected('Gradient A');
	assert.equal(await editor.locator('.gradient-stop-handle.selected').count(), 0);
	await marker.focus();
	await page.keyboard.press('Enter');
	await waitSelected('Gradient A stop 0');
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
	const original = await editor.elementHandle();
	const down = async () => {
		const stop = editor.locator('.gradient-stop-handle[data-node-id="2"]');
		await stop.scrollIntoViewIfNeeded();
		const rect = await stop.boundingBox();
		await editor.evaluate((element) =>
			element.addEventListener(
				'pointerdown',
				(event) => {
					element.dataset.gradientPointer = String(event.pointerId);
				},
				{ once: true, capture: true }
			)
		);
		await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
		await page.mouse.down();
		try {
			await waitSelected('Gradient A stop 0');
		} catch (error) {
			throw new Error(`Gradient down selected ${await selected.textContent()}: ${error}`);
		}
		return editor.locator('.gradient-track').boundingBox();
	};
	const armed = async () => {
		const track = await down();
		await page.mouse.move(track.x + track.width * 0.35, track.y + track.height);
		await page.mouse.move(track.x + track.width * 0.35, track.y + track.height + 40);
		await editor.locator('.gradient-stop-handle.removing').waitFor();
	};
	const closed = () =>
		page.waitForFunction(
			() =>
				document.querySelector('[aria-label="Gradient begins"]')?.textContent ===
				document.querySelector('[aria-label="Gradient ends"]')?.textContent
		);
	await down();
	assert(
		await editor.evaluate((element, previous) => element === previous, original),
		'stop selection retains its owner editor'
	);
	assert(
		await editor.evaluate((element) =>
			element.hasPointerCapture(Number(element.dataset.gradientPointer))
		),
		'primary-down captures the stable editor'
	);
	await editor.evaluate((element) =>
		element.dispatchEvent(
			new PointerEvent('pointercancel', {
				pointerId: Number(element.dataset.gradientPointer) + 1,
				bubbles: true
			})
		)
	);
	assert(
		await editor.evaluate((element) =>
			element.hasPointerCapture(Number(element.dataset.gradientPointer))
		),
		'foreign pointer cancellation cannot retire the owned drag'
	);
	assert.equal(
		await primary.locator('.gradient-stop-editor .node-inspector[data-node-id="2"]').count(),
		1
	);
	await page.mouse.up();
	assert.equal(await count('writes'), 0, 'selection alone cannot edit authored values');
	await editor.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
	await marker.dblclick();
	await page.getByRole('dialog', { name: 'Stop color', exact: true }).waitFor();
	const beforeColor = await count('writes'),
		beforeColorBegin = await count('begins');
	await page
		.getByRole('dialog', { name: 'Stop color', exact: true })
		.locator('.cp-area')
		.click({ position: { x: 15, y: 15 } });
	await page.waitForFunction(
		(before) =>
			Number(document.querySelector('[aria-label="Gradient writes"]')?.textContent) > before,
		beforeColor
	);
	await closed();
	assert.equal(
		await count('begins'),
		beforeColorBegin + 1,
		'the first color sample owns its edit group before publication'
	);
	await page.keyboard.press('Escape');
	await page
		.getByRole('dialog', { name: 'Stop color', exact: true })
		.waitFor({ state: 'detached' });
	await waitSelected('Gradient A stop 0');
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
	await editor.locator('.gradient-track').click({ position: { x: 10, y: 5 } });
	await waitSelected('Gradient A');
	await page.getByRole('button', { name: 'Hold Gradient begin', exact: true }).click();
	const beforeHeldBegin = await count('begins');
	await armed();
	await page.waitForFunction(
		(before) =>
			Number(document.querySelector('[aria-label="Gradient begins"]')?.textContent) > before,
		beforeHeldBegin
	);
	await page.keyboard.press('Escape');
	await page.mouse.up();
	const blocked = await down();
	await page.mouse.move(blocked.x + blocked.width * 0.4, blocked.y + blocked.height);
	await page.mouse.up();
	assert.equal(
		await count('begins'),
		beforeHeldBegin + 1,
		'pending closure cannot admit another history owner'
	);
	await page
		.getByRole('button', { name: 'Release Gradient begin', exact: true })
		.evaluate((button) => button.click());
	await closed();
	assert.equal(await count('removals'), 0, 'cancellation while Begin is held cannot delete a stop');
	for (const boundary of ['pointercancel', 'Escape', 'capture loss', 'blur', 'document']) {
		const removed = await count('removals');
		await armed();
		if (boundary === 'pointercancel')
			await editor.evaluate((element) =>
				element.dispatchEvent(
					new PointerEvent('pointercancel', {
						pointerId: Number(element.dataset.gradientPointer),
						bubbles: true
					})
				)
			);
		if (boundary === 'Escape') await page.keyboard.press('Escape');
		if (boundary === 'capture loss') {
			await editor.evaluate((element) =>
				element.releasePointerCapture(Number(element.dataset.gradientPointer))
			);
			await page.mouse.move(5, 5);
		}
		if (boundary === 'blur') await page.evaluate(() => window.dispatchEvent(new Event('blur')));
		if (boundary === 'document') {
			const before = await count('generation');
			await page
				.getByRole('button', { name: 'Replace Gradient document', exact: true })
				.evaluate((button) => button.click());
			await page.waitForFunction(
				(before) =>
					Number(document.querySelector('[aria-label="Gradient generation"]')?.textContent) >
					before,
				before
			);
		}
		await page.mouse.up();
		await closed();
		assert.equal(await count('removals'), removed, `${boundary} cannot complete an armed deletion`);
		if (boundary !== 'document') await waitSelected('Gradient A stop 0');
		assert.equal(await editor.locator('.gradient-stop-handle.removing').count(), 0);
	}
	await page.getByRole('button', { name: 'Select Gradient position', exact: true }).click();
	await waitSelected('Position');
	assert(
		await editor.evaluate((element, previous) => element === previous, original),
		'field selection retains editor'
	);
	const field = primary
		.locator('.gradient-stop-editor .node-inspector[data-node-id="3"] input')
		.first();
	await field.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Position');
	await editor.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
	await armed();
	const removed = await count('removals');
	await page.mouse.up();
	await closed();
	assert.equal(
		await count('removals'),
		removed + 1,
		'normal release retains the explicit delete gesture'
	);
	await armed();
	const beforeRevocation = await count('removals');
	await page
		.getByRole('button', { name: 'Disable Gradient position', exact: true })
		.evaluate((button) => button.click());
	await page.mouse.up();
	await closed();
	assert.equal(
		await count('removals'),
		beforeRevocation,
		'access revocation cancels an armed deletion'
	);
	const writes = await count('writes'),
		begins = await count('begins');
	const track = await down();
	await page.mouse.move(track.x + track.width * 0.4, track.y + track.height);
	await page.mouse.up();
	assert.equal(await count('writes'), writes, 'disabled Position stays selectable without edits');
	assert.equal(await count('begins'), begins, 'disabled Position cannot open history');
	await page.getByRole('button', { name: 'Replace Gradient document', exact: true }).click();
	await waitSelected('Gradient A');
	await armed();
	const beforeDispose = await count('removals');
	await page
		.getByRole('button', { name: 'Hide Gradient editors', exact: true })
		.evaluate((button) => button.click());
	await page.mouse.up();
	await closed();
	assert.equal(await count('removals'), beforeDispose, 'disposal cannot delete an armed stop');
	await page.getByRole('button', { name: 'Show Gradient editors', exact: true }).click();
	await page.getByRole('button', { name: 'Open locked Gradient editors', exact: true }).click();
	await page.getByRole('button', { name: 'Select Gradient stop B', exact: true }).click();
	const other = page.getByLabel('Other Gradient Inspector').getByRole('application');
	await other.locator('.gradient-stop-handle.selected').waitFor();
	await editor.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
	assert.equal(
		await other.locator('.gradient-stop-handle.selected').count(),
		1,
		'independent locked editor retains its stop'
	);
	await other.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Gradient B');
	await marker.dblclick();
	const popup = primary.getByRole('dialog', { name: 'Stop color', exact: true });
	await popup.waitFor();
	await other.focus();
	await page.keyboard.press('Escape');
	await waitSelected('Gradient B');
	assert(await popup.isVisible(), 'another editor cannot consume the local color tool Escape');
	await editor.focus();
	await page.keyboard.press('Escape');
	await popup.waitFor({ state: 'detached' });
	await waitSelected('Gradient B');
	await page.keyboard.press('Escape');
	await waitSelected('Gradient A');
}
