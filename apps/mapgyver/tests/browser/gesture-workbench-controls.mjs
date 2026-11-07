import assert from 'node:assert/strict';
import { verifyInspectorStability } from './inspector-stability.mjs';

// Functional native-WebView controls; no sampled-frame pacing or latency inference.
export async function verifyWorkbenchControls({ page, edit, post, qualification }) {
	const inspector = await verifyInspectorStability({ page, edit, post });
	const width = page
		.locator('.parameter-inspector')
		.filter({
			has: page.getByText('Resolution', { exact: true })
		})
		.locator('input.number-field')
		.first();
	assert.equal(await width.inputValue(), '64');
	await qualification?.after('ordinary-inspector-controls');
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Workbench scroll Matrix'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	await matrix({ kind: 'AddLayers', parent: null, count: 37 });
	await matrix({ kind: 'AddColumns', parent: null, count: 37 });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const before = await inspect();
	assert.equal(before.items.length, 40);
	assert.equal(before.columns.length, 40);
	assert.equal(before.cells.length, 0);
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
	await tab('Medias').click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', {
			name: 'Workbench scroll Matrix',
			exact: true
		})
		.click();
	await tab('Matrix').click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	await page.waitForFunction(
		() =>
			document
				.querySelector('[role="grid"][aria-label="Scrollable Matrix grid"]')
				?.getAttribute('aria-rowcount') === '42'
	);
	const identity = await grid.elementHandle();
	const readScroll = () =>
		grid.evaluate((element) => ({
			top: element.scrollTop,
			left: element.scrollLeft,
			maxTop: element.scrollHeight - element.clientHeight,
			maxLeft: element.scrollWidth - element.clientWidth
		}));
	const initial = await readScroll();
	assert(initial.maxTop > 600 && initial.maxLeft > 600, 'Both real overflow axes required');
	await page.evaluate(() => {
		window.__workbenchControlInputs = [];
		for (const type of ['wheel', 'pointerdown', 'pointerup'])
			window.addEventListener(
				type,
				(event) => window.__workbenchControlInputs.push({ type, trusted: event.isTrusted }),
				{ capture: true, passive: true }
			);
	});
	await grid.hover();
	await page.mouse.wheel(600, 600);
	await page.waitForFunction(() => {
		const grid = document.querySelector('[role="grid"][aria-label="Scrollable Matrix grid"]');
		return grid?.scrollTop > 50 && grid.scrollLeft > 50;
	});
	const scrolled = await readScroll();
	await qualification?.after('shared-grid-scrolled');
	await grid.hover();
	await page.mouse.wheel(-10000, -10000);
	await page.waitForFunction(() => {
		const grid = document.querySelector('[role="grid"][aria-label="Scrollable Matrix grid"]');
		return grid?.scrollTop === 0 && grid.scrollLeft === 0;
	});
	const original = await grid.boundingBox();
	assert(original);
	async function resize(delta) {
		const group = grid.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		const box = await group.boundingBox();
		const candidates = await page
			.locator('.dv-sash.dv-enabled')
			.evaluateAll((elements) =>
				elements.map((element) => element.getBoundingClientRect().toJSON())
			);
		const matches = candidates.filter(
			(rect) => rect.height > 100 && Math.abs(rect.x + rect.width / 2 - box.x - box.width) < 12
		);
		assert.equal(matches.length, 1, 'One actual right-hand dock divider required');
		const sash = matches[0],
			x = sash.x + sash.width / 2,
			y = sash.y + sash.height / 2;
		const previous = await grid.boundingBox();
		await page.mouse.move(x, y);
		await page.mouse.down();
		try {
			await page.mouse.move(x + delta, y, { steps: 12 });
		} finally {
			await page.mouse.up();
		}
		await page.waitForFunction(
			({ width }) => {
				const grid = document.querySelector('[role="grid"][aria-label="Scrollable Matrix grid"]');
				return Math.abs(grid.getBoundingClientRect().width - width) > 20;
			},
			{ width: previous.width }
		);
		const current = await grid.boundingBox();
		assert(
			await identity.evaluate((element) => element.isConnected),
			'Dock resize retains mounted grid'
		);
		await qualification?.after(delta < 0 ? 'dock-narrowed' : 'dock-restored');
		return current;
	}
	const narrowed = await resize(-80);
	assert(narrowed.width < original.width - 20);
	const restored = await resize(80);
	assert(Math.abs(restored.width - original.width) < 2, 'Dock width restores');
	assert.deepEqual(
		await inspect(),
		before,
		'Scrolling/docking never authors Matrix topology or settings'
	);
	const inputs = await page.evaluate(() => window.__workbenchControlInputs);
	assert(inputs.length > 4 && inputs.every((event) => event.trusted));
	for (const type of ['wheel', 'pointerdown', 'pointerup'])
		assert(inputs.some((event) => event.type === type));
	return {
		inspector,
		resolution_readback: 64,
		initial,
		scrolled,
		original,
		narrowed,
		restored,
		inputs,
		same_grid: true,
		authored_state_unchanged: true
	};
}
