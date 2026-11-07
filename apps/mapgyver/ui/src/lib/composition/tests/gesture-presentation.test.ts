import assert from 'node:assert/strict';
import { test } from 'node:test';
import { instanceDisplay } from '../presentation.ts';
import type { InstanceSnapshot } from '../../media/generated/InstanceSnapshot';
import type { InstancePresentation } from '../instance-gesture';

const instance: InstanceSnapshot = {
	reference: { composition: { media: 'media', channel: 'channel' }, instance: 'instance' },
	label: 'Instance',
	definition: {
		source: null,
		opacity: 1,
		enabled: true,
		blend: 'Normal',
		transform: { position: [100, 100], scale: [1, 1], pivot: [0.5, 0.5], rotation_degrees: 0 }
	}
};
const extent = { width: 40, height: 20 };
const hint = (
	mode: InstancePresentation['handle']['mode'],
	start: { x: number; y: number },
	sample: { x: number; y: number }
): InstancePresentation => ({
	reference: instance.reference,
	handle: { origin: structuredClone(instance.definition.transform), mode, start },
	sample
});
const close = (a: { x: number; y: number }, b: { x: number; y: number }) => {
	assert(Math.abs(a.x - b.x) < 1e-8);
	assert(Math.abs(a.y - b.y) < 1e-8);
};
test('Composition translation/rotation/anchored resize transform only display vertices', () => {
	const before = structuredClone(instance);
	const translated = instanceDisplay(
		instance,
		extent,
		hint('Translate', { x: 0, y: 0 }, { x: 7, y: -3 })
	);
	close(translated.points[0], { x: 87, y: 87 });
	close(translated.handles.pivot, { x: 107, y: 97 });
	const rotated = instanceDisplay(
		instance,
		extent,
		hint('Rotate', { x: 120, y: 100 }, { x: 100, y: 120 })
	);
	close(rotated.points[0], { x: 110, y: 80 });
	close(rotated.handles.pivot, { x: 100, y: 100 });
	const resized = instanceDisplay(
		instance,
		extent,
		hint({ Resize: { handle: 'TopLeft', extent } }, { x: 80, y: 90 }, { x: 70, y: 85 })
	);
	close(resized.points[0], { x: 70, y: 85 });
	close(resized.points[2], { x: 120, y: 110 });
	close(resized.handles.pivot, { x: 95, y: 97.5 });
	assert.deepEqual(instance, before);
});
test('rotated resize keeps its opposite world corner fixed and survives older accepted transforms', () => {
	const pending = hint(
		{ Resize: { handle: 'TopLeft', extent } },
		{ x: 110, y: 60 },
		{ x: 115, y: 40 }
	);
	pending.handle.origin.rotation_degrees = 90;
	pending.handle.origin.scale = [2, 1];
	const a = instanceDisplay(instance, extent, pending);
	close(a.points[0], { x: 115, y: 40 });
	close(a.points[2], { x: 90, y: 140 });
	const late = structuredClone(instance);
	late.definition.transform.position = [120, 130];
	assert.deepEqual(instanceDisplay(late, extent, pending), a);
	const collapsed = hint(
		{ Resize: { handle: 'BottomRight', extent } },
		{ x: 0, y: 0 },
		{ x: -40, y: -20 }
	);
	assert(
		instanceDisplay(instance, extent, collapsed).handles.rotate.every((h) =>
			Number.isFinite(h.offset.x + h.offset.y)
		)
	);
});
