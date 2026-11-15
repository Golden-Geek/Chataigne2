import type { CurveEasingKind } from './types';

export const CURVE_NODE_TYPE = 'animation_curve';
export const KEY_NODE_TYPE = 'animation_curve_key';
export const KEY_ITEM_KIND = 'animation_curve_key';
export const EASING_NODE_TYPE = 'animation_curve_easing';
export const RANGE_NODE_TYPE = 'animation_curve_range';

export const DECL_POSITION = 'position';
export const DECL_VALUE = 'value';
export const DECL_EASING = 'easing';
export const DECL_RANGE = 'range';
export const DECL_RANGE_X = 'x';
export const DECL_RANGE_Y = 'y';
export const DECL_KIND = 'kind';
export const DECL_OUT_POSITION = 'out_position';
export const DECL_OUT_VALUE = 'out_value';
export const DECL_IN_POSITION = 'in_position';
export const DECL_IN_VALUE = 'in_value';
export const DECL_STEP_MODE = 'step_mode';
export const DECL_STEP_SIZE = 'step_size';
export const DECL_NUM_STEPS = 'num_steps';
export const DECL_SHAPE = 'shape';
export const DECL_AMPLITUDE = 'amplitude';
export const DECL_PHASE_MODE = 'phase_mode';
export const DECL_FREQUENCY = 'frequency';
export const DECL_NUM_PHASES = 'num_phases';
export const DECL_FADE_IN = 'fade_in';
export const DECL_FADE_OUT = 'fade_out';
export const DECL_OCTAVES = 'octaves';
export const DECL_PHASE = 'phase';
export const DECL_SEED = 'seed';
export const DECL_SCRIPT_SOURCE = 'script_source';

export const CURVE_EPSILON = 1e-12;
export const MAX_PERLIN_OCTAVES = 12;
export const MIN_VIEW_SPAN = 1e-6;
export const MAX_VIEW_SPAN = 1e9;
export const CURVE_SCULPT_SHIFT_FULL_STRENGTH_PX = 8;
export const CURVE_DRAW_FIT_MAX_ERROR_PX = 4.5;
export const CURVE_DRAW_FIT_MAX_KEYS = 12;
export const VIEW_BOUNDS_ANIMATION_DURATION_MS = 220;
export const KEY_HIT_RADIUS_REM = 0.7;
export const CURVE_HIT_RADIUS_REM = 0.7;
export const BEZIER_HANDLE_HIT_RADIUS_REM = 0.64;
export const CANVAS_DOUBLE_CLICK_WINDOW_MS = 320;
export const CANVAS_DOUBLE_CLICK_RADIUS_PX = 7;
export const DEFAULT_EASING_KIND_ORDER: CurveEasingKind[] = [
	'linear',
	'bezier',
	'hold',
	'steps',
	'shape',
	'perlinNoise',
	'elastic',
	'bounce',
	'random',
	'script'
];
export const EASING_KIND_LABELS: Record<CurveEasingKind, string> = {
	linear: 'Linear',
	bezier: 'Bezier',
	hold: 'Hold',
	steps: 'Steps',
	shape: 'Shape',
	perlinNoise: 'Perlin Noise',
	elastic: 'Elastic',
	bounce: 'Bounce',
	random: 'Random',
	script: 'Script'
};
