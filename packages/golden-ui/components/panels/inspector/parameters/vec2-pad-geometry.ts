import type { UiRangeConstraint } from '../../../../types';
import { formatWatcherNumber } from '../../../common/watcher/watcher-utils';

const EPSILON = 1e-9;

export interface Bounds {
	xMin: number;
	xMax: number;
	yMin: number;
	yMax: number;
}

export interface GridLine {
	value: number;
	position: number;
	label: string;
}

function normalizeBounds(bounds: Bounds): Bounds {
	let { xMin, xMax, yMin, yMax } = bounds;
	if (Math.abs(xMax - xMin) <= EPSILON) {
		const center = (xMin + xMax) * 0.5;
		xMin = center - 0.5;
		xMax = center + 0.5;
	}
	if (Math.abs(yMax - yMin) <= EPSILON) {
		const center = (yMin + yMax) * 0.5;
		yMin = center - 0.5;
		yMax = center + 0.5;
	}
	return { xMin, xMax, yMin, yMax };
}

export function parseVec2Bounds(range: UiRangeConstraint | undefined): Bounds | null {
	if (!range) return null;
	if (range.kind === 'uniform') {
		if (
			range.min === undefined ||
			range.max === undefined ||
			!Number.isFinite(range.min) ||
			!Number.isFinite(range.max) ||
			range.min > range.max
		) {
			return null;
		}
		return normalizeBounds({
			xMin: range.min,
			xMax: range.max,
			yMin: range.min,
			yMax: range.max
		});
	}

	const xMin = range.min?.[0];
	const yMin = range.min?.[1];
	const xMax = range.max?.[0];
	const yMax = range.max?.[1];
	if (
		xMin === undefined ||
		yMin === undefined ||
		xMax === undefined ||
		yMax === undefined ||
		!Number.isFinite(xMin) ||
		!Number.isFinite(yMin) ||
		!Number.isFinite(xMax) ||
		!Number.isFinite(yMax) ||
		xMin > xMax ||
		yMin > yMax
	) {
		return null;
	}
	return normalizeBounds({ xMin, xMax, yMin, yMax });
}

export function clampPointToBounds(
	point: [number, number],
	bounds: Bounds | null
): [number, number] {
	if (!bounds) return point;
	return [
		Math.min(bounds.xMax, Math.max(bounds.xMin, point[0])),
		Math.min(bounds.yMax, Math.max(bounds.yMin, point[1]))
	];
}

export function toPlotX(value: number, bounds: Bounds, plotWidth: number): number {
	return ((value - bounds.xMin) / Math.max(EPSILON, bounds.xMax - bounds.xMin)) * plotWidth;
}

export function toPlotY(value: number, bounds: Bounds, plotHeight: number): number {
	return (
		plotHeight - ((value - bounds.yMin) / Math.max(EPSILON, bounds.yMax - bounds.yMin)) * plotHeight
	);
}

export function computeGridStep(baseStep: number, span: number, targetLineCount = 8): number {
	const safeBase = Math.max(0.0001, Math.abs(baseStep));
	const roughStep = Math.max(safeBase, span / Math.max(2, targetLineCount));
	const exponent = Math.floor(Math.log10(roughStep / safeBase));
	const scaledBase = safeBase * Math.pow(10, exponent);
	for (const factor of [1, 2, 5, 10]) {
		const candidate = scaledBase * factor;
		if (candidate >= roughStep - EPSILON) return candidate;
	}
	return scaledBase * 10;
}

export function buildGridLines(
	minValue: number,
	maxValue: number,
	step: number,
	mapValueToPosition: (value: number) => number
): GridLine[] {
	const safeStep = Math.max(0.0001, step);
	const first = Math.ceil(minValue / safeStep) * safeStep;
	const lines: GridLine[] = [];
	for (
		let value = first;
		value <= maxValue + safeStep * 0.001 && lines.length < 48;
		value += safeStep
	) {
		lines.push({ value, position: mapValueToPosition(value), label: formatWatcherNumber(value) });
	}
	return lines;
}
