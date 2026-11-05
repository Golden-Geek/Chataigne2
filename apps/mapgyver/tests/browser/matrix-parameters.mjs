import assert from 'node:assert/strict';
import { clearMatrixScene } from './matrix-scene.mjs';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixParametersUi({ base, edit, page: nativePage = null }) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Typed Matrix rows'
	});

	await clearMatrixScene(edit, reference.media);
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const { items: groups } = await matrix({ kind: 'AddGroup', parent: null, label: 'Typed Group' });
	const { items: layers } = await matrix({ kind: 'AddLayers', parent: groups[0].id, count: 1 });
	await matrix({ kind: 'AddColumns', count: 1 });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1200 } }));
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Typed Matrix row Parameters');
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	try {
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Typed Matrix rows', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix(?::|$)/ })
			.click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		for (const item of [groups[0], layers[0]]) {
			const row = (await inspect()).items.find((row) => row.id.id === item.id);
			await grid.getByTitle(`Select ${row.label}`, { exact: true }).click();
			await parameter('Opacity').locator('input.number-field').waitFor();
			assert.equal(await page.locator('.inspector .runtime-feedback').count(), 0);
			assert.equal(
				await page.getByRole('button', { name: 'Save row settings', exact: true }).count(),
				0
			);
			for (const [field, value] of [
				['opacity', 0.25],
				['blend', 'Add'],
				['blend', 'Multiply'],
				['blend', 'Screen'],
				['blend', 'Normal'],
				['enabled', false],
				['enabled', true]
			]) {
				const before = await inspect();
				if (field === 'opacity') {
					const input = parameter('Opacity').locator('input.number-field');
					await input.fill(String(value));
					await input.press('Enter');
				} else if (field === 'blend') {
					await parameter('Blend Mode').locator('select').selectOption(value);
				} else {
					const checkbox = parameter('Render enabled').locator('input[type=checkbox]');
					await wait(
						() => checkbox.isChecked(),
						(checked) =>
							checked === before.items.find((row) => row.id.id === item.id).settings.enabled
					);
					await checkbox.setChecked(value);
				}
				const after = await wait(
					inspect,
					(state) => state.items.find((row) => row.id.id === item.id).settings[field] === value
				);
				assert.deepEqual(after, {
					...before,
					items: before.items.map((row) =>
						row.id.id === item.id ? { ...row, settings: { ...row.settings, [field]: value } } : row
					)
				});
				await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
				await wait(inspect, (state) => isDeepStrictEqual(state, before));
				await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
				await wait(inspect, (state) => isDeepStrictEqual(state, after));
			}
			if (item.kind === 'Layer') {
				for (const [label, component, value, field] of [
					['Position', 0, 2.5, 'position'],
					['Position', 1, -3, 'position'],
					['Canvas size', 0, 8, 'size'],
					['Canvas size', 1, 4, 'size'],
					['Transition duration', 0, 900, 'duration_ms']
				]) {
					const before = await inspect();
					const input = parameter(label).locator('input.number-field').nth(component);
					await input.fill(field === 'duration_ms' ? '00:00:00.900' : String(value));
					await input.press('Tab');
					const after = await wait(inspect, (state) => {
						const layer = state.items.find((row) => row.id.id === item.id).layer;
						return field === 'position'
							? layer.position[component] === value
							: field === 'size'
								? layer.size[component === 0 ? 'width' : 'height'] === value
								: layer.duration_ms === value;
					});
					await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
					await wait(inspect, (state) => isDeepStrictEqual(state, before));
					await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
					await wait(inspect, (state) => isDeepStrictEqual(state, after));
				}
			} else {
				assert.equal(await parameter('Canvas size').count(), 0);
			}
		}
		await matrix({ kind: 'AddLayers', parent: null, count: 256 });
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(count) => count === '260'
		);
		await parameter('Opacity').locator('input.number-field').waitFor();
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.dispatchEvent(new Event('scroll'));
		});
		await wait(
			() => grid.getByTitle('Select Layer 1', { exact: true }).count(),
			(count) => count === 0
		);
		const input = parameter('Opacity').locator('input.number-field');
		await input.fill('0.625');
		await input.press('Enter');
		await wait(
			inspect,
			(state) => state.items.find((row) => row.id.id === layers[0].id).settings.opacity === 0.625
		);
		await grid.evaluate((element) => {
			element.scrollTop = 0;
			element.dispatchEvent(new Event('scroll'));
		});
		await grid
			.getByTitle('Layer 1 / Column 1', { exact: true })
			.first()
			.click({ position: { x: 5, y: 5 } });
		await grid
			.getByRole('gridcell', { selected: true })
			.click({ button: 'right', position: { x: 5, y: 5 } });
		await page.getByRole('menuitem', { name: 'Inspect Cell settings', exact: true }).click();
		await parameter('Enabled').locator('input[type=checkbox]').waitFor();
		const cellSettings = (state) =>
			state.cells.find((cell) => cell.layer === layers[0].id).definition.settings;
		const sizeToggle = parameter('Size').getByRole('switch', { name: 'Toggle node enabled' });
		assert.equal(cellSettings(await inspect()).size, null, 'new Cells follow natural source size');
		const beforeSize = await inspect();
		await sizeToggle.click();
		const sized = await wait(inspect, (state) => cellSettings(state).size !== null);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, beforeSize));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (state) => isDeepStrictEqual(state, sized));
		for (const [label, component, value, field] of [
			['Position', 0, 3.5, 'position'],
			['Position', 1, -2, 'position'],
			['Size', 0, 40, 'size'],
			['Size', 1, 24, 'size'],
			['Opacity', 0, 0.375, 'opacity'],
			['Enabled', 0, false, 'enabled'],
			['Enabled', 0, true, 'enabled']
		]) {
			const before = await inspect();
			if (field === 'enabled') {
				const input = parameter(label).locator('input[type=checkbox]');
				await wait(
					() => input.isChecked(),
					(checked) => checked === cellSettings(before).enabled
				);
				await input.setChecked(value);
			} else {
				const input = parameter(label).locator('input.number-field').nth(component);
				await input.fill(String(value));
				await input.press('Tab');
			}
			const after = await wait(inspect, (state) =>
				field === 'size'
					? cellSettings(state).size[component === 0 ? 'width' : 'height'] === value
					: field === 'position'
						? cellSettings(state).position[component] === value
						: cellSettings(state)[field] === value
			);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
		}
		for (const mode of ['Fill', 'Stretch', 'Fit']) {
			const before = await inspect();
			await parameter('Fit Mode').locator('select.dropdown-editor').selectOption(mode);
			const after = await wait(inspect, (state) => cellSettings(state).fit_mode === mode);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, before));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(inspect, (state) => isDeepStrictEqual(state, after));
		}
		await sizeToggle.click();
		await wait(inspect, (state) => cellSettings(state).size === null);
		assert.equal(await parameter('Canvas size').count(), 0);
		await grid.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
			element.dispatchEvent(new Event('scroll'));
		});
		await wait(
			() => grid.getByTitle('Layer 1 / Column 1', { exact: true }).count(),
			(count) => count === 0
		);
		await parameter('Opacity').locator('input.number-field').fill('0.625');
		await parameter('Opacity').locator('input.number-field').press('Tab');
		await wait(inspect, (state) => cellSettings(state).opacity === 0.625);
		assert.equal(await page.locator('.inspector .runtime-feedback').count(), 0);
		await matrix({
			kind: 'CreateCellMedia',
			layer: layers[0].id,
			column: (await inspect()).columns[0].id,
			media_kind: 'SolidColor'
		});
		const ownedSource = (await inspect()).cells.find((cell) => cell.layer === layers[0].id)
			.definition.source;
		const readOwned = async () =>
			(await edit({ kind: 'InspectChannel', reference: ownedSource })).snapshot.definition.source
				.rgba;
		await grid.evaluate((element) => {
			element.scrollTop = 0;
			element.dispatchEvent(new Event('scroll'));
		});
		await grid
			.locator(`[role="row"][data-row-id="${layers[0].id}"]`)
			.getByTitle('Layer 1 / Column 1', { exact: true })
			.click({ button: 'right', position: { x: 5, y: 5 } });
		await page.getByRole('menuitem', { name: 'Inspect media', exact: true }).click();
		const color = parameter('Color');
		await color.locator('.cp-preview').click();
		await color.locator('.cp-input-hex').fill('ff0000');
		await color.locator('.cp-input-hex').press('Tab');
		await wait(readOwned, (rgba) => isDeepStrictEqual(rgba, [1, 0, 0, 1]));
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(readOwned, (rgba) => isDeepStrictEqual(rgba, [1, 1, 1, 1]));
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(readOwned, (rgba) => isDeepStrictEqual(rgba, [1, 0, 0, 1]));
		await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Mixed Inspector peer' });
		const manager = page.getByRole('region', { name: 'Medias', exact: true });
		const matrixNode = manager.getByRole('button', { name: 'Typed Matrix rows', exact: true });
		const peerNode = manager.getByRole('button', { name: 'Mixed Inspector peer', exact: true });
		await matrixNode.click();
		await page
			.locator('.inspector-header .title-text')
			.filter({ hasText: /^Typed Matrix rows$/ })
			.waitFor();
		assert.equal(await page.locator('.inspector .runtime-feedback').count(), 0);
		const beforeMixed = await inspect();
		await peerNode.click({ modifiers: ['Control'] });
		await page
			.locator('.inspector-header .title-text')
			.filter({ hasText: /^No node selected$/ })
			.waitFor();
		assert.equal(
			await page.locator('.inspector .parameter-inspector').count(),
			0,
			'mixed selection exposes no stale ordinary widgets'
		);
		await peerNode.click({ modifiers: ['Control'] });
		await page
			.locator('.inspector-header .title-text')
			.filter({ hasText: /^Typed Matrix rows$/ })
			.waitFor();
		assert.deepEqual(await inspect(), beforeMixed, 'mixed selection does not author Matrix state');
		assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/m07-row-parameters-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Standard Layer/Group, Matrix and Cell Inspectors; exact per-field Undo/Redo; offscreen Layer/Cell editing; mixed selection clears stale widgets and restores the remaining selection; no Matrix runtime prose or playback side effects',
				browser: browser?.version() ?? 'native WebView'
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m07-row-parameters-ui-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		throw error;
	} finally {
		await browser?.close();
	}
}
