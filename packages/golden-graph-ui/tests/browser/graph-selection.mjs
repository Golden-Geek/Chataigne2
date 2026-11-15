import assert from 'node:assert/strict';

/** Real canvas input; the host's empty-selection callback remains the domain owner boundary. */
export async function qualifyGraphSelection(page, emit) {
	const setup = () => page.evaluate(() => window.goldenGraphFixture.setupSelection());
	const inspect = () => page.evaluate(() => window.goldenGraphFixture.selectionInspect());
	const canvas = page.getByRole('application', { name: 'Node graph', exact: true });
	const node = (id) => canvas.locator(`article.node[data-node-id="${id}"]`);
	const point = async (target) => {
		const box = await target.boundingBox();
		assert(box);
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const background = async () => {
		const box = await canvas.boundingBox();
		assert(box);
		return { x: box.x + box.width * 0.8, y: box.y + box.height * 0.65 };
	};
	await setup();
	assert.deepEqual(await inspect(), { nodes: ['a', 'b'], edges: ['edge'], changes: 0 });
	assert.equal(await node('a').locator('.resize-handle').count(), 1);
	assert.equal(await node('c').locator('.resize-handle').count(), 0);
	const bg = await background();
	await page.mouse.move(bg.x, bg.y);
	const contour = await node('c').evaluate((element) => getComputedStyle(element).borderTopColor);
	await node('c').hover();
	await page.waitForFunction((before) => {
		const element = document.querySelector('article.node[data-node-id="c"]');
		return element.matches(':hover') && getComputedStyle(element).borderTopColor !== before;
	}, contour);
	assert.equal(await node('c').locator('.resize-handle').count(), 0);
	assert.equal((await inspect()).changes, 0, 'hover cannot select or show transform controls');
	const title = node('c').locator('.node-title-label');
	const hit = await point(title);
	await page.mouse.move(hit.x, hit.y);
	await page.mouse.down();
	assert.deepEqual((await inspect()).nodes, ['c'], 'disabled node selects before release');
	assert.equal(await node('c').locator('.resize-handle').count(), 1);
	await page.mouse.up();
	await page.mouse.move(bg.x, bg.y);
	await page.mouse.down();
	const cleared = await inspect();
	assert.deepEqual(cleared.nodes, []);
	assert.deepEqual(cleared.edges, []);
	assert.equal(await canvas.locator('.resize-handle').count(), 0);
	await page.mouse.up();
	assert.equal((await inspect()).changes, cleared.changes, 'release cannot repeat an owner clear');
	emit({ kind: 'selection-down-hover-controls', passed: true });

	await setup();
	const field = page.getByRole('textbox', { name: 'Fixture node text', exact: true });
	await field.fill('text remains owned');
	await field.press('Escape');
	assert.deepEqual((await inspect()).nodes, ['a', 'b'], 'text Escape stays with its editor');
	const home = canvas.getByRole('button', { name: 'Home', exact: true });
	await home.focus();
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, ['a', 'b'], 'toolbar Escape cannot clear the document');
	await canvas.getByRole('button', { name: 'Nodes', exact: true }).click();
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, ['a', 'b'], 'finder consumes Escape');
	await canvas.focus();
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, []);
	assert.deepEqual((await inspect()).edges, []);
	assert.equal(await canvas.locator('.resize-handle').count(), 0);
	emit({ kind: 'selection-idle-escape-focus', passed: true });

	await setup();
	const empty = await background();
	await page.mouse.move(empty.x, empty.y);
	await page.mouse.down({ button: 'middle' });
	await page.mouse.move(empty.x + 20, empty.y + 12);
	await page.mouse.up({ button: 'middle' });
	assert.equal((await inspect()).changes, 0, 'middle-button navigation preserves selection');
	for (const key of ['Control', 'Meta', 'Alt']) {
		await page.keyboard.down(key);
		await page.mouse.move(empty.x, empty.y);
		await page.mouse.down();
		await page.mouse.up();
		await page.keyboard.up(key);
	}
	assert.equal((await inspect()).changes, 0, 'modified background input preserves multi-selection');
	await page.evaluate(() => window.goldenGraphFixture.keyboardSelect('c'));
	const b = await node('b').boundingBox();
	assert(b);
	await page.keyboard.down('Shift');
	await page.mouse.move(b.x - 4, b.y + b.height + 4);
	await page.mouse.down();
	assert.deepEqual((await inspect()).nodes, ['c']);
	await page.mouse.move(b.x + b.width + 4, b.y - 4);
	await page.mouse.up();
	await page.keyboard.up('Shift');
	assert.deepEqual(new Set((await inspect()).nodes), new Set(['c', 'b']), 'marquee stays additive');
	emit({ kind: 'selection-modifiers-marquee-pan', passed: true });

	await setup();
	const socket = node('a').locator('button.socket[data-socket-id="out"]').first();
	await socket.focus();
	await page.keyboard.press('Enter');
	assert((await page.evaluate(() => window.goldenGraphFixture.keyboardInspect())).state.endpoint);
	await page.keyboard.press('Escape');
	assert.equal(
		(await page.evaluate(() => window.goldenGraphFixture.keyboardInspect())).state.endpoint,
		null
	);
	assert.deepEqual(
		(await inspect()).nodes,
		['a', 'b'],
		'connection cancellation precedes deselection'
	);
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, [], 'idle Escape also works from a focused socket');
	await setup();
	const port = await point(socket);
	await page.mouse.move(port.x, port.y);
	await page.mouse.down();
	assert(await canvas.evaluate((element) => element.classList.contains('connecting')));
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, ['a', 'b']);
	assert.equal(await canvas.evaluate((element) => element.classList.contains('connecting')), false);
	await page.mouse.up();
	assert.equal(
		(await page.evaluate(() => window.goldenGraphFixture.keyboardInspect())).connections.length,
		0
	);
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).nodes, []);
	emit({ kind: 'selection-connection-capture', passed: true });

	await setup();
	const edge = canvas.getByRole('button', { name: 'Select connection', exact: true });
	const center = await edge.evaluate((element) => {
		const local = element.getPointAtLength(element.getTotalLength() / 2);
		const screen = new DOMPoint(local.x, local.y).matrixTransform(element.getScreenCTM());
		return { x: screen.x, y: screen.y };
	});
	await page.mouse.move(center.x, center.y);
	await page.mouse.down();
	assert.deepEqual((await inspect()).nodes, []);
	assert.deepEqual((await inspect()).edges, ['edge']);
	await page.mouse.up();
	await page.keyboard.press('Escape');
	assert.deepEqual((await inspect()).edges, []);
	await page.evaluate(() => window.goldenGraphFixture.keyboardMode('readonly'));
	const readonly = await point(title);
	await page.mouse.move(readonly.x, readonly.y);
	await page.mouse.down();
	assert.deepEqual((await inspect()).nodes, ['c'], 'selection survives absent movement callbacks');
	await page.mouse.up();
	emit({ kind: 'selection-edges-readonly', passed: true });
}
