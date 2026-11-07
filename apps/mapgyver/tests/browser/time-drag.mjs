import assert from 'node:assert/strict';
import path from 'node:path';

export async function verifyTimeDragging({
	page,
	inspect,
	edit,
	post,
	select,
	parameter,
	column,
	cell,
	row,
	wait
}) {
	const checks = [];
	const clock = () => parameter('Transition Time').locator('input.number-field');
	const slider = (label = 'Transition Time') => parameter(label).getByRole('slider');
	const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const duration = async () =>
		(await inspect()).cells.find((value) => value.id === cell).definition.transition.duration_ms;
	async function drag(target, dx, { cancel = false, live } = {}) {
		const bounds = await target.boundingBox();
		assert(bounds);
		const x = bounds.x + bounds.width / 2,
			y = bounds.y + bounds.height / 2;
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x + dx, y, { steps: 10 });
		if (live) await live();
		if (cancel) await page.keyboard.press('Escape');
		await page.mouse.up();
		await wait(snapshot, (value) => !value.history.active_edit_session);
	}
	await select(cell, 'Cell');
	const history = (await snapshot()).history.undo_len;
	await drag(slider(), 50, {
		live: async () => {
			await wait(duration, (value) => value > 0);
			assert(
				(await snapshot()).history.active_edit_session,
				'drag values reach the real engine before release'
			);
		}
	});
	const changed = await duration();
	assert.equal(changed, 50);
	assert.equal(
		(await snapshot()).history.undo_len,
		history + 1,
		'one gesture, one backend history item'
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(duration, (value) => value === 0);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(duration, (value) => value === changed);
	checks.push({
		time_drag: 'Cell',
		zero_to_positive: changed,
		live_before_release: true,
		one_history_item: true,
		undo_redo: true
	});
	await drag(slider(), 25, {
		cancel: true,
		live: () => wait(duration, (value) => value > changed)
	});
	await wait(duration, (value) => value === changed);
	assert.equal(await clock().inputValue(), '00:00:00.050');
	checks.push({ escape_restores_real_parameter: true, group_closed: true });
	for (const invalid of ['not-a-clock', '-00:00:01.000', '999999999999999:00:00']) {
		await clock().fill(invalid);
		await clock().press('Enter');
		assert.equal(await duration(), changed);
		assert.equal(await clock().inputValue(), '00:00:00.050');
	}
	await slider().press('ArrowRight');
	await wait(duration, (value) => value === changed + 1);
	checks.push({ invalid_and_out_of_range_entry_restored: true, keyboard_step_ms: 1 });
	await select(row, 'Layer 1');
	const layerDuration = async () =>
		(await inspect()).items.find((item) => item.id.id === row).layer.duration_ms;
	await drag(slider(), 30);
	await wait(layerDuration, (value) => value === 730);
	checks.push({ time_drag: 'Layer', value_ms: 730 });
	await select(column, 'Column 1');
	const toggle = parameter('Transition Time').getByRole('switch', {
		name: 'Toggle node enabled',
		exact: true
	});
	await toggle.click();
	await wait(inspect, (state) => state.columns[0].definition.transition !== null);
	await drag(slider(), 30);
	await wait(inspect, (state) => state.columns[0].definition.transition.duration_ms === 1264);
	checks.push({ time_drag: 'Column', disabled_dormant_value_retained: true, value_ms: 1264 });
	// App Control times are different real float Parameters, with tighter domain bounds.
	const executable = path.resolve('target/debug/application_process_fixture.exe');
	await parameter('App file').locator('input.path-input').fill(executable);
	await parameter('App file').locator('input.path-input').press('Tab');
	await parameter('Graceful stop').waitFor();
	const app = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: column } })).snapshot;
	await drag(slider('Graceful stop'), 20);
	await wait(app, (value) => value.definition.stop.graceful_ms === 2020);
	await parameter('Graceful stop').locator('input.number-field').fill('00:10:00.000');
	await parameter('Graceful stop').locator('input.number-field').press('Enter');
	await wait(app, (value) => value.definition.stop.graceful_ms === 600000);
	await drag(slider('Graceful stop'), 50);
	assert.equal((await app()).definition.stop.graceful_ms, 600000);
	checks.push({ time_drag: 'App Control', value_and_upper_bound_ms: [2020, 600000] });
	// Golden intentionally clears editable content for a multi-selection; the control must
	// not keep writing the previously selected Parameter when that happens.
	const saved = await inspect();
	const manager = page.locator('section[data-panel-id="media-manager"]');
	await manager.getByPlaceholder('Search media…').fill('Column');
	const other = (await snapshot()).nodes.find((node) => node.uuid === saved.columns[1].id);
	await manager
		.locator(`[data-tree-node-id="${other.node_id}"]`)
		.getByRole('button', { name: other.meta.label, exact: true })
		.click({ modifiers: ['Control'] });
	await wait(
		() => manager.locator('[role="treeitem"][aria-selected="true"]').count(),
		(count) => count === 2
	);
	assert.equal(await parameter('Transition Time').count(), 0);
	assert.deepEqual(await inspect(), saved);
	await select(column, 'Column 1');
	assert.equal(await clock().inputValue(), '00:00:01.264');
	checks.push({
		mixed_selection:
			'safe ordinary Golden empty Inspector; no stale writes; single selection restores'
	});
	return checks;
}
