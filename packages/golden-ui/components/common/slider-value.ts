export interface SliderRange {
	min?: number;
	max?: number;
	step?: number;
	stepBase?: number;
}

/** Pointer and keyboard input share the declared grid, including one-sided ranges. */
export function constrainSliderValue(value: number, range: SliderRange): number {
	if (!Number.isFinite(value)) return NaN;
	const { min, max, step, stepBase } = range;
	let next = Math.max(min ?? -Infinity, Math.min(max ?? Infinity, value));
	if (step !== undefined && step > 0) {
		const base = stepBase ?? min ?? 0;
		const first = min === undefined ? -Infinity : Math.ceil((min - base) / step - 1e-9);
		const last = max === undefined ? Infinity : Math.floor((max - base) / step + 1e-9);
		if (first > last) return NaN;
		next = base + Math.max(first, Math.min(last, Math.round((next - base) / step))) * step;
	}
	return Math.max(min ?? -Infinity, Math.min(max ?? Infinity, next));
}

export function sliderDragDelta(
	delta: number,
	options: SliderRange & {
		width: number;
		relative: boolean;
		dragStep?: number;
		sensitivity: number;
	}
): number {
	const units = options.relative
		? options.step || options.dragStep || 0.1
		: ((options.max ?? 0) - (options.min ?? 0)) / Math.max(1, options.width);
	return delta * units * options.sensitivity;
}
