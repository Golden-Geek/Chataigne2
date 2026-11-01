import { test } from 'node:test';
import assert from 'node:assert/strict';
import { sameSelection } from '../selection.ts';

test('Inspector selection matches backend replies with reordered JSON members', () => {
	const local = { item: { kind: 'Group' as const, id: 'group' }, column: null };
	const reply = { column: null, item: { id: 'group', kind: 'Group' as const } };
	assert.notEqual(JSON.stringify(local), JSON.stringify(reply));
	assert.equal(sameSelection(local, reply), true);
});

test('different selected axes, kinds and identities cannot share Inspector data', () => {
	const selected = { item: { kind: 'Layer' as const, id: 'row' }, column: 'column' };
	assert.equal(sameSelection(selected, { ...selected, column: 'other' }), false);
	assert.equal(sameSelection(selected, { ...selected, item: { kind: 'Group', id: 'row' } }), false);
	assert.equal(
		sameSelection(selected, { ...selected, item: { kind: 'Layer', id: 'other' } }),
		false
	);
	assert.equal(sameSelection(selected, { ...selected, item: null }), false);
	assert.equal(sameSelection(selected, null), false);
	assert.equal(sameSelection(null, undefined), true);
});
