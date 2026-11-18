import { describe, expect, it } from 'vitest';
import { ViewportAlignment } from '../alignment';
import type { ViewPoint } from '../geometry';

const box = (p: ViewPoint) => ({ x: p.x, y: p.y, width: 40, height: 30 });
const target = { x: 100, y: 120, width: 80, height: 60 };
const tolerance = { x: 6, y: 6 };
describe('viewport alignment hints', () => {
	it('snaps moving edges and centers independently, including both axes', () => {
		const snap = new ViewportAlignment([target], box);
		const result = snap.snap({ x: 63, y: 92 }, tolerance);
		expect(result.point.x).toBeCloseTo(60);
		expect(result.point.y).toBeCloseTo(90);
		expect(result.guides).toEqual([
			{ axis: 'x', value: 100 },
			{ axis: 'y', value: 120 }
		]);
		snap.clear();
		const center = snap.snap({ x: 122, y: 60 }, tolerance);
		expect(center.point.x).toBeCloseTo(120);
		expect(center.point.y).toBe(60);
		expect(center.guides).toEqual([{ axis: 'x', value: 140 }]);
	});
	it('holds an acquired axis until the larger release radius, without pointer accumulation', () => {
		const snap = new ViewportAlignment([target], box);
		expect(snap.snap({ x: 63, y: 20 }, tolerance).point.x).toBeCloseTo(60);
		expect(snap.snap({ x: 68, y: 20 }, tolerance).point.x).toBeCloseTo(60);
		expect(snap.snap({ x: 70, y: 20 }, tolerance).point.x).toBe(70);
		snap.clear();
		expect(snap.snap({ x: 68, y: 20 }, tolerance).point.x).toBe(68);
	});
	it('aligns resize edges without attracting the fixed opposite edge', () => {
		const snap = new ViewportAlignment([target], (p) => ({
			x: 100,
			y: 30,
			width: p.x - 100,
			height: p.y - 30
		}));
		const result = snap.snap({ x: 177, y: 92 }, tolerance);
		expect(result.point.x).toBeCloseTo(180);
		expect(result.point.y).toBe(92);
		expect(result.guides).toEqual([{ axis: 'x', value: 140 }]);
	});
	it('resize mode cannot acquire the moving body center while traversing a nearby guide', () => {
		const snap = new ViewportAlignment(
			[
				{ x: 260, y: 170, width: 80, height: 60 },
				{ x: 0, y: 0, width: 400, height: 300 }
			],
			(p) => ({ x: 180, y: 110, width: p.x - 180, height: p.y - 110 }),
			{ centers: false }
		);
		let result;
		for (let step = 1; step <= 8; step++)
			result = snap.snap({ x: 260 + (38 * step) / 8, y: 170 + (28 * step) / 8 }, { x: 4, y: 4 });
		expect(result!.point.x).toBeCloseTo(300);
		expect(result!.point.y).toBeCloseTo(200);
		expect(result!.guides).toEqual([
			{ axis: 'x', value: 300 },
			{ axis: 'y', value: 200 }
		]);
	});
	it('solves coupled rotated resize coordinates and ignores singular fixed-axis matches', () => {
		const project = (p: ViewPoint) => ({
			x: 10,
			y: 20,
			width: 0.8 * p.x + 0.6 * p.y,
			height: -0.6 * p.x + 0.8 * p.y
		});
		const destination = { x: 90, y: 100, width: 30, height: 30 };
		const snap = new ViewportAlignment([destination], project);
		const result = snap.snap({ x: 17, y: 113 }, { x: 3, y: 3 });
		const after = project(result.point);
		expect(result.guides.length).toBeGreaterThan(0);
		for (const guide of result.guides) {
			const first = guide.axis === 'x' ? after.x : after.y;
			const size = guide.axis === 'x' ? after.width : after.height;
			expect(
				[first, first + size / 2, first + size].some(
					(value) => Math.abs(value - guide.value) < 1e-6
				)
			).toBe(true);
		}
	});
	it('retains normalized crop precision and uses viewport-scaled thresholds', () => {
		const project = (p: ViewPoint) => ({ x: p.x, y: 0.1, width: 0.8 - p.x, height: 0.4 });
		const snap = new ViewportAlignment([{ x: 0.2, y: 0.2, width: 0.1, height: 0.1 }], project);
		expect(snap.snap({ x: 0.203, y: 0.1 }, { x: 0.005, y: 0.005 }).point.x).toBeCloseTo(0.2);
		snap.clear();
		expect(snap.snap({ x: 0.203, y: 0.1 }, { x: 0.001, y: 0.001 }).guides).toEqual([]);
	});
	it('ignores unavailable/degenerate candidates and never fabricates geometry for an invalid projection', () => {
		const candidates = [
			{ ...target, width: 0 },
			{ ...target, x: NaN }
		];
		expect(new ViewportAlignment(candidates, box).snap({ x: 63, y: 92 }, tolerance).guides).toEqual(
			[]
		);
		expect(
			new ViewportAlignment([target], () => null).snap({ x: 63, y: 92 }, tolerance).guides
		).toEqual([]);
	});
	it('indexes a large candidate list once; each sample projects a constant number of bounds', () => {
		let calls = 0;
		const snap = new ViewportAlignment(
			Array.from({ length: 10000 }, (_, i) => ({ x: 100 * i, y: 120 * i, width: 80, height: 60 })),
			(p) => {
				calls++;
				return box(p);
			}
		);
		snap.snap({ x: 63, y: 92 }, tolerance);
		expect(calls).toBe(4);
	});
});
