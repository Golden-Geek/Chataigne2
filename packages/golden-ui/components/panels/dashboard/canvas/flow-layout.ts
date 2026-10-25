import { CSS_UNIT_OPTIONS, formatCssValue, type CssValueData } from '../../../../css-value';
import type { CssUnit } from '../../../../types';
import { sumCssLengthExpressions } from './geometry';
import type { NonFreeSurfaceFlowRenderItem } from './contracts';

/** Prefix sums retain complete CSS flow extents while a bounded set of children mounts. */
export class FlowLengths {
	readonly #positions = new Map<string, number>();
	readonly #units = new Map<CssUnit, Float64Array>();
	readonly #auto: Uint32Array;
	readonly length: number;
	constructor(entries: readonly { key: string; size: CssValueData | null }[]) {
		this.length = entries.length;
		this.#auto = new Uint32Array(entries.length + 1);
		for (const { unit } of CSS_UNIT_OPTIONS)
			this.#units.set(unit, new Float64Array(entries.length + 1));
		entries.forEach(({ key, size }, index) => {
			this.#positions.set(key, index);
			this.#auto[index + 1] = this.#auto[index] + (size === null ? 1 : 0);
			for (const [unit, values] of this.#units) values[index + 1] = values[index];
			// Match each slot's CSS serialization before aggregation, rather than rounding only the sum.
			if (size) this.#units.get(size.unit)![index + 1] += Number.parseFloat(formatCssValue(size));
		});
	}
	index(key: string): number | undefined {
		return this.#positions.get(key);
	}
	expression(start: number, end: number, autoBasis: string, gap: CssValueData): string {
		const parts: string[] = [];
		for (const [unit, values] of this.#units) {
			const value = values[end] - values[start];
			if (value !== 0) parts.push(formatCssValue({ value, unit }));
		}
		const auto = this.#auto[end] - this.#auto[start];
		if (auto) parts.push(`(${autoBasis} * ${auto})`);
		if (end - start > 1) parts.push(`(${formatCssValue(gap)} * ${end - start - 1})`);
		return `calc(${sumCssLengthExpressions(parts)})`;
	}
}

export type FlowSegment =
	| { kind: 'item'; key: string; item: NonFreeSurfaceFlowRenderItem }
	| { kind: 'spacer'; key: string; style: string };

export function flowSegments(
	items: readonly NonFreeSurfaceFlowRenderItem[],
	lengths: FlowLengths | null,
	style: (start: number, end: number) => string
): FlowSegment[] {
	const result: FlowSegment[] = [];
	let cursor = 0;
	for (const item of items) {
		const index = lengths?.index(item.key);
		if (index !== undefined && index > cursor)
			result.push({ kind: 'spacer', key: `space-before-${item.key}`, style: style(cursor, index) });
		result.push({ kind: 'item', key: item.key, item });
		if (index !== undefined) cursor = index + 1;
	}
	if (lengths && cursor < lengths.length)
		result.push({ kind: 'spacer', key: 'space-after-flow', style: style(cursor, lengths.length) });
	return result;
}
