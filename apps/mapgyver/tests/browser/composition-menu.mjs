/** Exercise the ordinary node menu from the editor, including selected transform handles. */
export async function compositionNodeMenuAction(page, panel, action, submenu = null) {
	const handle = panel.locator('[data-instance-handle="Translate"][aria-disabled="false"]');
	await handle.waitFor();
	await handle.evaluate((element) =>
		element.dispatchEvent(
			new MouseEvent('contextmenu', { bubbles: true, clientX: 500, clientY: 350 })
		)
	);
	if (submenu) await page.getByRole('menuitem', { name: submenu, exact: true }).hover();
	await page.getByRole('menuitem', { name: new RegExp(`^${action}(?:\\s|$)`) }).click();
}
