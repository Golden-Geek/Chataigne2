import assert from 'node:assert/strict';
import { test } from 'node:test';
import { displayRect, type LayoutRect } from '../rect-presentation.ts';
import type { LayerPresentation } from '../layer-gesture';

const object: LayoutRect = {
	id: 'layer',
	label: 'Layer',
	enabled: true,
	position: [10, 20],
	size: { width: 100, height: 80 }
};
const draft = (mode: LayerPresentation['handle']['mode']): LayerPresentation => ({
	reference: { media: 'matrix', layer: 'layer' },
	handle: {
		mode,
		origin: { position: [10, 20], size: { width: 100, height: 80 } },
		start: { x: 0, y: 0 }
	},
	sample: { x: -1.25, y: -2.75 }
});
test('Matrix control hints use immutable gesture bases and do not round or validate authored dimensions', () => {
	const saved = structuredClone(object);
	assert.deepEqual(displayRect(object, draft('Translate')).position, [8.75, 17.25]);
	const resize = displayRect(object, draft({ Resize: { handle: 'TopLeft' } }));
	assert.deepEqual(resize.position, [8.75, 17.25]);
	assert.deepEqual(resize.size, { width: 101.25, height: 82.75 });
	assert.deepEqual(object, saved);
	const large = draft({ Resize: { handle: 'BottomRight' } });
	large.sample.x = 20000;
	assert.equal(displayRect(object, large).size?.width, 20100, 'domain limits are not UI policy');
	assert.equal(displayRect({ ...object, id: 'other' }, large).id, 'other');
});
test('a pending Cell translation preserves unknown natural size and cannot rewind to an old receipt', () => {
	const pending = {
		reference: { media: 'matrix', cell: 'cell' },
		handle: {
			kind: 'Translate' as const,
			origin: [10, 20] as [number, number],
			start: { x: 0, y: 0 }
		},
		sample: { x: 8, y: 4 }
	};
	const old = { ...object, id: 'cell', position: [11, 21] as [number, number], size: null };
	assert.deepEqual(displayRect(old, pending), { ...old, position: [18, 24] });
	assert.deepEqual(displayRect({ ...old, position: [12, 22] }, pending).position, [18, 24]);
});
