import assert from 'node:assert/strict';
import { test } from 'node:test';
import { outline, gizmo } from '../presentation.ts';

test('source outlines match the mirrored/rotated renderer reference and gizmos remain usable without metadata', () => {
	const transform = {
		position: [4, 4] as [number, number],
		scale: [-2, 4] as [number, number],
		rotation: Math.PI / 2,
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
	const handles = gizmo(transform, { width: 8, height: 8 }, { width: 4, height: 2 });
	assert.deepEqual(handles.pivot, { x: 4, y: 4 });
	assert.ok(points.some((point) => point.x === handles.scale.x && point.y === handles.scale.y));
	const missing = gizmo(transform, { width: 8, height: 8 });
	assert.notDeepEqual(missing.scale, missing.pivot);
	assert.notDeepEqual(missing.rotate, missing.pivot);
});
