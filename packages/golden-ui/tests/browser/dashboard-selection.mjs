import assert from 'node:assert/strict';

export async function selectionChecks(page, record) {
	page.setDefaultTimeout(5000);
	const setup = (layout = 'free', editing = true) =>
		page.evaluate(
			({ layout, editing }) => window.dashboardFixture.setup(layout, editing, 6, 'grid'),
			{
				layout,
				editing
			}
		);
	const selected = () => page.evaluate(() => window.dashboardFixture.selected());
	const state = () => page.evaluate(() => window.dashboardFixture.editState());
	const viewport = page.locator('main .dashboard-page-viewport');
	const shell = (id) => page.locator(`main .dashboard-widget-shell[data-node-id="${id}"]`);
	const bounds = async (element) => {
		const result = await element.boundingBox();
		assert(result);
		return result;
	};
	const closed = () =>
		page.waitForFunction(() => {
			const state = window.dashboardFixture.editState();
			return !state.active && state.pending === 0;
		});
	const idleEscape = async (owner) => {
		await viewport.focus();
		await page.keyboard.press('Escape');
		assert.deepEqual(await selected(), [owner]);
		assert.equal(await page.locator('main .dashboard-resize-zone').count(), 0);
	};
	for (const layout of ['free', 'horizontal', 'vertical']) {
		const scene = await setup(layout);
		const owner = Number(
			await page.locator('main .dashboard-page').getAttribute('data-dashboard-surface-node-id')
		);
		const first = shell(scene.ids[0]),
			second = shell(scene.ids[1]);
		assert.equal(
			await page.locator('main .dashboard-resize-zone').count(),
			0,
			`${layout}: no unselected hit controls`
		);
		const initialFill = await first.evaluate(
			(element) => getComputedStyle(element).backgroundColor
		);
		await first.hover();
		assert.equal(
			await first.evaluate((element) => getComputedStyle(element).backgroundColor),
			initialFill
		);
		assert.deepEqual(await selected(), []);
		const box = await bounds(first);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		assert.deepEqual(await selected(), [scene.ids[0]], 'primary-down selection precedes release');
		assert.equal(await second.locator(':scope > .dashboard-resize-zone').count(), 0);
		assert.equal(
			await first.locator(':scope > .dashboard-resize-zone').count(),
			layout === 'free' ? 8 : 2
		);
		await page.mouse.up();
		await closed();
		await page.keyboard.down('Shift');
		await second.click();
		await page.keyboard.up('Shift');
		assert.deepEqual(
			await selected(),
			[scene.ids[0], scene.ids[1]],
			'modifier selection is preserved'
		);
		await idleEscape(owner);
		await first.focus();
		await page.keyboard.press('Enter');
		assert.deepEqual(await selected(), [scene.ids[0]]);
		await idleEscape(owner);
		const area = await bounds(viewport);
		await page.mouse.move(area.x + 2, area.y + 2);
		await page.mouse.down();
		assert.deepEqual(await selected(), [owner], 'background selects edited page before release');
		await page.mouse.up();
		record(`selection-${layout}-down-hover-controls-owner-keyboard-modifiers`);
	}

	for (const layout of ['free', 'horizontal']) {
		for (const mode of ['move', 'resize']) {
			const scene = await setup(layout);
			const owner = Number(
				await page.locator('main .dashboard-page').getAttribute('data-dashboard-surface-node-id')
			);
			await shell(scene.ids[0]).click();
			await closed();
			await page.evaluate(() => window.dashboardFixture.configureEdits());
			const box = await bounds(shell(scene.ids[0]));
			const origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
			const command =
				mode === 'move'
					? { kind: 'move', origin }
					: {
							kind: 'resize',
							origin,
							edges: { left: false, top: false, right: true, bottom: true }
						};
			assert(
				await page.evaluate((command) => window.dashboardFixture.layoutEdit(0, command), command)
			);
			assert(
				await page.evaluate(
					(point) => window.dashboardFixture.layoutEdit(0, { kind: 'update', point }),
					{ x: origin.x + 20, y: origin.y + 10 }
				)
			);
			await viewport.focus();
			await page.keyboard.press('Escape');
			await closed();
			assert.deepEqual(
				await selected(),
				[scene.ids[0]],
				'first Escape cancels tool without changing selection'
			);
			const after = await state();
			assert.equal(
				await page.evaluate(() => window.dashboardFixture.layoutEdit(0, { kind: 'finish' })),
				false,
				'late release cannot commit'
			);
			assert.deepEqual(await state(), after);
			await idleEscape(owner);
			record(`selection-${layout}-${mode}-escape-cancel-before-owner`);
		}
	}

	const scene = await setup();
	const owner = Number(
		await page.locator('main .dashboard-page').getAttribute('data-dashboard-surface-node-id')
	);
	await shell(scene.ids[0]).click();
	await closed();
	const area = await bounds(viewport),
		box = await bounds(shell(scene.ids[0]));
	await page.mouse.move(area.x + 2, area.y + 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width, box.y + box.height, { steps: 5 });
	assert.equal(await page.locator('.dashboard-marquee-selection').count(), 1);
	await page.keyboard.press('Escape');
	assert.equal(await page.locator('.dashboard-marquee-selection').count(), 0);
	assert.deepEqual(
		await selected(),
		[scene.ids[0]],
		'marquee cancellation restores its prior multi-selection basis'
	);
	await page.mouse.up();
	assert.deepEqual(await selected(), [scene.ids[0]]);
	await idleEscape(owner);
	record('selection-marquee-escape-restores-basis-and-retires-release');
	for (const terminal of ['pointercancel', 'blur']) {
		await shell(scene.ids[0]).click();
		await closed();
		await viewport.evaluate((element) =>
			element.addEventListener(
				'pointerdown',
				(event) => {
					element.dataset.testPointerId = String(event.pointerId);
				},
				{ once: true, capture: true }
			)
		);
		await page.mouse.move(area.x + 2, area.y + 2);
		await page.mouse.down();
		await page.mouse.move(box.x + box.width, box.y + box.height, { steps: 5 });
		assert.equal(await page.locator('.dashboard-marquee-selection').count(), 1);
		await viewport.evaluate((element) =>
			window.dispatchEvent(
				new PointerEvent('pointercancel', { pointerId: Number(element.dataset.testPointerId) + 1 })
			)
		);
		assert.equal(
			await page.locator('.dashboard-marquee-selection').count(),
			1,
			'foreign pointer cannot cancel'
		);
		await viewport.evaluate(
			(element, terminal) =>
				window.dispatchEvent(
					terminal === 'blur'
						? new Event('blur')
						: new PointerEvent('pointercancel', {
								pointerId: Number(element.dataset.testPointerId)
							})
				),
			terminal
		);
		assert.equal(await page.locator('.dashboard-marquee-selection').count(), 0);
		assert.deepEqual(await selected(), [scene.ids[0]]);
		await page.mouse.up();
		assert.deepEqual(await selected(), [scene.ids[0]]);
		record(`selection-marquee-${terminal}-restores-basis`);
	}

	for (const mode of ['move', 'resize']) {
		await shell(scene.ids[0]).click();
		await closed();
		await page.evaluate(() => window.dashboardFixture.configureEdits());
		const target =
			mode === 'move'
				? shell(scene.ids[0])
				: shell(scene.ids[0]).locator(':scope > .dashboard-resize-zone.south-east');
		await target.hover();
		const targetBox = await bounds(target);
		await page.mouse.move(targetBox.x + targetBox.width / 2, targetBox.y + targetBox.height / 2);
		await page.mouse.down();
		await page.mouse.move(
			targetBox.x + targetBox.width / 2 + 18,
			targetBox.y + targetBox.height / 2 + 9,
			{ steps: 3 }
		);
		await page.keyboard.press('Escape');
		await closed();
		assert.deepEqual(await selected(), [scene.ids[0]]);
		const canceled = await state();
		await page.mouse.up();
		assert.deepEqual(await state(), canceled, 'physical late release cannot add a commit');
		record(`selection-pointer-${mode}-escape-cancel`);
	}

	await viewport.focus();
	await page.mouse.move(area.x + 5, area.y + 5);
	await page.mouse.down({ button: 'middle' });
	await page.mouse.move(area.x + 20, area.y + 12);
	await page.keyboard.press('Escape');
	const stoppedCamera = await page.locator('main .dashboard-page-frame').getAttribute('style');
	await page.mouse.move(area.x + 40, area.y + 25);
	assert.equal(
		await page.locator('main .dashboard-page-frame').getAttribute('style'),
		stoppedCamera
	);
	assert.deepEqual(await selected(), [scene.ids[0]], 'navigation cancellation preserves selection');
	await page.mouse.up({ button: 'middle' });
	await idleEscape(owner);
	record('selection-navigation-escape-before-owner');

	await shell(scene.ids[0]).click();
	await closed();
	await page.evaluate(() => window.dashboardFixture.configureEdits());
	assert(
		await page.evaluate(() =>
			window.dashboardFixture.layoutEdit(0, { kind: 'move', origin: { x: 100, y: 100 } })
		)
	);
	await page.evaluate(() => window.dashboardFixture.showComparison(true));
	await page.getByLabel('Independent Dashboard editor').locator('.dashboard-page-viewport').focus();
	await page.keyboard.press('Escape');
	assert(
		await page.evaluate(() =>
			window.dashboardFixture.layoutEdit(0, { kind: 'update', point: { x: 110, y: 105 } })
		),
		'other editor Escape cannot cancel this interaction'
	);
	await viewport.focus();
	await page.keyboard.press('Escape');
	await closed();
	await page.evaluate(() => window.dashboardFixture.showComparison(false));
	record('selection-independent-editor-cancellation-scope');
	const nested = await page.evaluate(() =>
		window.dashboardFixture.setup('free', true, 6, 'grid', true)
	);
	await shell(nested.ids[0]).focus();
	await page.keyboard.press('Enter');
	assert.deepEqual(
		await selected(),
		[nested.ids[0]],
		'nested widget key cannot select its ancestor'
	);
	record('selection-nested-widget-keyboard-isolation');

	await setup('free', false);
	const input = page.locator('main input[type="text"]').first();
	await input.focus();
	assert(
		await input.evaluate((element) =>
			element.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
			)
		),
		'Run mode text Enter is not swallowed by widget selection'
	);
	await page.keyboard.press('Escape');
	assert.deepEqual(await selected(), [], 'Run mode text input owns Escape');
	assert.equal(await page.locator('main .dashboard-resize-zone').count(), 0);
	record('selection-run-mode-text-isolation');
}
