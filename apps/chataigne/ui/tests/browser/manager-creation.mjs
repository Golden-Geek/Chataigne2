import assert from 'node:assert/strict';

export async function qualifyManagerCreation(page, emit) {
	const menu = (name) => page.getByRole('menuitem', { name, exact: true });
	async function choose() {
		await menu('Fixture').hover();
		await menu('Nested').hover();
		await menu('Create item').click();
	}
	for (const kind of ['modules', 'formulas', 'processors', 'properties']) {
		await page.evaluate((kind) => window.appTrees.setupCreation(kind), kind);
		const manager = page.locator('.manager-list-panel');
		await manager
			.locator('.manager-list-header')
			.getByRole('button', { name: 'Add child item' })
			.click();
		await choose();
		await page.waitForFunction(() => window.appTrees.selection().includes(2000));
		assert.equal(
			await manager.locator('[data-tree-node-id="2000"]').getAttribute('aria-selected'),
			'true'
		);
		assert.equal(
			await manager.locator('[data-tree-node-id="2000"]').getAttribute('aria-expanded'),
			null
		);
		await page.evaluate(() => window.appTrees.focusNode(2));
		await manager
			.locator('[data-tree-node-id="2"] .outliner-item-label')
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: /^Add\b/ }).hover();
		await choose();
		await page.waitForFunction(() => window.appTrees.selection().includes(2001));
		await manager.locator('input[type="search"]').fill('does-not-match-new-item');
		await manager
			.locator('.manager-list-header')
			.getByRole('button', { name: 'Add child item' })
			.click();
		await choose();
		await page.waitForFunction(() => window.appTrees.selection().includes(2002));
		assert.equal(await manager.locator('[data-tree-node-id="2002"]').count(), 0);
		await manager.locator('input[type="search"]').fill('');
		await page.waitForFunction(() => document.querySelector('[data-tree-node-id="2002"]') !== null);
		assert.deepEqual(await page.evaluate(() => window.appTrees.creations()), [2000, 2001, 2002]);
		const mounted = Number(
			await manager.locator('.virtual-rows').getAttribute('data-mounted-row-count')
		);
		assert(mounted > 0 && mounted <= 42, String(mounted));
		emit({
			kind: 'manager-creation',
			consumer: kind,
			paths: ['nested header Add', 'folder context Add', 'filtered header Add'],
			concrete_receipts: 3,
			mounted,
			passed: true
		});
	}
}
