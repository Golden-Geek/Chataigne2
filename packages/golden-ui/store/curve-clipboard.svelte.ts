import type { NodeId, ParamValue } from '../types';

export interface CurveClipboardKey {
	position_offset: number;
	value_offset: number;
	easing_values: Record<string, ParamValue>;
}

export const curveClipboardState = $state({
	sourceCurveNodeId: null as NodeId | null,
	keys: [] as CurveClipboardKey[]
});
