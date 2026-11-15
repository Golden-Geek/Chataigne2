import type { ParamValue, UiNodeDto, UiParamConstraints } from '../../../types';
import type {
	CurveEasingKind,
	CurveStepMode,
	CurvePhaseMode,
	CurveShape,
	ParamNodeRef,
	CurveRangeConstraint
} from './types';
import { CURVE_EPSILON } from './constants';

export const clamp = (value: number, min: number, max: number): number =>
	Math.min(max, Math.max(min, value));
export const finite_or = (value: number, fallback: number): number =>
	Number.isFinite(value) ? value : fallback;
export const param_number_value = (value: ParamValue | undefined, fallback: number): number => {
	if (!value) {
		return fallback;
	}
	if (value.kind === 'float') {
		return finite_or(value.value, fallback);
	}
	if (value.kind === 'int') {
		return finite_or(value.value, fallback);
	}
	return fallback;
};
export const param_string_value = (value: ParamValue | undefined, fallback: string): string => {
	if (!value) {
		return fallback;
	}
	if (value.kind === 'enum') {
		return value.value.trim();
	}
	if (value.kind === 'str') {
		return value.value;
	}
	return fallback;
};
export const normalize_easing_kind = (value: string): CurveEasingKind => {
	const normalized = value.trim().toLowerCase();
	if (normalized === 'elastic') {
		return 'elastic';
	}
	if (normalized === 'bounce') {
		return 'bounce';
	}
	if (normalized === 'bezier') {
		return 'bezier';
	}
	if (normalized === 'hold') {
		return 'hold';
	}
	if (normalized === 'steps') {
		return 'steps';
	}
	if (normalized === 'shape') {
		return 'shape';
	}
	if (normalized === 'perlinnoise') {
		return 'perlinNoise';
	}
	if (normalized === 'random') {
		return 'random';
	}
	if (normalized === 'script') {
		return 'script';
	}
	return 'linear';
};
export const normalize_step_mode = (value: string): CurveStepMode =>
	value.trim().toLowerCase() === 'stepsize' ? 'stepSize' : 'numSteps';
export const normalize_shape = (value: string): CurveShape => {
	const normalized = value.trim().toLowerCase();
	if (normalized === 'triangle') {
		return 'triangle';
	}
	if (normalized === 'saw') {
		return 'saw';
	}
	if (normalized === 'reversesaw') {
		return 'reverseSaw';
	}
	if (normalized === 'square') {
		return 'square';
	}
	return 'sine';
};
export const normalize_phase_mode = (value: string): CurvePhaseMode =>
	value.trim().toLowerCase() === 'numphases' ? 'numPhases' : 'frequency';

export const normalize_bounds_pair = (
	min_candidate: number,
	max_candidate: number
): { min: number; max: number } | null => {
	if (!Number.isFinite(min_candidate) || !Number.isFinite(max_candidate)) {
		return null;
	}
	const min = Math.min(min_candidate, max_candidate);
	const max = Math.max(min_candidate, max_candidate);
	if (max - min <= CURVE_EPSILON) {
		return null;
	}
	return { min, max };
};
export const make_curve_range_constraint = (
	x_min_candidate: number,
	x_max_candidate: number,
	y_min_candidate: number,
	y_max_candidate: number
): CurveRangeConstraint | null => {
	const x_bounds = normalize_bounds_pair(x_min_candidate, x_max_candidate);
	const y_bounds = normalize_bounds_pair(y_min_candidate, y_max_candidate);
	if (!x_bounds || !y_bounds) {
		return null;
	}
	return {
		x_min: x_bounds.min,
		x_max: x_bounds.max,
		y_min: y_bounds.min,
		y_max: y_bounds.max
	};
};
export const range_bounds_from_value = (
	value: ParamValue | undefined
): { min: number; max: number } | null => {
	if (!value || value.kind !== 'vec2') {
		return null;
	}
	return normalize_bounds_pair(value.value[0], value.value[1]);
};
export const uniform_bounds_from_constraints = (
	constraints: UiParamConstraints | undefined
): { min: number; max: number } | null => {
	const range = constraints?.range;
	if (!range || range.kind !== 'uniform') {
		return null;
	}
	if (range.min === undefined || range.max === undefined) {
		return null;
	}
	return normalize_bounds_pair(range.min, range.max);
};

export const to_param_ref = (candidate: UiNodeDto | undefined): ParamNodeRef | undefined => {
	if (!candidate || candidate.data.kind !== 'parameter') {
		return undefined;
	}
	return {
		node_id: candidate.node_id,
		uuid: candidate.uuid,
		value: candidate.data.param.value,
		constraints: candidate.data.param.constraints
	};
};

export const coerce_param_value_for_ref = (
	ref: ParamNodeRef,
	value: ParamValue
): ParamValue | null => {
	if (ref.value.kind === 'int') {
		if (value.kind === 'int') {
			return { kind: 'int', value: Math.round(value.value) };
		}
		if (value.kind === 'float') {
			return { kind: 'int', value: Math.round(value.value) };
		}
		return null;
	}
	if (ref.value.kind === 'float') {
		if (value.kind === 'int' || value.kind === 'float') {
			return { kind: 'float', value: value.value };
		}
		return null;
	}
	if (ref.value.kind === 'enum') {
		if (value.kind === 'enum' || value.kind === 'str') {
			return { kind: 'enum', value: value.value };
		}
		return null;
	}
	if (ref.value.kind === 'str') {
		if (value.kind === 'enum' || value.kind === 'str') {
			return { kind: 'str', value: value.value };
		}
		return null;
	}
	if (ref.value.kind === value.kind) {
		return value;
	}
	return null;
};
