import { describe, expect, it } from 'vitest';
import { fixtureNode } from 'golden_ui/testing/session';
import { endpointView, spatialViewport, voronoiCandidates } from '../view';
import type { SpatialEndpoint } from '../types';

function endpoint(
	id: number,
	x = id,
	y = 0,
	kind: 'target' | 'source' = 'target'
): SpatialEndpoint {
	return {
		key: `${kind}:${id}`,
		kind,
		node: fixtureNode(id, `Endpoint ${id}`),
		valueDeclId: `v${id}`,
		positionParam: null,
		radiusParam: null,
		freezeRadiusParam: null,
		positionKind: 'vec2',
		middle: 0,
		x,
		y,
		radius: null,
		freezeRadius: null,
		color: '#fff',
		enabled: true,
		positionWritable: true,
		radiusWritable: false,
		freezeRadiusWritable: false
	};
}
const bounds = { left: -1, top: -1, right: 1, bottom: 1 };

describe('Spatializer bounded world membership', () => {
	it('includes intersecting radii and retains a focused control outside the viewport', () => {
		const radius = { ...endpoint(1, 2), radius: 1.1 },
			focused = endpoint(2, 100),
			absent = endpoint(3, 200);
		const view = endpointView([radius, focused, absent], bounds, [focused.key]);
		expect(view.overview).toBe(false);
		expect(view.visibleCount).toBe(1);
		expect(view.glyphs.map((e) => e.key)).toEqual([focused.key, radius.key]);
	});
	it('accounts for every dense endpoint, including coincident sites, without overspending retention', () => {
		const points = Array.from({ length: 8192 }, (_, id) =>
			endpoint(id, 0, 0, id % 2 ? 'source' : 'target')
		);
		const view = endpointView(points, bounds, points.map((e) => e.key).reverse());
		expect(view.overview).toBe(true);
		expect(view.glyphs).toHaveLength(128);
		expect(view.glyphs[0].key).toBe(points.at(-1)!.key);
		expect(view.groups.length).toBeLessThanOrEqual(256);
		expect(view.groups.reduce((n, g) => n + g.sources + g.targets, 0)).toBe(8192);
	});
	it('returns to complete detail after changing the viewport', () => {
		const points = Array.from({ length: 4096 }, (_, id) => endpoint(id));
		expect(endpointView(points, { ...bounds, right: 4096 }, []).overview).toBe(true);
		expect(endpointView(points, bounds, []).glyphs.map((e) => e.x)).toEqual([0, 1]);
	});
	it('converts camera pixels to spatial units independently of zoom and root font size', () => {
		const view = spatialViewport({
			camera: { x: -128, y: -64, zoom: 0.5 },
			remPx: 32,
			viewportWidth: 640,
			viewportHeight: 320,
			focusedItemId: null
		});
		expect(view).toEqual({ left: 1.5, top: 0.5, right: 12.5, bottom: 6.5 });
	});
});

describe('complete bounded Voronoi candidates', () => {
	it('includes sites outside the viewport that own part of it', () => {
		const points = [endpoint(1, -2), endpoint(2, 2), endpoint(3, 100)];
		expect(voronoiCandidates(points, bounds)?.map((e) => e.key)).toEqual([
			points[0].key,
			points[1].key
		]);
	});
	it('contains the nearest site for every sampled point across a nonuniform viewport', () => {
		const points = Array.from({ length: 1000 }, (_, id) =>
			endpoint(id, (id % 50) - 20, Math.floor(id / 50) - 10)
		);
		const candidates = new Set(voronoiCandidates(points, bounds)!.map((e) => e.key));
		for (let x = -1; x <= 1; x += 0.1)
			for (let y = -1; y <= 1; y += 0.1) {
				const nearest = points.reduce((a, b) =>
					Math.hypot(a.x - x, a.y - y) <= Math.hypot(b.x - x, b.y - y) ? a : b
				);
				expect(candidates.has(nearest.key)).toBe(true);
			}
	});
	it('reports an overview instead of silently dropping coincident cells', () => {
		const points = Array.from({ length: 257 }, (_, id) => endpoint(id, 0, 0));
		expect(voronoiCandidates(points, bounds)).toBeNull();
	});
});
