import { describe, expect, it } from 'vitest';
import {
	cssValueToPx,
	pxToCssValue,
	type CssValueData,
	type CssUnitConversionContext
} from '../css-value';

const context: CssUnitConversionContext = {
	rootRemPx: 16,
	emPx: 12,
	axisBasePx: 200,
	viewportWidthPx: 1000,
	viewportHeightPx: 800
};

describe('CSS units at their measured presentation boundary', () => {
	it.each([
		['px', 10],
		['rem', 160],
		['em', 120],
		['percent', 20],
		['vw', 100],
		['vh', 80]
	] as const)('resolves and preserves the %s authoring unit', (unit, pixels) => {
		const value: CssValueData = { value: 10, unit };
		expect(cssValueToPx(value, 'x', context)).toBe(pixels);
		expect(pxToCssValue(pixels, value, 'x', context)).toEqual(value);
	});
});
