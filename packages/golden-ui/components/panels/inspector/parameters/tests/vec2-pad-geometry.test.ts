import { describe, expect, it } from 'vitest';
import {
	buildGridLines,
	clampPointToBounds,
	computeGridStep,
	parseVec2Bounds,
	toPlotX,
	toPlotY
} from '../vec2-pad-geometry';

describe('vec2 pad presentation geometry', () => {
	it('validates ranges and expands collapsed axes around their center', () => {
		expect(parseVec2Bounds({ kind: 'uniform', min: 2, max: 2 })).toEqual({
			xMin: 1.5,
			xMax: 2.5,
			yMin: 1.5,
			yMax: 2.5
		});
		expect(parseVec2Bounds({ kind: 'components', min: [-4, 6], max: [8, 6] })).toEqual({
			xMin: -4,
			xMax: 8,
			yMin: 5.5,
			yMax: 6.5
		});
		expect(parseVec2Bounds({ kind: 'uniform', min: 3, max: 2 })).toBeNull();
		expect(parseVec2Bounds({ kind: 'components', min: [0], max: [1, 2] })).toBeNull();
		expect(parseVec2Bounds({ kind: 'uniform', min: Number.NaN, max: 2 })).toBeNull();
	});

	it('clamps values while leaving unbounded input unchanged', () => {
		const bounds = parseVec2Bounds({ kind: 'components', min: [-2, 3], max: [4, 9] });
		expect(clampPointToBounds([10, 1], bounds)).toEqual([4, 3]);
		expect(clampPointToBounds([10, 1], null)).toEqual([10, 1]);
	});

	it('maps both axes into plot coordinates with vertical inversion', () => {
		const bounds = { xMin: -2, xMax: 2, yMin: 1, yMax: 5 };
		expect(toPlotX(0, bounds, 200)).toBe(100);
		expect(toPlotY(1, bounds, 100)).toBe(100);
		expect(toPlotY(5, bounds, 100)).toBe(0);
	});

	it('selects readable grid steps and caps visible lines', () => {
		expect(computeGridStep(1, 80)).toBe(10);
		expect(computeGridStep(1, 20)).toBe(5);
		const lines = buildGridLines(-1000, 1000, 1, (value) => value + 1000);
		expect(lines).toHaveLength(48);
		expect(lines[0]).toMatchObject({ value: -1000, position: 0 });
		expect(lines[1]).toMatchObject({ value: -999, position: 1 });
	});
});
