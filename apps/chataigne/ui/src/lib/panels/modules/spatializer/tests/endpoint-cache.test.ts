import { describe, expect, it } from 'vitest';
import { fixtureNode } from 'golden_ui/testing/session';
import { SpatializerEndpointCache } from '../endpoint-cache';
import type { SpatialEndpoint } from '../types';

const endpoint = (id: number): SpatialEndpoint => ({
	key: `source:${id}`,
	kind: 'source',
	node: fixtureNode(id, 'Source'),
	valueDeclId: 'source',
	positionParam: fixtureNode(id + 100, 'Position'),
	radiusParam: null,
	freezeRadiusParam: null,
	positionKind: 'vec2',
	middle: 0,
	x: 0,
	y: 0,
	radius: null,
	freezeRadius: null,
	color: 'red',
	enabled: true,
	positionWritable: true,
	radiusWritable: false,
	freezeRadiusWritable: false
});

describe('Spatializer endpoint presentation identity', () => {
	it('retains geometry input identity across unrelated computed-value updates', () => {
		const cache = new SpatializerEndpointCache(),
			a = endpoint(1),
			b = endpoint(2);
		const first = cache.reconcile([a, b], 1);
		expect(cache.reconcile([{ ...a }, { ...b }], 1)).toBe(first);
		const moved = { ...b, positionParam: { ...b.positionParam! }, x: 1 };
		const next = cache.reconcile([{ ...a }, moved], 1);
		expect(next).not.toBe(first);
		expect(next[0]).toBe(a);
		expect(next[1]).toBe(moved);
	});
	it('publishes metadata, radius and ordered palette changes', () => {
		const cache = new SpatializerEndpointCache(),
			a = endpoint(1);
		cache.reconcile([a], 1);
		const renamed = { ...a, node: { ...a.node, meta: { ...a.node.meta, label: 'Renamed' } } };
		expect(cache.reconcile([renamed], 1)[0]).toBe(renamed);
		const radius = { ...renamed, radiusParam: fixtureNode(99, 'Radius'), radius: 2 };
		expect(cache.reconcile([radius], 1)[0]).toBe(radius);
		const colored = { ...radius, color: 'blue' };
		expect(cache.reconcile([colored], 1)[0]).toBe(colored);
	});
	it('keeps endpoint order and retires deletion and snapshot identities', () => {
		const cache = new SpatializerEndpointCache(),
			a = endpoint(1),
			b = endpoint(2);
		const first = cache.reconcile([a, b], 1);
		expect(cache.reconcile([{ ...b }, { ...a }], 1)).toEqual([b, a]);
		expect(cache.reconcile([{ ...b }], 1)).toEqual([b]);
		const replacement = { ...b };
		expect(cache.reconcile([replacement], 2)[0]).toBe(replacement);
		expect(cache.reconcile([], 2)).toEqual([]);
		expect(cache.reconcile([{ ...a }, { ...b }], 3)).not.toBe(first);
	});
});
