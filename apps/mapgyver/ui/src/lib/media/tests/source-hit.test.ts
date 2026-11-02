import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pickCompositionSource, pickSurfaceSource } from '../source-hit.ts';
import type { SurfaceSnapshot } from '../../screens/generated/SurfaceSnapshot';
import type { InstanceSnapshot } from '../generated/InstanceSnapshot';
import type { Point } from '../generated/Point';
import type { Transform2D } from '../generated/Transform2D';

const source = { media: 'media', channel: 'channel' };
const canvas = { width: 100, height: 100 };
const extent = () => ({ width: 10, height: 20 });
function quad(left = 0, top = 0, width = 1, height = 1): [Point, Point, Point, Point] {
	return [
		{ x: left, y: top },
		{ x: left + width, y: top },
		{ x: left + width, y: top + height },
		{ x: left, y: top + height }
	];
}
function surface(id: string, corners = quad()): SurfaceSnapshot {
	return {
		reference: { screen: 'screen', surface: id },
		label: id,
		definition: {
			source,
			crop: { x: 0, y: 0, width: 1, height: 1 },
			quad: { corners },
			opacity: 1,
			calibration: 'Off',
			enabled: true
		}
	};
}
function instance(id: string, transform: Partial<Transform2D> = {}): InstanceSnapshot {
	return {
		reference: { composition: source, instance: id },
		label: id,
		definition: {
			source,
			transform: {
				position: [50, 50],
				scale: [1, 1],
				rotation: 0,
				pivot: [0.5, 0.5],
				...transform
			},
			opacity: 1,
			blend: 'Normal',
			enabled: true
		}
	};
}

test('Surface picking covers objects beyond 128 overlays and follows reverse paint order', () => {
	const surfaces = Array.from({ length: 129 }, (_, index) =>
		surface(`surface-${index}`, quad(0, 0, 0.1, 0.1))
	);
	surfaces[128] = surface('beyond-overlay', quad(0.6, 0.6, 0.3, 0.3));
	assert.equal(pickSurfaceSource({ x: 0.75, y: 0.75 }, surfaces), 'beyond-overlay');
	surfaces.push(surface('topmost', quad(0.7, 0.7, 0.2, 0.2)));
	assert.equal(pickSurfaceSource({ x: 0.75, y: 0.75 }, surfaces), 'topmost');
	assert.equal(
		pickSurfaceSource({ x: 0.6, y: 0.6 }, surfaces),
		'beyond-overlay',
		'polygon corners are included'
	);
	assert.equal(
		pickSurfaceSource({ x: 0.6, y: 0.75 }, surfaces),
		'beyond-overlay',
		'polygon edges are included'
	);
});

test('Surface picking clips to the Screen and rejects invalid or nonconvex outlines', () => {
	const clipped = surface('clipped', quad(-1, -1, 3, 3));
	assert.equal(pickSurfaceSource({ x: 0, y: 0 }, [clipped]), 'clipped');
	assert.equal(pickSurfaceSource({ x: 1, y: 1 }, [clipped]), 'clipped');
	for (const point of [
		{ x: -0.01, y: 0.5 },
		{ x: 1.01, y: 0.5 },
		{ x: Infinity, y: 0.5 },
		{ x: 0.5, y: NaN }
	]) {
		assert.equal(pickSurfaceSource(point, [clipped]), null);
	}
	const invalid = [
		quad(0, 0, 0, 1),
		[
			{ x: 0, y: 0 },
			{ x: 1, y: 0 },
			{ x: 0.25, y: 0.25 },
			{ x: 0, y: 1 }
		],
		[
			{ x: 0, y: 0 },
			{ x: 1, y: 1 },
			{ x: 0, y: 1 },
			{ x: 1, y: 0 }
		],
		[
			{ x: NaN, y: 0 },
			{ x: 1, y: 0 },
			{ x: 1, y: 1 },
			{ x: 0, y: 1 }
		]
	] as [Point, Point, Point, Point][];
	for (const corners of invalid)
		assert.equal(pickSurfaceSource({ x: 0.1, y: 0.1 }, [surface('invalid', corners)]), null);
	const reversed = surface('mirrored', [...quad()].reverse() as [Point, Point, Point, Point]);
	assert.equal(pickSurfaceSource({ x: 0.5, y: 0.5 }, [reversed]), 'mirrored');
});

test('Composition picking covers later instances and resolves the topmost overlapping source body', () => {
	const instances = Array.from({ length: 129 }, (_, index) =>
		instance(`instance-${index}`, { position: [5, 5] })
	);
	instances[128] = instance('beyond-overlay');
	assert.equal(
		pickCompositionSource({ x: 50, y: 50 }, { canvas, instances }, extent),
		'beyond-overlay'
	);
	instances.push(instance('topmost'));
	assert.equal(pickCompositionSource({ x: 50, y: 50 }, { canvas, instances }, extent), 'topmost');
	assert.equal(pickCompositionSource({ x: 55, y: 50 }, { canvas, instances }, extent), 'topmost');
	assert.equal(pickCompositionSource({ x: 55, y: 60 }, { canvas, instances }, extent), 'topmost');
});

test('Composition picking follows rotated mirrored geometry rather than its axis-aligned bounds', () => {
	const instances = [instance('transformed', { scale: [-2, 1], rotation: Math.PI / 4 })];
	assert.equal(
		pickCompositionSource({ x: 50, y: 50 }, { canvas, instances }, extent),
		'transformed'
	);
	assert.equal(
		pickCompositionSource({ x: 59, y: 59 }, { canvas, instances }, extent),
		null,
		'inside AABB but outside rotated body'
	);
	const corner = { x: 50 + 10 * Math.SQRT2, y: 50 };
	assert.equal(pickCompositionSource(corner, { canvas, instances }, extent), 'transformed');
});

test('Composition body picking rejects absent metadata, invalid transforms and points outside the canvas', () => {
	const instances = [instance('body', { position: [0, 0] })];
	assert.equal(pickCompositionSource({ x: 0, y: 0 }, { canvas, instances }, extent), 'body');
	for (const point of [
		{ x: -1, y: 0 },
		{ x: 101, y: 0 },
		{ x: 0, y: NaN },
		{ x: Infinity, y: 0 }
	]) {
		assert.equal(pickCompositionSource(point, { canvas, instances }, extent), null);
	}
	assert.equal(
		pickCompositionSource({ x: 0, y: 0 }, { canvas, instances }, () => null),
		null
	);
	for (const invalid of [
		{ width: 0, height: 20 },
		{ width: 10, height: NaN },
		{ width: Infinity, height: 20 }
	]) {
		assert.equal(
			pickCompositionSource({ x: 0, y: 0 }, { canvas, instances }, () => invalid),
			null
		);
		assert.equal(
			pickCompositionSource({ x: 0, y: 0 }, { canvas: invalid, instances }, extent),
			null
		);
	}
	for (const transform of [{ scale: [0, 1] }, { rotation: NaN }, { position: [Infinity, 0] }]) {
		assert.equal(
			pickCompositionSource(
				{ x: 50, y: 50 },
				{ canvas, instances: [instance('invalid', transform as Partial<Transform2D>)] },
				extent
			),
			null
		);
	}
	const missing = instance('source-less');
	missing.definition.source = null;
	assert.equal(
		pickCompositionSource({ x: 50, y: 50 }, { canvas, instances: [missing] }, extent),
		null
	);
});

test('one pick observes each shared source extent once, including unavailable sources', () => {
	const instances = Array.from({ length: 200 }, (_, index) =>
		instance(`shared-${index}`, { position: [0, 0] })
	);
	for (const observed of [extent(), null]) {
		let reads = 0;
		const result = pickCompositionSource({ x: 90, y: 90 }, { canvas, instances }, () => {
			reads++;
			return observed;
		});
		assert.equal(result, null);
		assert.equal(reads, 1);
	}
});
