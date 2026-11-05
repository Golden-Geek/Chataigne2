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
				return (
					state &&
					Object.entries(expected).every(
						([key, value]) => JSON.stringify(state[key]) === JSON.stringify(value)
					)
				);
			}),
		{ id, expected }
	);
}
