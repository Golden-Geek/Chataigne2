import assert from 'node:assert/strict';

// Bespoke fixtures author their entire shape; verify then explicitly remove product defaults.
export async function clearMatrixScene(edit, media) {
	const matrix = (operation) => edit({ kind: 'Matrix', media, edit: operation });
	const { snapshot } = await matrix({ kind: 'Inspect' });
	assert.equal(snapshot.items.length, 3);
	assert.equal(snapshot.columns.length, 3);
	assert.equal(snapshot.cells.length, 0);
	for (const row of snapshot.items) await matrix({ kind: 'RemoveItem', item: row.id });
	for (const column of snapshot.columns) await matrix({ kind: 'RemoveColumn', column: column.id });
}
