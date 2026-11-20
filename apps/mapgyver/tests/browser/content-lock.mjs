import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

/** Locate the owning tab rather than a body-specific lock implementation. */
export async function contentLock(panel, locked = false) {
	const id = await panel.getAttribute('data-panel-id');
	assert(id);
	const tab = panel.page().locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
	const button = tab.getByRole('button', {
		name: locked ? 'Unlock panel content' : 'Lock panel content',
		exact: true
	});
	await button.waitFor();
	assert.equal(await button.getAttribute('aria-pressed'), String(locked));
	assert.equal(
		await button.evaluate((node) => node.nextElementSibling?.className),
		'gc-dv-tab-close'
	);
	assert.equal(await panel.locator('.lock-toggle').count(), 0);
	return button;
}

export async function persistedPanelState(page, id, expected) {
	await page.waitForFunction(
		({ id, expected }) =>
			Object.entries(localStorage).some(([key, value]) => {
				if (!key.endsWith('layout.v1')) return false;
				const state = JSON.parse(value).payload?.panels?.[id]?.params?.__gc_panel_state;
				const equal = (left, right) => {
					if (left === right) return true;
					if (!left || !right || typeof left !== 'object' || typeof right !== 'object')
						return false;
					if (Array.isArray(left) !== Array.isArray(right)) return false;
					const keys = Object.keys(left);
					return (
						keys.length === Object.keys(right).length &&
						keys.every((key) => Object.hasOwn(right, key) && equal(left[key], right[key]))
					);
				};
				return state && Object.entries(expected).every(([key, value]) => equal(state[key], value));
			}),
		{ id, expected }
	);
}

/** Seed the owned profile's recent path before reload so Reopen Last avoids platform dialogs. */
export async function seedProjectReopen(page, file) {
	await page.evaluate((file) => {
		const key = Object.keys(localStorage).find((key) => key.endsWith('layout.v1'));
		if (!key) throw new Error('Missing persisted workbench layout');
		localStorage.setItem(
			key.replace(/layout\.v1$/, 'project-path.v1'),
			JSON.stringify({ version: 1, savedAt: new Date().toISOString(), payload: file })
		);
	}, file);
}

export async function saveLockedWorkspace(page, post, file, panelIds) {
	const layout = await page.evaluate(() => {
		const key = Object.keys(localStorage).find((key) => key.endsWith('layout.v1'));
		return key ? JSON.parse(localStorage.getItem(key)).payload : null;
	});
	assert(panelIds.every((id) => layout?.panels[id]?.params?.__gc_panel_state?.locked === true));
	const saved = await post('/api/ui/project-save', {
		path: file,
		ui_state: { dock_layout: layout, selected_node_ids: [] }
	});
	assert.equal(saved.ok, true);
}

export async function reopenWorkspace(page, post, file) {
	await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
	await page.getByRole('menuitem', { name: /^Reopen Last/ }).click();
	const until = Date.now() + 7000;
	for (;;) {
		const snapshot = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		const current = snapshot.project_file.current_path;
		if (current && path.resolve(current) === file) break;
		assert(Date.now() < until, `Reopen Last did not open ${file}`);
		await delay(25);
	}
	await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
}
