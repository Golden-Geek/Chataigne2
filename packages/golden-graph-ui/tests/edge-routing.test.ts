import { describe, expect, it } from 'vitest';
import {
	routeEdgeAroundObstacles,
	routingObstacleBuckets,
	type RoutingObstacle
} from '../edge-routing';

const route = (obstacles: RoutingObstacle[]) =>
	routeEdgeAroundObstacles(
		{ x: 0, y: 0 },
		{ x: 12, y: 0 },
		{ x: 2, y: 0 },
		{ x: 10, y: 0 },
		obstacles,
		routingObstacleBuckets(obstacles, 1),
		1
	);

describe('generic graph edge routing', () => {
	it('keeps a clear edge straight', () => {
		expect(route([])).toBe('M 0 0 L 12 0');
	});

	it('takes a deterministic detour around node bounds', () => {
		const obstacles = [{ left: 5, top: -1, right: 7, bottom: 1 }];
		const path = route(obstacles);
		expect(path).toBeTruthy();
		expect(path).not.toBe(route([]));
		expect(path).toContain('L 9 -2');
		expect(route(obstacles)).toBe(path);
	});

	it('indexes obstacles across positive and negative buckets', () => {
		const obstacle = { left: -17, top: -1, right: 17, bottom: 1 };
		const buckets = routingObstacleBuckets([obstacle], 1);
		expect(buckets.get('-2:-1')).toContain(obstacle);
		expect(buckets.get('1:0')).toContain(obstacle);
		expect(buckets.has('2:0')).toBe(false);
	});

	it('returns no route when the search boundary is obstructed', () => {
		expect(route([{ left: 3, top: -20, right: 9, bottom: 20 }])).toBeNull();
	});
});
