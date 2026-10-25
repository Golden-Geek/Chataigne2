import assert from 'node:assert/strict';

export async function qualifyCatalog(page, emit) {
	const counts = () =>
		page.locator('.catalog-list .virtual-rows').evaluateAll((nodes) =>
			nodes.map((node) => ({
				rows: Number(node.dataset.rowCount),
				mounted: Number(node.dataset.mountedRowCount)
			}))
		);
	const id = (index) => `create:FixtureItem/${index}`;
	const button = (index) => page.locator(`[data-menu-item-id="${id(index)}"]`);
	for (const count of [1000, 10000, 20000, 50000, 100000]) {
		await page.evaluate((count) => window.collectionFixture.setupCatalog(count), count);
		assert.deepEqual(
			(await counts()).map((list) => list.rows),
			[count + 1]
		);
		assert((await counts()).every((list) => list.mounted > 0 && list.mounted <= 42));
		await page.evaluate(() => window.collectionFixture.catalogWidth('14rem'));
		assert.equal(
			await page
				.locator('.catalog-menu')
				.evaluate(
					(menu) =>
						menu.getBoundingClientRect().width <=
						14 * parseFloat(getComputedStyle(document.documentElement).fontSize) + 0.1
				),
			true
		);
		await page.evaluate(() => window.collectionFixture.catalogWidth('max-content'));
		await page.evaluate((index) => window.collectionFixture.catalogReveal(index), count);
		assert.equal(await button(count - 1).count(), 1);
		assert.equal(
			(await button(count - 1).textContent()).includes(
				`Catalog ${String(count - 1).padStart(6, '0')}`
			),
			true
		);
		const middle = Math.floor(count / 2);
		await page.evaluate((index) => window.collectionFixture.catalogReveal(index, true), middle + 1);
		const focused = await button(middle).elementHandle();
		assert(focused);
		await page.evaluate((index) => window.collectionFixture.catalogReveal(index), count);
		assert.equal(await focused.evaluate((node) => node === document.activeElement), true);
		await page.evaluate(() => window.collectionFixture.catalogReverse());
		assert.equal(
			await focused.evaluate((node) => node.isConnected && node === document.activeElement),
			true
		);
		assert((await counts()).every((list) => list.mounted <= 42));
		await page.evaluate(() => window.collectionFixture.catalogSearch('no-match'));
		assert.equal(await page.locator('.catalog-list .gc-context-item').count(), 0);
		assert.equal(
			await page.evaluate(() => document.activeElement?.classList.contains('catalog-list')),
			true
		);
		await focused.dispose();
		await page.evaluate((count) => window.collectionFixture.setupCatalog(count), count);
		await page.evaluate(() => window.collectionFixture.catalogNested());
		assert.equal((await counts()).length, 2);
		await page.evaluate(
			(index) => window.collectionFixture.catalogReveal(index, true, 1),
			count - 1
		);
		assert.equal(
			await page
				.locator('.gc-context-submenu [data-menu-item-id]')
				.filter({ hasText: `Catalog ${String(count - 1).padStart(6, '0')}` })
				.count(),
			1
		);
		assert.equal(await page.locator('.gc-context-submenu.is-positioning').count(), 0);
		assert.equal(
			await page.locator('.gc-context-submenu').evaluate((menu) => {
				const row = menu.querySelector('[data-menu-item-id]:focus');
				const list = menu.querySelector('.catalog-list');
				if (!row || !list) return false;
				const rect = row.getBoundingClientRect(),
					viewport = list.getBoundingClientRect();
				return (
					rect.width > 0 &&
					rect.bottom > viewport.top &&
					rect.top < viewport.bottom &&
					viewport.left >= 0 &&
					viewport.right <= window.innerWidth
				);
			}),
			true
		);
		assert((await counts()).every((list) => list.mounted <= 42));
		await page.evaluate(() => window.collectionFixture.catalogSearch('Catalog 000005', 1));
		assert.equal(await page.locator('.gc-context-submenu .gc-context-item').count(), 1);
		assert.equal(
			await page.locator('.gc-context-submenu .gc-context-item').getAttribute('data-menu-item-id'),
			id(5)
		);
		// A scrolled-away pointer submenu parent must retire its child layer.
		await page.evaluate(() => window.collectionFixture.catalogSearch('Catalog 000010', 0));
		await page.waitForSelector('.gc-context-submenu', { state: 'detached' });
		assert.equal((await counts()).length, 1);
		emit({ kind: 'catalog-tier', size: count, views: await counts(), passed: true });
		await page.evaluate((count) => window.collectionFixture.setupCatalog(count), count);
		const measured = await page.evaluate(() => window.collectionFixture.measureCatalog());
		emit({ kind: 'measurement', ...measured });
		assert(measured.passed, `Catalog feedback gate failed: ${JSON.stringify(measured.summary)}`);
	}
	await page.evaluate(() => window.collectionFixture.setupCatalog(20));
	assert.equal(await page.locator('.catalog-list').count(), 0);
	assert.equal(await page.locator('.gc-context-item').count(), 21);
	emit({ kind: 'catalog-result', passed: true });
}
