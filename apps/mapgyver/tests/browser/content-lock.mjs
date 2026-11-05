import assert from 'node:assert/strict';

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
