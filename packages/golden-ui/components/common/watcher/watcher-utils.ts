import type { ParamValue, UiParamConstraints } from '../../../types';

export type WatcherRangeMode = 'adaptive' | 'fixed';
export type WatcherDecimationMode = 'off' | 'auto' | 'minmax';
export type WatcherVectorViewMode = 'phase' | 'curves';

export interface WatcherUiSettings {
	timeWindowSec: number;
	rangeMode: WatcherRangeMode;
	decimationMode: WatcherDecimationMode;
	vectorViewMode: WatcherVectorViewMode;
}

export interface NumericSample {
	labels: string[];
	values: number[];
}

const CURVE_COLORS = ['#ff5500', '#aaff00', '#00aaff', '#ff55cc'];
export const WATCHER_WINDOW_OPTIONS_SEC = [2, 5, 10, 20, 40] as const;
const WATCHER_DECIMATION_OPTIONS: WatcherDecimationMode[] = ['off', 'auto', 'minmax'];

export const WATCHER_DEFAULT_SETTINGS: WatcherUiSettings = {
	timeWindowSec: 10,
	rangeMode: 'adaptive',
	decimationMode: 'auto',
	vectorViewMode: 'phase'
};

const clamp01 = (value: number): number => {
	if (!Number.isFinite(value)) {
		return 0;
	}
	return Math.max(0, Math.min(1, value));
};

const shorten = (value: string, maxLength = 24): string => {
	if (value.length <= maxLength) {
		return value;
	}
	return `${value.slice(0, Math.max(1, maxLength - 3))}...`;
};

export const getNowMs = (): number =>
	typeof performance !== 'undefined' ? performance.now() : Date.now();

export const sanitizeWatcherUiSettings = (
	value: Partial<WatcherUiSettings> | null | undefined
): WatcherUiSettings => {
	const safeTimeWindow = Number(value?.timeWindowSec);
	const safeRangeMode = value?.rangeMode;
	const safeDecimationMode = value?.decimationMode;
	const safeVectorViewMode = value?.vectorViewMode;
	const nextDecimationMode: WatcherDecimationMode = WATCHER_DECIMATION_OPTIONS.includes(
		safeDecimationMode as WatcherDecimationMode
	)
		? (safeDecimationMode as WatcherDecimationMode)
		: WATCHER_DEFAULT_SETTINGS.decimationMode;

	return {
		timeWindowSec: WATCHER_WINDOW_OPTIONS_SEC.includes(
			safeTimeWindow as (typeof WATCHER_WINDOW_OPTIONS_SEC)[number]
		)
			? safeTimeWindow
			: WATCHER_DEFAULT_SETTINGS.timeWindowSec,
		rangeMode: safeRangeMode === 'fixed' ? 'fixed' : WATCHER_DEFAULT_SETTINGS.rangeMode,
		decimationMode: nextDecimationMode,
		vectorViewMode:
			safeVectorViewMode === 'curves' ? 'curves' : WATCHER_DEFAULT_SETTINGS.vectorViewMode
	};
};

export const getFixedRange = (
	constraints: UiParamConstraints | undefined
): { min: number; max: number } | null => {
	const range = constraints?.range;
	if (!range) {
		return null;
	}

	if (range.kind === 'uniform') {
		const min = Number(range.min);
		const max = Number(range.max);
		if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
			return null;
		}
		return { min, max };
	}

	if (range.kind === 'components') {
		const mins =
			range.min?.map((entry) => Number(entry)).filter((entry) => Number.isFinite(entry)) ?? [];
		const maxs =
			range.max?.map((entry) => Number(entry)).filter((entry) => Number.isFinite(entry)) ?? [];
		if (mins.length === 0 || maxs.length === 0) {
			return null;
		}
		const min = Math.min(...mins);
		const max = Math.max(...maxs);
		if (max <= min) {
			return null;
		}
		return { min, max };
	}

	return null;
};

export const extractNumericSample = (value: ParamValue): NumericSample | null => {
	switch (value.kind) {
		case 'int':
		case 'float':
		case 'css_value':
			return {
				labels: ['V'],
				values: [value.value]
			};
		case 'vec2':
			return {
				labels: ['X', 'Y'],
				values: [value.value[0], value.value[1]]
			};
		case 'vec3':
			return {
				labels: ['X', 'Y', 'Z'],
				values: [value.value[0], value.value[1], value.value[2]]
			};
		default:
			return null;
	}
};

export const extractEventLabel = (value: ParamValue): string | null => {
	switch (value.kind) {
		case 'trigger':
			return 'trigger';
		case 'enum':
			return shorten(value.value);
		case 'bool':
			return value.value ? 'true' : 'false';
		case 'str': {
			const trimmed = value.value.trim();
			return trimmed.length > 0 ? shorten(trimmed) : '<empty>';
		}
		case 'file': {
			const trimmed = value.value.trim();
			return trimmed.length > 0 ? shorten(trimmed) : '<none>';
		}
		case 'css_value':
			return `${value.value}${value.unit === 'percent' ? '%' : value.unit}`;
		case 'reference':
			return shorten(value.cached_name || value.uuid || 'none');
		default:
			return null;
	}
};

export const extractColorSample = (value: ParamValue): [number, number, number, number] | null => {
	if (value.kind !== 'color') {
		return null;
	}
	return [
		clamp01(value.value[0]),
		clamp01(value.value[1]),
		clamp01(value.value[2]),
		clamp01(value.value[3])
	];
};

export const watcherSeriesColor = (seriesIndex: number): string =>
	CURVE_COLORS[seriesIndex % CURVE_COLORS.length];

export const watcherEventColor = (label: string): string => {
	let hash = 2166136261;
	for (let index = 0; index < label.length; index += 1) {
		hash ^= label.charCodeAt(index);
		hash = Math.imul(hash, 16777619);
	}
	const hue = Math.abs(hash) % 360;
	return `hsl(${hue} 74% 62%)`;
};

export const toCssColor = (color: ReadonlyArray<number>): string => {
	const red = Math.round(clamp01(Number(color[0] ?? 0)) * 255);
	const green = Math.round(clamp01(Number(color[1] ?? 0)) * 255);
	const blue = Math.round(clamp01(Number(color[2] ?? 0)) * 255);
	const alpha = clamp01(Number(color[3] ?? 1));
	return `rgba(${red} ${green} ${blue} / ${alpha})`;
};

export const formatWatcherNumber = (value: number): string => {
	if (!Number.isFinite(value)) {
		return '0';
	}
	const abs = Math.abs(value);
	if (abs >= 10000 || (abs > 0 && abs < 0.001)) {
		return value.toExponential(2);
	}
	if (abs >= 100) {
		return value.toFixed(1);
	}
	if (abs >= 10) {
		return value.toFixed(2);
	}
	return value.toFixed(3);
};
