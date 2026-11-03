import assert from 'node:assert/strict';
import { test } from 'node:test';
import { outline, gizmo } from '../presentation.ts';

test('real bounds project eight resize and four outside rotation handles, including mirrored local axes', () => {
	const transform = {
		position: [4, 4] as [number, number],
		scale: [-2, 4] as [number, number],
		rotation_degrees: 90,
		pivot: [0.25, 0.5] as [number, number]
	};
	const points = outline(transform, { width: 4, height: 2 });
	for (const [index, expected] of [
		[8, 6],
		[8, -2],
		[0, -2],
		[0, 6]
	].entries()) {
		assert.ok(Math.abs(points[index].x - expected[0]) < 1e-5);
		assert.ok(Math.abs(points[index].y - expected[1]) < 1e-5);
	}
	const handles = gizmo(transform, { width: 4, height: 2 });
	assert.deepEqual(handles.pivot, { x: 4, y: 4 });
	assert.equal(handles.resize.length, 8);
	assert.equal(handles.rotate.length, 4);
	for (const [index, handle] of handles.rotate.entries()) {
		assert.deepEqual(handle.point, points[index]);
		assert.ok(Math.abs(Math.hypot(handle.offset.x, handle.offset.y) - 1) < 1e-10);
	}
	assert.equal(handles.resize.find((handle) => handle.name === 'Right')?.cursor, 'ns-resize');
	assert.equal(handles.resize.find((handle) => handle.name === 'Top')?.cursor, 'ew-resize');
	const missing = gizmo(transform);
	assert.deepEqual(missing, { pivot: { x: 4, y: 4 }, resize: [], rotate: [] });
});
