import assert from 'node:assert/strict';

export async function qualifyNodeMenu(page, emit) {
	await page.evaluate(() => window.collectionFixture.setupNodeMenu());
	const consoleErrors = [];
	page.on('console', (message) => {
		if (message.type() === 'error') consoleErrors.push(message.text());
	});
	const committed = async (count) => {
		try {
			await page.waitForFunction(
				(count) => window.collectionFixture.nodeMenuIntents().length === count,
				count,
				{ timeout: 5000 }
			);
		} catch (error) {
			emit({
				kind: 'node-menu-failure',
				expected: count,
				intents: await page.evaluate(() => window.collectionFixture.nodeMenuIntents()),
				consoleErrors,
				body: await page.locator('body').innerText()
			});
			throw error;
		}
	};
	const menu = (name) => page.getByRole('menuitem', { name, exact: true });
	const open = async (label) => {
		await page.getByRole('button', { name: label, exact: true }).click({ button: 'right' });
		await menu('Set Constraints').hover();
	};
	await open('Numeric');
	await menu('Range').hover();
	await menu('Custom Range...').click();
	const panel = page.locator('.gc-node-context-side-panel');
	await panel.getByLabel('Min', { exact: true }).fill('-2');
	await panel.getByLabel('Max', { exact: true }).fill('8');
	await panel.getByRole('button', { name: 'Apply', exact: true }).click();
	await committed(1);
	const range = await page.evaluate(() => window.collectionFixture.nodeMenuIntents()[0]);
	assert.equal(range.kind, 'setParamConstraints');
	assert.equal(range.node, 2);
	assert.deepEqual(range.constraints.range, { kind: 'uniform', min: -2, max: 8 });
	await panel.waitFor({ state: 'detached' });
	await open('Numeric');
	await menu('Step').hover();
	await menu('0.1').click();
	await committed(2);
	assert.equal(
		(await page.evaluate(() => window.collectionFixture.nodeMenuIntents()[1])).constraints.step,
		0.1
	);
	await page.keyboard.press('Escape');
	await open('Choice');
	await menu('Set Options...').click();
	await panel.locator('textarea').fill('First=first\nSecond=second');
	await panel.getByRole('button', { name: 'Apply', exact: true }).click();
	await committed(3);
	const options = await page.evaluate(() => window.collectionFixture.nodeMenuIntents()[2]);
	assert.equal(options.node, 3);
	assert.deepEqual(
		options.constraints.enum_options.map(({ label, variant_id }) => ({ label, variant_id })),
		[
			{ label: 'First', variant_id: 'first' },
			{ label: 'Second', variant_id: 'second' }
		]
	);
	await panel.waitFor({ state: 'detached' });
	await open('Numeric');
	await menu('Range').hover();
	await menu('Custom Range...').click();
	await panel.waitFor();
	assert.equal(await panel.getByLabel('Min', { exact: true }).inputValue(), '-2');
	assert.equal(await panel.getByLabel('Max', { exact: true }).inputValue(), '8');
	await panel.getByLabel('Min', { exact: true }).fill('');
	await panel.getByRole('button', { name: 'Apply', exact: true }).click();
	await committed(4);
	assert.deepEqual(
		(await page.evaluate(() => window.collectionFixture.nodeMenuIntents()[3])).constraints.range,
		{ kind: 'uniform', min: undefined, max: 8 }
	);
	await open('Numeric');
	await menu('Range').hover();
	await menu('Custom Range...').click();
	await panel.waitFor();
	await page.keyboard.press('Escape');
	await panel.waitFor({ state: 'detached' });
	assert.equal((await page.evaluate(() => window.collectionFixture.nodeMenuIntents())).length, 4);
	assert.deepEqual(consoleErrors, []);
	emit({
		kind: 'node-menu',
		passed: true,
		scope:
			'numeric range/step and enum constraints, side-panel input, close/reset and intent parity'
	});
}
