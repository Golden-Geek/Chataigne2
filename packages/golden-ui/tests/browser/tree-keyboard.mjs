import assert from 'node:assert/strict';

export async function qualifyTreeKeyboard(page, emit) {
	const root = (panel) =>
		panel === 'manager'
			? '.manager-list-content'
			: panel === 'outliner'
				? '.outliner-content'
				: '.picker-tree';
	const focus = (panel, id) =>
		page.evaluate(([panel, id]) => window.collectionFixture.treeFocus(panel, id), [panel, id]);
	const navigate = (panel, key) =>
		page.evaluate(
			([panel, key]) => window.collectionFixture.treeNavigate(panel, key),
			[panel, key]
		);
	const focused = () =>
		page.evaluate(() => Number(document.activeElement?.getAttribute('data-tree-node-id') ?? -1));
	const assertBounds = async () => {
		const counts = await page
			.locator('.virtual-rows')
			.evaluateAll((nodes) => nodes.map((node) => Number(node.dataset.mountedRowCount)));
		assert(
			counts.every((count) => count > 0 && count <= 42),
			`Mounted tree bounds: ${counts}`
		);
	};
	for (const size of [1000, 10000, 20000, 50000, 100000]) {
		for (const panel of ['manager', 'outliner', 'picker']) {
			await page.evaluate((size) => window.collectionFixture.setup(size), size);
			if (panel === 'picker')
				await page.evaluate(() => window.collectionFixture.openPicker(2, true));
			assert.equal(await focus(panel, 2), true);
			assert.equal(await focused(), 2);
			await navigate(panel, 'ArrowRight');
			assert.equal(await focused(), 2);
			await navigate(panel, 'ArrowRight');
			assert.equal(await focused(), size + 2);
			await page.evaluate((panel) => window.collectionFixture.treeCollapse(panel, 2), panel);
			assert.equal(
				await focused(),
				2,
				`${panel}: collapsing a focused descendant returns to the visible parent`
			);
			await navigate(panel, 'ArrowRight');
			await navigate(panel, 'ArrowRight');
			await navigate(panel, 'ArrowLeft');
			// Nested folders start expanded; Left first collapses the current branch.
			if ((await focused()) !== 2) await navigate(panel, 'ArrowLeft');
			assert.equal(await focused(), 2);
			await navigate(panel, 'ArrowLeft');
			assert.equal(
				await page.locator(`${root(panel)} [data-tree-node-id="2"]`).getAttribute('aria-expanded'),
				'false'
			);
			await navigate(panel, 'ArrowDown');
			assert.equal(await focused(), 3);
			await navigate(panel, 'ArrowUp');
			assert.equal(await focused(), 2);
			if (panel !== 'outliner') {
				await page.evaluate((panel) => window.collectionFixture.treeReject(panel), panel);
				assert.equal(
					await page
						.locator(`${root(panel)} [data-tree-node-id="2"]`)
						.getAttribute('aria-expanded'),
					'false'
				);
			}
			await navigate(panel, 'End');
			assert.equal(await focused(), size + 1);
			assert(
				(
					await page.locator(`${root(panel)} [data-tree-node-id="${size + 1}"]`).textContent()
				).includes(`Item ${String(size - 1).padStart(6, '0')}`)
			);
			await navigate(panel, 'Home');
			assert.equal(await focused(), panel === 'manager' ? 2 : panel === 'outliner' ? 0 : 1);
			await assertBounds();
			if (panel !== 'picker') {
				const selections = await page.evaluate(
					(panel) => window.collectionFixture.treeModifiers(panel),
					panel
				);
				assert.deepEqual(selections, { toggled: [2, 3], removed: [2] });
			}
			await focus(panel, 3);
			const handle = await page.locator(`${root(panel)} [data-tree-node-id="3"]`).elementHandle();
			assert(handle);
			try {
				await page.locator(root(panel)).evaluate((element) => {
					element.scrollTop = element.scrollHeight;
				});
				await page.evaluate(() => new Promise(requestAnimationFrame));
				assert.equal(
					await handle.evaluate(
						(element) => element.isConnected && element === document.activeElement
					),
					true
				);
				await assertBounds();
			} finally {
				await handle.dispose();
			}
			await navigate(panel, 'Enter');
			if (panel !== 'picker')
				assert.deepEqual(await page.evaluate(() => window.collectionFixture.treeSelection()), [3]);
			else assert.equal(await page.locator('.node-picker-modal').count(), 1);
			emit({ kind: 'tree-keyboard-correctness', panel, size, passed: true });
			for (const action of ['focus', 'expand']) {
				const timing = await page.evaluate(
					([panel, action]) => window.collectionFixture.treeMeasure(panel, action),
					[panel, action]
				);
				emit({ kind: 'tree-keyboard-timing', ...timing });
				assert(timing.passed, `${timing.name}/${size}: ${JSON.stringify(timing.summary)}`);
			}
			await focus(panel, 3);
			assert.deepEqual(
				await page.evaluate((panel) => window.collectionFixture.treeRace(panel, 'newer'), panel),
				[false, true]
			);
			assert.equal(await focused(), 3);
			const searchRace = await page.evaluate(
				(panel) => window.collectionFixture.treeRace(panel, 'search'),
				panel
			);
			assert.equal(searchRace[0], false);
			assert.equal(await page.evaluate(() => document.activeElement?.tagName), 'INPUT');
			await page.evaluate(
				(panel) => window.collectionFixture.treeSearch(panel, 'no matching label'),
				panel
			);
			await page.evaluate((panel) => window.collectionFixture.treeSearch(panel, ''), panel);
			assert.equal(await page.evaluate(() => document.activeElement?.tagName), 'INPUT');
			await page.evaluate((panel) => window.collectionFixture.treeEnter(panel), panel);
			assert.notEqual(await focused(), -1);
			await focus(panel, 3);
			await page.evaluate(() => window.collectionFixture.treeDelete(3));
			assert.notEqual(await focused(), 3);
			assert.notEqual(await focused(), -1, `${panel}: removal retains a visible focus owner`);
			await page.evaluate(
				(panel) => window.collectionFixture.treeSearch(panel, 'no matching label'),
				panel
			);
			assert.equal(
				await page.evaluate((selector) => document.activeElement?.matches(selector), root(panel)),
				true
			);
			await page.evaluate((panel) => window.collectionFixture.treeSearch(panel, ''), panel);
			await page.evaluate((panel) => window.collectionFixture.treeEnter(panel), panel);
			const replacement = await page.evaluate(
				(panel) => window.collectionFixture.treeRace(panel, 'replace'),
				panel
			);
			assert.equal(replacement[0], false);
			await assertBounds();
			emit({ kind: 'tree-keyboard-lifecycle', panel, size, passed: true });
		}
	}
}
