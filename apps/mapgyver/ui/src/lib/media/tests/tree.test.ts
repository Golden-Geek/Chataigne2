import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { UiNodeDto } from 'golden_ui';
import { isMediaTreeNode, canRenderMediaTreeChildren } from '../tree.ts';

test('media manager admits real domain hierarchy and hides parameter/channel presentation', () => {
	for (const type of [
		'mapgyver.media.v8',
		'mapgyver.media.reference.v3',
		'mapgyver.media.folder',
		'mapgyver.matrix.layer.v3',
		'mapgyver.matrix.group.v2',
		'mapgyver.matrix.cell.v3',
		'mapgyver.matrix.column.v2',
		'mapgyver.matrix.column_group.v1'
	])
		assert(isMediaTreeNode({ node_type: type } as UiNodeDto), type);
	for (const type of ['parameter', 'folder', 'mapgyver.composition.placement.v2']) {
		const node = { node_type: type } as UiNodeDto;
		assert.equal(isMediaTreeNode(node), false, type);
		assert.equal(canRenderMediaTreeChildren(node), false, type);
	}
	const channel = { node_type: 'mapgyver.media.channel' } as UiNodeDto;
	assert.equal(isMediaTreeNode(channel), false);
	assert.equal(canRenderMediaTreeChildren(channel), true);
});
