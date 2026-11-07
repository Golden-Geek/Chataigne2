import { describe, expect, it } from 'vitest';
import { constrainSliderValue, sliderDragDelta } from '../slider-value';

describe('slider units and declared grids', () => {
	it('keeps relative step drags finite without both bounds', () => {
		for (const range of [{}, { min: 0 }, { max: 10 }, { min: 0, max: 4294967295 }]) {
			expect(
				sliderDragDelta(12, { ...range, step: 0.25, width: 100, relative: true, sensitivity: 1 })
			).toBe(3);
		}
		expect(
			sliderDragDelta(20, { width: 100, relative: true, dragStep: 0.001, sensitivity: 1 })
		).toBe(0.02);
	});
	it('uses range spans only for bounded sliders, preserving grid origins and bounds', () => {
		expect(
			sliderDragDelta(20, { min: 0, max: 10, width: 100, relative: false, sensitivity: 1 })
		).toBe(2);
		expect(constrainSliderValue(1.37, { min: 1, max: 2, step: 0.25 })).toBe(1.25);
		expect(constrainSliderValue(-100, { min: 0, step: 1 })).toBe(0);
		expect(constrainSliderValue(100, { max: 10 })).toBe(10);
		expect(constrainSliderValue(0.4, { step: 0.5, stepBase: 0.25 })).toBe(0.25);
		expect(constrainSliderValue(1, { min: 0, max: 1, step: 1, stepBase: 0.5 })).toBe(0.5);
	});
});
