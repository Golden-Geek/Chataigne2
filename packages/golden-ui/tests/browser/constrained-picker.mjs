import assert from 'node:assert/strict';

export async function qualifyConstrainedPicker(page, emit) {
	const command = (name, value = 0) =>
		page.evaluate(
			([name, value]) => window.collectionFixture.constrainedCommand(name, value),
			[name, value]
		);
	const open = (id = 3, mode = 'multiple') =>
		page.evaluate(([id, mode]) => window.collectionFixture.openConstrained(id, mode), [id, mode]);
	const outcome = (slot) =>
		page.evaluate((slot) => window.collectionFixture.pickerResults()[slot], slot);
	const closed = () => page.waitForSelector('.node-picker-modal', { state: 'detached' });
	const row = (id) => page.locator(`.picker-tree [data-tree-node-id="${id}"]`);
	const bounds = async () => {
		const values = await page
			.locator('.virtual-rows')
			.evaluateAll((nodes) => nodes.map((n) => Number(n.dataset.mountedRowCount)));
		assert(
			values.every((n) => n <= 42),
			`Picker collection bounds: ${values}`
		);
	};
	for (const size of [1000, 10000, 20000, 50000, 100000]) {
		await page.evaluate((size) => window.collectionFixture.setupConstrained(size), size);
		const origin = await page.locator('.manager-list-search').elementHandle();
		assert(origin);
		try {
			const first = await open();
			assert.equal(await row(1).count(), 0);
			assert.equal(await row(3).getAttribute('aria-disabled'), 'true');
			for (const id of [0, 2, 3]) assert.equal(await command('activate', id), false);
			assert.equal(await command('confirm'), false);
			await page.evaluate(() => window.collectionFixture.treeFocus('picker', 2));
			await page.evaluate(() => window.collectionFixture.treeNavigate('picker', 'ArrowRight'));
			assert.equal(await row(size + 2).count(), 1);
			await page.evaluate(() => window.collectionFixture.treeNavigate('picker', 'Enter'));
			assert.equal(await outcome(first), null);
			await command('even');
			assert.equal(await row(2).count(), 0);
			assert.equal(await row(size + 2).getAttribute('aria-level'), '1');
			await command('all');
			await page.evaluate(() => window.collectionFixture.treeNavigate('picker', 'End'));
			assert.equal(
				await page.evaluate(() =>
					Number(document.activeElement?.getAttribute('data-tree-node-id'))
				),
				size + 1
			);
			assert(
				(await row(size + 1).textContent()).includes(`Item ${String(size - 1).padStart(6, '0')}`)
			);
			await bounds();
			assert.equal(await command('activate', 4), true);
			assert.equal(await command('confirm'), false);
			assert.equal(
				await command('double', 4),
				false,
				'Multiple projections require an explicit choice'
			);
			assert.equal(await command('auto'), false);
			assert.equal(await command('y'), true);
			await command('even');
			assert.equal(await command('confirm'), true);
			assert.deepEqual(await outcome(first), {
				kind: 'pick',
				id: 4,
				uuid: (await outcome(first)).uuid,
				projection: 'vec2Y'
			});
			await closed();
			assert.equal(await origin.evaluate((element) => element === document.activeElement), true);

			const excluded = await open(5, 'single');
			await command('even');
			assert.equal(await command('confirm'), false);
			await command('none');
			assert.equal(await page.locator('.picker-tree [role="treeitem"]').count(), 0);
			assert.equal(await command('confirm'), false);
			await command('all');
			assert.equal(await command('confirm'), true);
			assert.equal((await outcome(excluded)).projection, 'vec2X');
			await closed();

			const optional = await open(4, 'optional');
			assert.equal(await command('auto'), true);
			assert.equal(await command('confirm'), true);
			assert.equal((await outcome(optional)).projection, undefined);
			await closed();
			const instant = await open(3, 'instant');
			await page.evaluate(() => window.collectionFixture.treeFocus('picker', 4));
			await page.evaluate(() => window.collectionFixture.treeNavigate('picker', 'Enter'));
			assert.equal((await outcome(instant)).id, 4);
			await closed();

			const invalid = await open(4, 'invalid');
			assert.equal(await command('confirm'), false);
			await command('y');
			assert.equal(await command('confirm'), true);
			assert.equal((await outcome(invalid)).projection, 'vec2Y');
			await closed();
			const clearing = await open(4);
			assert.equal(await command('clear'), true);
			assert.deepEqual(await outcome(clearing), { kind: 'clear' });
			await closed();

			const blocked = await open(4, 'optional');
			await command('block');
			assert.equal(await command('confirm'), false);
			await page.evaluate(() => window.collectionFixture.treeFocus('picker', 4));
			assert.equal(await row(4).getAttribute('aria-disabled'), 'true');
			await command('close');
			assert.deepEqual(await outcome(blocked), { kind: 'cancel' });
			await closed();

			const replaced = await open(4);
			await command('x');
			await command('replace-node', 4);
			assert.equal(await command('confirm'), false);
			assert.equal(await command('activate', 4), true);
			await command('y');
			assert.equal(await command('confirm'), true);
			assert((await outcome(replaced)).uuid.endsWith('-replaced'));
			await closed();

			const slots = await page.evaluate(() =>
				Promise.all([
					window.collectionFixture.openConstrained(4),
					window.collectionFixture.openConstrained(5, 'single')
				])
			);
			assert.deepEqual(await outcome(slots[0]), { kind: 'cancel' });
			assert.equal(await command('confirm'), true);
			assert.equal((await outcome(slots[1])).id, 5);
			await closed();

			const focusSlot = await open(4);
			await page.locator('.node-picker-modal .close-button').evaluate((element) => element.focus());
			assert.equal(await command('previous'), true);
			assert.equal(await page.evaluate(() => document.activeElement?.textContent?.trim()), 'Clear');
			assert.equal(await command('next'), true);
			assert.equal(
				await page.evaluate(() => document.activeElement?.classList.contains('close-button')),
				true
			);
			await command('replace-root');
			assert.deepEqual(await outcome(focusSlot), { kind: 'cancel' });
			await closed();
			assert.equal(await origin.evaluate((element) => element === document.activeElement), true);
			await bounds();
			emit({ kind: 'constrained-picker', size, passed: true, groups: 11 });
		} finally {
			await origin.dispose();
		}
	}
}
