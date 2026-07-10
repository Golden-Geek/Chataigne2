import type { CssUnit, ParamValue } from './types';

export interface CssValueData {
	value: number;
	unit: CssUnit;
}

export interface CssUnitConversionContext {
	rootRemPx: number;
	axisBasePx: number;
	viewportWidthPx: number;
	viewportHeightPx: number;
}

export const CSS_UNIT_OPTIONS: Array<{ unit: CssUnit; label: string }> = [
	{ unit: 'rem', label: 'rem' },
	{ unit: 'px', label: 'px' },
	{ unit: 'em', label: 'em' },
	{ unit: 'percent', label: '%' },
	{ unit: 'vw', label: 'vw' },
	{ unit: 'vh', label: 'vh' }
];

const CSS_UNIT_SUFFIX: Record<CssUnit, string> = {
	px: 'px',
	rem: 'rem',
	em: 'em',
	percent: '%',
	vw: 'vw',
	vh: 'vh'
};

const CSS_UNITS = new Set<CssUnit>(['px', 'rem', 'em', 'percent', 'vw', 'vh']);

export const isCssUnit = (value: unknown): value is CssUnit =>
	typeof value === 'string' && CSS_UNITS.has(value as CssUnit);

const normalizeCssNumber = (value: number): number => (Number.isFinite(value) ? value : 0);

const formatCssNumber = (value: number): string => {
	const normalized = normalizeCssNumber(value);
	if (Math.abs(normalized) < 1e-9) {
		return '0';
	}
	return normalized
		.toFixed(4)
		.replace(/\.0+$/, '')
		.replace(/(\.\d*?)0+$/, '$1');
};

export const formatCssValue = (value: CssValueData): string =>
	`${formatCssNumber(value.value)}${CSS_UNIT_SUFFIX[value.unit]}`;

export const parseCssValue = (
	input: string,
	fallbackUnit: CssUnit | null = null
): CssValueData | null => {
	const trimmed = input.trim();
	if (!trimmed) {
		return null;
	}

	const normalized = trimmed.toLowerCase();
	for (const unit of CSS_UNIT_OPTIONS) {
		const suffix = CSS_UNIT_SUFFIX[unit.unit];
		if (!normalized.endsWith(suffix)) {
			continue;
		}
		const numberText = trimmed.slice(0, trimmed.length - suffix.length).trim();
		const parsed = Number(numberText);
		if (!Number.isFinite(parsed)) {
			return null;
		}
		return { value: parsed, unit: unit.unit };
	}

	if (!fallbackUnit) {
		return null;
	}

	const parsed = Number(trimmed);
	if (!Number.isFinite(parsed)) {
		return null;
	}
	return { value: parsed, unit: fallbackUnit };
};

export const cssPixelsPerUnit = (
	unit: CssUnit,
	axis: 'x' | 'y',
	context: CssUnitConversionContext
): number => {
	switch (unit) {
		case 'px':
			return 1;
		case 'rem':
		case 'em':
			return Math.max(context.rootRemPx, 1e-6);
		case 'percent':
			return Math.max(context.axisBasePx / 100, 1e-6);
		case 'vw':
			return Math.max(context.viewportWidthPx / 100, 1e-6);
		case 'vh':
			return Math.max(context.viewportHeightPx / 100, 1e-6);
	}
};

export const cssValueToPx = (
	value: CssValueData,
	axis: 'x' | 'y',
	context: CssUnitConversionContext
): number => normalizeCssNumber(value.value) * cssPixelsPerUnit(value.unit, axis, context);

export const pxToCssValue = (
	pixels: number,
	template: CssValueData,
	axis: 'x' | 'y',
	context: CssUnitConversionContext
): CssValueData => ({
	value: pixels / cssPixelsPerUnit(template.unit, axis, context),
	unit: template.unit
});

export const pxToCssUnitDelta = (
	deltaPx: number,
	unit: CssUnit,
	axis: 'x' | 'y',
	context: CssUnitConversionContext
): number => deltaPx / cssPixelsPerUnit(unit, axis, context);

export const cssValueFromParamValue = (
	value: ParamValue | undefined,
	fallback: CssValueData = { value: 0, unit: 'rem' }
): CssValueData => {
	if (value?.kind === 'css_value') {
		return { value: value.value, unit: value.unit };
	}
	if (value?.kind === 'float' || value?.kind === 'int') {
		return { value: value.value, unit: fallback.unit };
	}
	return fallback;
};
