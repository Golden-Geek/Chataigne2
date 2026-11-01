// Actual Matrix controls and keyboard navigation; backend operations only inspect committed state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	try {
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Matrix$/ });
		await tab.click();
		const panel = page.locator('section[aria-label="Matrix"]');
		const properties = page.locator('.properties');
		const actions = page.locator('section[aria-label="Matrix"] .editor-toolbar, .properties');
		const button = (name) => actions.getByRole('button', { name, exact: true });
		const ready = async () => {
			await wait(() => button('Add Matrix').isEnabled(), Boolean);
			await wait(
				() => panel.getByRole('status').filter({ hasText: 'Refreshing visible cells' }).count(),
				(value) => value === 0
			);
		};
		const originalMedia = await inspectMedia();
		const solid = originalMedia.media.find((value) => value.media_kind === 'SolidColor');
		await button('Add Matrix').click();
		const catalog = await wait(inspectMedia, (value) =>
			value.media.some((entry) => entry.media_kind === 'Matrix')
		);
		const root = catalog.media.find((entry) => entry.media_kind === 'Matrix');
		const inspect = async () =>
			(await edit({ kind: 'Matrix', media: root.id, edit: { kind: 'Inspect' } })).snapshot;
		const playback = async () =>
			(await edit({ kind: 'Matrix', media: root.id, edit: { kind: 'InspectPlayback' } })).snapshot;
		const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		async function restore(before, after, returnBefore = false) {
			await undo();
			await wait(inspect, (value) => isDeepStrictEqual(value, before));
			await redo();
			await wait(inspect, (value) => isDeepStrictEqual(value, after));
			if (returnBefore) {
				await undo();
				await wait(inspect, (value) => isDeepStrictEqual(value, before));
			}
			await ready();
		}
		const grid = panel.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const selectRow = async (label) => {
			await ready();
			await grid.focus();
			await page.keyboard.press('PageUp');
			await wait(
				() => grid.getByTitle(`Select ${label}`, { exact: true }).count(),
				(value) => value === 1
			);
			await grid.getByTitle(`Select ${label}`, { exact: true }).click();
			await wait(
				() => properties.getByLabel('Row name', { exact: true }).inputValue(),
				(value) => value === label
			);
		};
		const selectColumn = async (label) => {
			await ready();
			await wait(
				() => grid.getByTitle(`Select ${label}`, { exact: true }).count(),
				(value) => value === 1
			);
			await grid.getByTitle(`Select ${label}`, { exact: true }).click();
			await wait(
				() => properties.getByLabel('Column name', { exact: true }).inputValue(),
				(value) => value === label
			);
		};
		const nameForm = (label) =>
			properties
				.locator('form')
				.filter({ has: page.getByRole('textbox', { name: label, exact: true }) });
		await properties.getByRole('textbox', { name: 'Channel name', exact: true }).waitFor();
		const outer = await panel.boundingBox(),
			canvas = await panel.locator('.editor-canvas').boundingBox();
		assert(
			canvas.width >= outer.width * 0.95 && canvas.height >= outer.height * 0.8,
			'Matrix uses its panel'
		);
		assert.equal(
			await panel.locator('input,select,fieldset').count(),
			0,
			'Matrix properties live in Inspector'
		);
		assert(
			(await panel.locator('.editor-toolbar button').allTextContents()).every(
				(text) => text.trim() === ''
			),
			'Structural toolbar actions are icon-only'
		);
		stage = 'create rows columns and independent selection';
		await button('Add layer').click();
		let snapshot = await wait(inspect, (value) => value.items.length === 1);
		const lower = snapshot.items[0];
		await button('Add group').click();
		snapshot = await wait(inspect, (value) => value.items.length === 2);
		const group = snapshot.items.find((value) => value.id.kind === 'Group');
		await selectRow(group.label);
		await button('Add child layer').click();
		snapshot = await wait(inspect, (value) => value.items.length === 3);
		const child = snapshot.items.find((value) => value.parent === group.id.id);
		// Nested groups may legitimately reuse a row label. Name this child through its visible row.
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(value) => value === '4'
		);
		await grid.focus();
		await page.keyboard.press('PageDown');
		await grid
			.locator('[role="row"][aria-rowindex="4"]')
			.getByRole('rowheader')
			.getByRole('button')
			.click();
		await nameForm('Row name').getByLabel('Row name', { exact: true }).fill('UI Child');
		await nameForm('Row name').getByRole('button', { name: 'Rename', exact: true }).click();
		await wait(
			inspect,
			(value) => value.items.find((entry) => entry.id.id === child.id.id).label === 'UI Child'
		);
		await grid.getByTitle('Select UI Child', { exact: true }).waitFor({ state: 'attached' });
		for (let index = 1; index <= 2; index++) {
			await button('Add column').click();
			await wait(inspect, (value) => value.columns.length === index);
		}
		snapshot = await inspect();
		const [first, second] = snapshot.columns;
		assert.equal((await playback()).desired_column, null);
		await selectRow(lower.label);
		await selectColumn(first.label);
		const cell = grid.getByTitle(`${lower.label} / ${first.label}`, { exact: true });
		await cell.click();
		assert.equal(await cell.getAttribute('aria-selected'), 'true');
		assert.deepEqual(await inspect(), snapshot);
		assert.equal(
			(await playback()).desired_column,
			null,
			'row/column/cell selection cannot launch'
		);
		stage = 'source assignment history and feedback rejection';
		const sourcePicker = properties.getByRole('combobox', { name: /^Source to assign/ });
		await properties.getByLabel('Find Matrix source', { exact: true }).fill(solid.label);
		await button('Search sources').click();
		await sourcePicker.selectOption(
			JSON.stringify([solid.id, solid.channels[0].reference.channel])
		);
		assert.deepEqual(await inspect(), snapshot, 'assignment remains a draft');
		await button('Assign source').click();
		let after = await wait(inspect, (value) => value.cells.length === 1);
		assert.deepEqual(after.cells[0].definition.source, solid.channels[0].reference);
		assert.equal((await playback()).desired_column, null);
		await restore(snapshot, after);
		await wait(
			() => cell.innerText(),
			(value) => value === solid.channels[0].label
		);
		const assigned = after;
		await properties.getByLabel('Find Matrix source', { exact: true }).fill(root.label);
		await button('Search sources').click();
		await sourcePicker.selectOption(JSON.stringify([root.id, root.channels[0].reference.channel]));
		const beforeError = await grid.boundingBox();
		await button('Assign source').click();
		await panel.getByRole('alert').waitFor();
		const afterError = await grid.boundingBox();
		assert(
			beforeError &&
				afterError &&
				['x', 'y', 'width', 'height'].every(
					(key) => Math.abs(beforeError[key] - afterError[key]) < 0.01
				),
			'Rejection cannot shift Matrix geometry'
		);
		assert.deepEqual(await inspect(), assigned);
		await restore(snapshot, assigned);
		stage = 'row and group settings';
		for (const row of [lower, group]) {
			await selectRow(row.label);
			for (const blend of ['Normal', 'Add', 'Multiply', 'Screen']) {
				snapshot = await inspect();
				const settings = properties.getByRole('group', { name: 'Row blending', exact: true });
				const expected = snapshot.items.find((entry) => entry.id.id === row.id.id).settings;
				await wait(
					async () => ({
						opacity: Math.fround(
							Number(await settings.getByLabel('Opacity', { exact: true }).inputValue())
						),
						blend: await settings.getByRole('combobox', { name: /^Blend/ }).inputValue(),
						enabled: await settings.getByLabel('Enabled', { exact: true }).isChecked()
					}),
					(value) => isDeepStrictEqual(value, expected)
				);
				await settings.getByLabel('Opacity', { exact: true }).fill('0.5');
				await settings.getByRole('combobox', { name: /^Blend/ }).selectOption(blend);
				await settings.getByLabel('Enabled', { exact: true }).uncheck();
				assert.deepEqual(await inspect(), snapshot);
				await button('Save row settings').click();
				after = await wait(inspect, (value) => {
					const settings = value.items.find((entry) => entry.id.id === row.id.id).settings;
					return settings.opacity === 0.5 && settings.blend === blend && !settings.enabled;
				});
				assert.deepEqual(after, {
					...snapshot,
					items: snapshot.items.map((entry) =>
						entry.id.id === row.id.id
							? { ...entry, settings: { opacity: 0.5, blend, enabled: false } }
							: entry
					)
				});
				await restore(snapshot, after);
			}
		}
		stage = 'canvas and transitions';
		const matrixSettings = properties.getByRole('group', { name: 'Matrix settings', exact: true });
		snapshot = await inspect();
		await matrixSettings.getByLabel('Canvas width', { exact: true }).fill('320');
		await matrixSettings.getByLabel('Canvas height', { exact: true }).fill('240');
		await matrixSettings.getByLabel('Duration (ms)', { exact: true }).fill('350');
		await matrixSettings.getByRole('combobox', { name: /^Curve/ }).selectOption('SmoothStep');
		assert.deepEqual(await inspect(), snapshot);
		await button('Save Matrix settings').click();
		after = await wait(inspect, (value) => value.definition.canvas.width === 320);
		assert.deepEqual(after.definition, {
			...snapshot.definition,
			canvas: { width: 320, height: 240 },
			transition: { duration_ms: 350, curve: 'SmoothStep' }
		});
		await restore(snapshot, after);
		await selectColumn(first.label);
		snapshot = after;
		// History receipt precedes the Inspector's bounded viewport refresh.
		await wait(
			async () => ({
				width: Number(
					await matrixSettings.getByLabel('Canvas width', { exact: true }).inputValue()
				),
				height: Number(
					await matrixSettings.getByLabel('Canvas height', { exact: true }).inputValue()
				),
				duration: Number(
					await matrixSettings.getByLabel('Duration (ms)', { exact: true }).inputValue()
				),
				curve: await matrixSettings.getByRole('combobox', { name: /^Curve/ }).inputValue()
			}),
			(value) =>
				isDeepStrictEqual(value, { width: 320, height: 240, duration: 350, curve: 'SmoothStep' })
		);
		const transition = properties.getByRole('group', { name: 'Column transition', exact: true });
		await transition.getByLabel('Override Matrix transition', { exact: true }).check();
		await transition.getByLabel('Duration (ms)', { exact: true }).fill('100');
		await transition.getByRole('combobox', { name: /^Curve/ }).selectOption('Linear');
		assert.equal(
			await transition.getByLabel('Override Matrix transition', { exact: true }).isChecked(),
			true
		);
		assert.equal(await transition.getByLabel('Duration (ms)', { exact: true }).inputValue(), '100');
		await button('Save column transition').click();
		after = await wait(
			inspect,
			(value) => value.columns[0].definition.transition?.duration_ms === 100
		);
		await restore(snapshot, after);
		stage = 'row column names and exported identities';
		await selectRow(lower.label);
		await nameForm('Row name').getByLabel('Row name', { exact: true }).fill('UI Lower');
		await nameForm('Row name').getByRole('button', { name: 'Rename', exact: true }).click();
		await wait(
			inspect,
			(value) => value.items.find((entry) => entry.id.id === lower.id.id).label === 'UI Lower'
		);
		await selectColumn(first.label);
		await nameForm('Column name').getByLabel('Column name', { exact: true }).fill('UI First');
		await nameForm('Column name').getByRole('button', { name: 'Rename', exact: true }).click();
		await wait(inspect, (value) => value.columns[0].label === 'UI First');
		await selectRow(group.label);
		await nameForm('Row name').getByLabel('Row name', { exact: true }).fill('UI Group');
		await nameForm('Row name').getByRole('button', { name: 'Rename', exact: true }).click();
		await wait(
			inspect,
			(value) => value.items.find((entry) => entry.id.id === group.id.id).label === 'UI Group'
		);
		for (const [label, action, predicate] of [
			['UI Lower', 'Copy row', (before, value) => value.items.length === before.items.length + 1],
			['UI Group', 'Copy row', (before, value) => value.items.length === before.items.length + 2],
			['UI Lower', 'Remove row', (before, value) => value.items.length === before.items.length - 1],
			['UI Group', 'Remove row', (before, value) => value.items.length === before.items.length - 2]
		]) {
			await selectRow(label);
			snapshot = await inspect();
			await button(action).click();
			after = await wait(inspect, (value) => predicate(snapshot, value));
			await restore(snapshot, after, true);
		}
		await selectColumn('UI First');
		for (const action of ['Copy column', 'Remove column']) {
			snapshot = await inspect();
			await button(action).click();
			after = await wait(
				inspect,
				(value) =>
					value.columns.length === snapshot.columns.length + (action === 'Copy column' ? 1 : -1)
			);
			await restore(snapshot, after, true);
			await selectColumn('UI First');
		}
		stage = 'move row into out of group and reorder columns';
		await selectRow('UI Lower');
		snapshot = await inspect();
		const moveRow = properties.getByRole('group', { name: 'Move row', exact: true });
		await moveRow
			.getByRole('combobox', { name: /^Destination group/ })
			.selectOption({ label: 'UI Group' });
		await button('Move row').click();
		after = await wait(
			inspect,
			(value) => value.items.find((entry) => entry.id.id === lower.id.id).parent === group.id.id
		);
		assert.deepEqual(
			after.items.find((entry) => entry.id.id === lower.id.id).channel,
			lower.channel
		);
		await restore(snapshot, after);
		await moveRow.getByRole('combobox', { name: /^Destination group/ }).selectOption('');
		await button('Move row').click();
		await wait(
			inspect,
			(value) => value.items.find((entry) => entry.id.id === lower.id.id).parent === null
		);
		await selectColumn('UI First');
		snapshot = await inspect();
		await properties
			.getByRole('group', { name: 'Reorder column', exact: true })
			.getByRole('combobox', { name: /^Place after column/ })
			.selectOption({ label: second.label });
		await button('Move column').click();
		after = await wait(inspect, (value) => value.columns[1].id === first.id);
		await restore(snapshot, after);
		stage = 'requested playback is distinct from selection and rendering';
		const document = await inspect();
		await button('Cut').click();
		let live = await wait(playback, (value) => value.desired_column === first.id);
		const feedback = properties.getByRole('region', {
			name: 'Matrix runtime feedback',
			exact: true
		});
		await wait(
			() => feedback.innerText(),
			(value) => value.includes('Requested column: UI First')
		);
		assert(
			(await feedback.innerText()).includes(
				'No native Matrix observation. A launch request does not confirm rendering.'
			)
		);
		await selectColumn(second.label);
		assert.equal((await playback()).launch_id, live.launch_id, 'selection cannot relaunch');
		await button('Launch column').click();
		live = await wait(playback, (value) => value.desired_column === second.id);
		assert.deepEqual(await inspect(), document, 'runtime launch cannot author document history');
		await button('Unload Matrix').click();
		await wait(playback, (value) => value.desired_column === null);
		assert.deepEqual(await inspect(), document);
		stage = 'missing source and panel lifetime';
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.locator('section[aria-label="Media Manager"]')
			.getByRole('button', { name: solid.label, exact: true })
			.click();
		await properties.getByRole('button', { name: 'Delete media', exact: true }).click();
		await wait(inspectMedia, (value) => !value.media.some((entry) => entry.id === solid.id));
		await tab.click();
		await grid.getByTitle(`UI Lower / UI First`, { exact: true }).click();
		await wait(
			() => properties.innerText(),
			(value) => value.includes('Assigned source: Missing source')
		);
		assert.deepEqual(await inspect(), document);
		await undo();
		await wait(inspectMedia, (value) => value.media.some((entry) => entry.id === solid.id));
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		await tab.click();
		await ready();
		assert.deepEqual(await inspect(), document);
		assert.equal((await playback()).desired_column, null);
		stage = 'virtualized keyboard scroll and resize';
		for (let count = document.items.length + 1; count <= 24; count++) {
			await button('Add layer').click();
			await wait(inspect, (value) => value.items.length === count);
		}
		for (let count = document.columns.length + 1; count <= 16; count++) {
			await button('Add column').click();
			await wait(inspect, (value) => value.columns.length === count);
		}
		const large = await inspect();
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(value) => value === '25'
		);
		await wait(
			() => grid.getAttribute('aria-colcount'),
			(value) => value === '17'
		);
		await grid.scrollIntoViewIfNeeded();
		await grid.focus();
		const beforeScroll = await grid.evaluate((element) => ({
			top: element.scrollTop,
			left: element.scrollLeft
		}));
		for (let count = 0; count < 4; count++) await page.keyboard.press('PageDown');
		for (let count = 0; count < 10; count++) await page.keyboard.press('ArrowRight');
		await wait(
			() => grid.evaluate((element) => ({ top: element.scrollTop, left: element.scrollLeft })),
			(value) => value.top > beforeScroll.top && value.left > beforeScroll.left
		);
		const indices = await wait(
			() =>
				grid
					.getByRole('row')
					.evaluateAll((elements) =>
						elements.map((element) => Number(element.getAttribute('aria-rowindex')))
					),
			(value) => Math.min(...value.filter((index) => index > 1)) > 2
		);
		const rendered = await grid.getByRole('gridcell').count();
		assert(rendered > 0 && rendered < 24 * 16, 'DOM renders a bounded visible window');
		assert.deepEqual(await inspect(), large);
		assert.equal((await playback()).desired_column, null, 'scrolling cannot launch');
		stage = 'offscreen selection retains usable Inspector and history';
		assert.equal(await grid.getByTitle('Select UI Lower', { exact: true }).count(), 0);
		assert.equal(await grid.getByTitle('Select UI First', { exact: true }).count(), 0);
		await wait(
			() => properties.getByLabel('Row name', { exact: true }).inputValue(),
			(value) => value === 'UI Lower'
		);
		await wait(
			() => properties.getByLabel('Column name', { exact: true }).inputValue(),
			(value) => value === 'UI First'
		);
		await wait(
			() => properties.innerText(),
			(value) => value.includes(`Assigned source: ${solid.channels[0].label}`)
		);
		await nameForm('Row name').getByLabel('Row name', { exact: true }).fill('Offscreen edited');
		await nameForm('Row name').getByRole('button', { name: 'Rename', exact: true }).click();
		const offscreenEdit = await wait(
			inspect,
			(value) =>
				value.items.find((entry) => entry.id.id === lower.id.id).label === 'Offscreen edited'
		);
		await restore(large, offscreenEdit, true);
		await wait(
			() => properties.getByLabel('Row name', { exact: true }).inputValue(),
			(value) => value === 'UI Lower'
		);
		assert.equal(
			await grid.getByTitle('Select UI Lower', { exact: true }).count(),
			0,
			'Inspector editing never scrolls the grid'
		);
		await page.setViewportSize({ width: 2560, height: 1200 });
		await ready();
		await grid.scrollIntoViewIfNeeded();
		assert.deepEqual(await inspect(), large);
		await page.setViewportSize({ width: 1440, height: 1000 });
		await grid.scrollIntoViewIfNeeded();
		const geometry = await grid.boundingBox();
		assert(
			geometry && geometry.width > 224,
			'narrow dock retains room beyond the sticky row labels'
		);
		const finalMedia = await inspectMedia();
		assert.deepEqual(finalMedia.assets, originalMedia.assets);
		assert.deepEqual(
			finalMedia.media.filter((entry) => entry.id !== root.id),
			originalMedia.media
		);
		assert.deepEqual(errors, []);
		assert.equal(rejected.length, 1, 'Only the feedback source assignment rejects');
		assert(
			rejected.every(
				(item) =>
					item.request?.stage === 'source assignment history and feedback rejection' &&
					item.request.intent?.kind === 'invokeAppOperation' &&
					item.request.intent.namespace === 'mapgyver.media' &&
					item.request.intent.payload?.kind === 'Matrix' &&
					item.request.intent.payload.edit?.kind === 'SetCells'
			)
		);
		await page.screenshot({ path: 'target/m07-matrix-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual Matrix grid/selection/source/settings/structure/history/requested-playback/unavailable-renderer/source-loss/scroll/resize/panel input; native imagery and remaining complete-editor checklist unverified',
				browser: browser.version(),
				media: root.id,
				rows: large.items.length,
				columns: large.columns.length,
				offscreen_inspector_edit_history: 'pass',
				rendered_cells: rendered,
				visible_row_indices: indices,
				grid_geometry: geometry,
				final_launch_id: live.launch_id,
				page_errors: errors.length,
				expected_feedback_rejections: rejected.length,
				screenshot: 'target/m07-matrix-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				page_errors: errors,
				rejected_requests: rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-4500)
			})
		);
		await page.screenshot({ path: 'target/m07-matrix-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
