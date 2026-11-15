import assert from 'node:assert/strict';

export async function checkCurveOperations(page, canvas) {
	const settle = () =>
		page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	await page.getByRole('button', { name: 'Hold Curve Begin', exact: true }).click();
	const before = await page.getByLabel('Curve fit owners').textContent();
	const box = await canvas.boundingBox();
	await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.85);
	await page.keyboard.down('Control');
	await page.keyboard.down('Shift');
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.8);
	await page.mouse.up();
	await page.keyboard.up('Shift');
	await page.keyboard.up('Control');
	await page.waitForFunction(
		() => Number(document.querySelector('[aria-label="Curve pending begins"]').textContent) > 0
	);
	await page.getByRole('button', { name: 'Select Curve B', exact: true }).click();
	await page.getByRole('button', { name: 'Release Curve Begin', exact: true }).click();
	await settle();
	assert.equal(
		await page.getByLabel('Curve fit owners').textContent(),
		before,
		'pending draw cannot fit the replacement Curve owner'
	);
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	await drawCurve(page, canvas);
	await settle();
	assert.equal(
		await page.getByLabel('Curve fit owners').textContent(),
		before ? `${before}|Curve A` : 'Curve A',
		'in-scope drawing still targets its original owner'
	);
	await page.getByRole('button', { name: 'Hold Curve Begin', exact: true }).click();
	const beforeSnapshot = await page.getByLabel('Curve fit owners').textContent();
	await drawCurve(page, canvas);
	await pendingBegin(page);
	await page.getByRole('button', { name: 'Replace Curve snapshot', exact: true }).click();
	await page.getByRole('button', { name: 'Release Curve Begin', exact: true }).click();
	await settle();
	assert.equal(
		await page.getByLabel('Curve fit owners').textContent(),
		beforeSnapshot,
		'snapshot replacement cannot revive pending drawing'
	);

	await page.getByRole('button', { name: 'Select both keys', exact: true }).click();
	await page.getByRole('button', { name: 'Copy Curve keys', exact: true }).click();
	await page.getByRole('button', { name: 'Hold Curve Begin', exact: true }).click();
	const beforePaste = await page.getByLabel('Curve create owners').textContent();
	await page.getByRole('button', { name: 'Paste Curve keys', exact: true }).click();
	await pendingBegin(page);
	await page.getByRole('button', { name: 'Select Curve B', exact: true }).click();
	await page.getByRole('button', { name: 'Release Curve Begin', exact: true }).click();
	await settle();
	assert.equal(
		await page.getByLabel('Curve create owners').textContent(),
		beforePaste,
		'queued clipboard creation cannot move to the replacement owner'
	);
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	await page.getByRole('button', { name: 'Paste Curve keys', exact: true }).click();
	await page.waitForFunction(
		() => document.querySelector('[aria-label="Curve command result"]').textContent !== 'pending'
	);
	assert.equal(
		await page.getByLabel('Curve command result').textContent(),
		'true',
		'valid multi-key paste completes'
	);
	assert.equal(
		(await page.getByLabel('Curve create owners').textContent()).split('|').slice(-2).join('|'),
		'Curve A|Curve A'
	);
	const id = await page.getByLabel('Curve last created key').textContent();
	assert.equal(
		await page.getByLabel('Selected curve nodes').textContent(),
		`Created ${id}`,
		'paste selects its final exact receipt'
	);

	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	await page.getByRole('button', { name: 'Hold Curve Create', exact: true }).click();
	const current = await canvas.boundingBox();
	await page.mouse.move(current.x + current.width * 0.7, current.y + current.height * 0.9);
	await page.keyboard.down('Control');
	await page.mouse.click(current.x + current.width * 0.7, current.y + current.height * 0.9);
	await page.keyboard.up('Control');
	await page.waitForFunction(
		() => Number(document.querySelector('[aria-label="Curve pending creates"]').textContent) > 0
	);
	await page.getByRole('button', { name: 'Select Curve B', exact: true }).click();
	await page.getByRole('button', { name: 'Release Curve Create', exact: true }).click();
	await settle();
	assert.equal(
		await page.getByLabel('Selected curve nodes').textContent(),
		'Curve B',
		'late creation cannot steal newer owner selection'
	);
	assert(
		Number(await page.getByLabel('Curve created position').textContent()) > 0,
		'accepted creation initializes its exact original-owner key after view replacement'
	);
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
}

async function pendingBegin(page) {
	await page.waitForFunction(
		() => Number(document.querySelector('[aria-label="Curve pending begins"]').textContent) > 0
	);
}
async function drawCurve(page, canvas) {
	const box = await canvas.boundingBox();
	await page.mouse.move(box.x + box.width * 0.55, box.y + box.height * 0.94);
	await page.keyboard.down('Control');
	await page.keyboard.down('Shift');
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * 0.65, box.y + box.height * 0.9);
	await page.mouse.up();
	await page.keyboard.up('Shift');
	await page.keyboard.up('Control');
}
export async function checkCurveDisposal(page, canvas) {
	await page.getByRole('button', { name: 'Hold Curve Begin', exact: true }).click();
	const before = await page.getByLabel('Curve fit owners').textContent();
	await drawCurve(page, canvas);
	await pendingBegin(page);
	await page.getByRole('button', { name: 'Hide Curve editors', exact: true }).click();
	await page.getByRole('button', { name: 'Release Curve Begin', exact: true }).click();
	await page.evaluate(
		() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
	);
	assert.equal(
		await page.getByLabel('Curve fit owners').textContent(),
		before,
		'disposal retires delayed drawing'
	);
}
