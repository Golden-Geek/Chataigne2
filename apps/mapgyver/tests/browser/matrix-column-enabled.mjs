import assert from 'node:assert/strict';
import path from 'node:path';

/** Ordinary Inspector metadata, grid selection/rename, history and project persistence. */
export async function verifyMatrixColumnEnabled({
	page,
	grid,
	column,
	matrix,
	post,
	directory,
	wait
}) {
	const header = grid.locator(`[role="columnheader"][data-column-id="${column.id}"]`);
	await header.locator('button[title^="Select "]').click();
	const inspector = page.locator('.inspector');
	await wait(
		() => inspector.getAttribute('data-inspected-uuid'),
		(value) => value === column.id
	);
	const toggle = inspector
		.locator('.inspector-header')
		.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	const playback = JSON.stringify(await matrix({ kind: 'InspectPlayback' }));
	const initial = (await matrix({ kind: 'Inspect' })).snapshot.cells.map((cell) => ({
		id: cell.id,
		media: cell.media,
		definition: cell.definition
	}));
	await toggle.click();
	await wait(
		() => header.getAttribute('class'),
		(value) => value.includes('disabled-item')
	);
	assert.equal(await toggle.getAttribute('aria-checked'), 'false');
	assert.equal(
		await header.getByRole('button', { name: `Launch ${column.label}`, exact: true }).isDisabled(),
		true
	);
	assert.equal(await header.getAttribute('aria-selected'), 'true');
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => header.getAttribute('class'),
		(value) => !value.includes('disabled-item')
	);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(
		() => header.getAttribute('class'),
		(value) => value.includes('disabled-item')
	);
	await header.locator('button[title^="Select "]').dblclick();
	const rename = header.getByRole('textbox', { name: `Rename ${column.label}`, exact: true });
	await rename.fill('Muted Column');
	await rename.press('Enter');
	await header.getByRole('button', { name: 'Muted Column', exact: true }).waitFor();
	assert.equal(JSON.stringify(await matrix({ kind: 'InspectPlayback' })), playback);
	assert.deepEqual(
		(await matrix({ kind: 'Inspect' })).snapshot.cells.map((cell) => ({
			id: cell.id,
			media: cell.media,
			definition: cell.definition
		})),
		initial
	);
	const file = path.join(directory, 'Column disabled.mapgyver');
	assert((await post('/api/ui/project-save', { path: file })).ok);
	await post('/api/ui/project-load', { path: file });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Selection colors', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await wait(
		() => header.getAttribute('class'),
		(value) => value.includes('disabled-item')
	);
	await header.getByRole('button', { name: 'Muted Column', exact: true }).click();
	await wait(
		() => inspector.getAttribute('data-inspected-uuid'),
		(value) => value === column.id
	);
	await toggle.click();
	await wait(
		() => header.getAttribute('class'),
		(value) => !value.includes('disabled-item')
	);
	await page.screenshot({ path: path.join(directory, 'column-enabled-restored.png') });
	return {
		result: 'pass',
		column: column.id,
		inspectorToggle: true,
		history: true,
		disabledRename: true,
		cellsUnchanged: initial.length,
		playbackUnchanged: true,
		savedDisabledStateRestored: true,
		reenabled: true
	};
}
