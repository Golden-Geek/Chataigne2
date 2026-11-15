import assert from 'node:assert/strict';

export async function qualifyGraphCancellation(page, emit) {
	const canvas = page.getByRole('application', { name: 'Node graph', exact: true });
	const inspect = () => page.evaluate(() => window.goldenGraphFixture.pointerInspect());
	const setup = () => page.evaluate(() => window.goldenGraphFixture.setupSelection());
	const point = async (target) => {
		const box = await target.boundingBox();
		assert(box);
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const press = async (target, button = 'left') => {
		await canvas.evaluate((element) =>
			element.addEventListener(
				'pointerdown',
				(event) => {
					element.dataset.cancelTestPointer = String(event.pointerId);
					element.dataset.cancelTestX = String(event.clientX);
					element.dataset.cancelTestY = String(event.clientY);
				},
				{ once: true, capture: true }
			)
		);
		await target.hover();
		await page.mouse.down({ button });
		const id = Number(await canvas.getAttribute('data-cancel-test-pointer'));
		assert(await canvas.evaluate((element, pointer) => element.hasPointerCapture(pointer), id));
		return {
			x: Number(await canvas.getAttribute('data-cancel-test-x')),
			y: Number(await canvas.getAttribute('data-cancel-test-y')),
			id
		};
	};
	await setup();
	const before = await inspect();
	const title = canvas.locator('article[data-node-id="a"] .node-title-label');
	const active = await press(title);
	assert(await canvas.evaluate((element) => element.classList.contains('node-dragging')));
	await page.mouse.move(active.x + 64, active.y + 24);
	await page.waitForFunction(
		(position) => {
			const node = document.querySelector('article[data-node-id="a"]');
			return node && parseFloat(node.style.left) !== position;
		},
		before.positions[0].x,
		{ timeout: 5000 }
	);
	assert.notEqual(
		await canvas
			.locator('article[data-node-id="a"]')
			.evaluate((node) => parseFloat(node.style.left)),
		before.positions[0].x
	);
	await canvas.dispatchEvent('pointercancel', { pointerId: active.id });
	await page.mouse.up();
	assert.deepEqual(
		await inspect(),
		before,
		'pointer cancellation cannot publish a movement mutation'
	);
	emit({ kind: 'pointer-cancel-no-commit', passed: true });
	const titleNode = canvas.locator('article[data-node-id="a"]');
	const begin = async (kind) => {
		const target = kind === 'move' ? title : titleNode.locator('.resize-handle');
		const state = await inspect();
		const beforeStyle = await titleNode.evaluate((node) => ({
			x: node.style.left,
			width: node.style.width
		}));
		const pointer = await press(target);
		await page.mouse.move(pointer.x + 64, pointer.y + 24);
		await page.waitForFunction(
			({ kind, before }) => {
				const node = document.querySelector('article[data-node-id="a"]');
				return (
					node &&
					(kind === 'move' ? node.style.left !== before.x : node.style.width !== before.width)
				);
			},
			{ kind, before: beforeStyle }
		);
		return { ...pointer, state, beforeStyle };
	};
	const expectRetired = async (id) => {
		await page.waitForFunction(() => {
			const node = document.querySelector('.graph-canvas');
			return !node.matches('.node-dragging, .node-resizing, .panning, .selecting, .connecting');
		});
		assert.equal(
			await canvas.evaluate((element, pointer) => element.hasPointerCapture(pointer), id),
			false
		);
	};
	for (const kind of ['move', 'resize']) {
		for (const terminal of ['pointercancel', 'Escape', 'capture-loss', 'blur']) {
			await setup();
			const gesture = await begin(kind);
			await canvas.dispatchEvent('pointercancel', { pointerId: gesture.id + 1000 });
			assert(
				await canvas.evaluate((element, pointer) => element.hasPointerCapture(pointer), gesture.id)
			);
			if (terminal === 'pointercancel')
				await canvas.dispatchEvent('pointercancel', { pointerId: gesture.id });
			else if (terminal === 'Escape') await page.keyboard.press('Escape');
			else if (terminal === 'capture-loss') {
				await canvas.evaluate(
					(element, pointer) => element.releasePointerCapture(pointer),
					gesture.id
				);
				await page.mouse.move(gesture.x + 65, gesture.y + 24);
			} else await page.evaluate(() => window.dispatchEvent(new Event('blur')));
			await expectRetired(gesture.id);
			await page.mouse.up();
			assert.deepEqual(
				await inspect(),
				gesture.state,
				`${kind}/${terminal} cannot publish a mutation`
			);
			assert.deepEqual(
				await titleNode.evaluate((node) => ({ x: node.style.left, width: node.style.width })),
				gesture.beforeStyle
			);
			assert.deepEqual(
				(await page.evaluate(() => window.goldenGraphFixture.selectionInspect())).nodes,
				['a', 'b']
			);
			if (terminal === 'Escape') {
				await page.keyboard.press('Escape');
				assert.deepEqual(
					(await page.evaluate(() => window.goldenGraphFixture.selectionInspect())).nodes,
					[],
					'second idle Escape deselects'
				);
			}
			emit({ kind: 'pointer-cancel-case', gesture: kind, terminal, passed: true });
		}
	}
	emit({ kind: 'pointer-move-resize-cancel-identity-focus', passed: true });

	for (const kind of ['move', 'resize']) {
		for (const boundary of ['scope', 'topology', 'dispose']) {
			await setup();
			const gesture = await begin(kind);
			if (boundary === 'scope')
				await page.evaluate(() => window.goldenGraphFixture.replaceKeyboardScope());
			else if (boundary === 'topology')
				await page.evaluate(() => window.goldenGraphFixture.replacePointerTopology());
			else await page.evaluate(() => window.goldenGraphFixture.replaceCanvasWhileSearching());
			await expectRetired(gesture.id);
			const replacement = await inspect();
			await page.mouse.up();
			assert.deepEqual(
				await inspect(),
				replacement,
				'retired pointer-up cannot edit the replacement'
			);
			assert.equal(replacement.moves, 0);
			assert.equal(replacement.resizes, 0);
		}
	}
	emit({ kind: 'pointer-scope-topology-disposal', passed: true });

	await setup();
	await page.evaluate(() => window.goldenGraphFixture.keyboardMode('hold'));
	await page.evaluate(() => window.goldenGraphFixture.keyboardMove('ArrowRight'));
	const optimistic = await titleNode.evaluate((node) => node.style.left);
	const held = await begin('move');
	await page.keyboard.press('Escape');
	await expectRetired(held.id);
	await page.mouse.up();
	assert.equal(
		await titleNode.evaluate((node) => node.style.left),
		optimistic,
		'cancel preserves earlier pending optimistic work'
	);
	assert.equal((await inspect()).moves, 1);
	await page.evaluate(() => window.goldenGraphFixture.rejectHeldMove(0));
	assert.equal(await titleNode.evaluate((node) => parseFloat(node.style.left)), 1);
	emit({ kind: 'pointer-cancel-retains-pending-work', passed: true });
	await setup();
	await page.evaluate(() => window.goldenGraphFixture.keyboardSocket('a', 'out', 'output'));
	const simultaneous = await begin('move');
	await page.keyboard.press('Escape');
	await expectRetired(simultaneous.id);
	await page.mouse.up();
	assert.equal(
		(await page.evaluate(() => window.goldenGraphFixture.keyboardInspect())).state.endpoint,
		null
	);
	assert.deepEqual(await inspect(), simultaneous.state);
	await page.keyboard.press('Escape');
	assert.deepEqual(
		(await page.evaluate(() => window.goldenGraphFixture.selectionInspect())).nodes,
		[]
	);
	await setup();
	const pan = await press(canvas, 'middle');
	await page.mouse.move(pan.x + 20, pan.y + 20);
	await page.keyboard.press('Escape');
	await expectRetired(pan.id);
	await page.mouse.up({ button: 'middle' });
	assert.deepEqual(
		(await page.evaluate(() => window.goldenGraphFixture.selectionInspect())).nodes,
		['a', 'b']
	);
	assert.equal((await inspect()).moves, 0);
	emit({ kind: 'pointer-pan-and-concurrent-keyboard-tool', passed: true });

	await setup();
	await page.evaluate(() => window.goldenGraphFixture.keyboardSelect('a'));
	const b = await canvas.locator('article[data-node-id="b"]').boundingBox();
	assert(b);
	await page.keyboard.down('Shift');
	await page.mouse.move(b.x - 4, b.y + b.height + 4);
	await page.mouse.down();
	await page.mouse.move(b.x + b.width + 4, b.y - 4);
	await page.keyboard.press('Escape');
	await page.mouse.up();
	await page.keyboard.up('Shift');
	assert.deepEqual(
		(await page.evaluate(() => window.goldenGraphFixture.selectionInspect())).nodes,
		['a'],
		'marquee cancellation cannot select its enclosed nodes'
	);
	const port = canvas
		.locator('article[data-node-id="a"] button.socket[data-socket-id="out"]')
		.first();
	const draft = await press(port);
	const input = await point(
		canvas.locator('article[data-node-id="b"] button.socket[data-socket-id="in"]').first()
	);
	await page.mouse.move(input.x, input.y);
	await canvas.dispatchEvent('pointercancel', { pointerId: draft.id });
	await expectRetired(draft.id);
	await page.mouse.up();
	assert.equal(
		(await inspect()).connections,
		0,
		'cancelled connection cannot submit its snapped endpoint'
	);
	emit({ kind: 'pointer-marquee-connection-cancel', passed: true });

	for (const kind of ['move', 'resize']) {
		await setup();
		await begin(kind);
		await page.mouse.up();
		await expectRetired(Number(await canvas.getAttribute('data-cancel-test-pointer')));
		assert.equal(
			(await inspect())[kind === 'move' ? 'moves' : 'resizes'],
			1,
			'normal release commits once'
		);
	}
	emit({ kind: 'pointer-normal-release-commit', passed: true });
}
