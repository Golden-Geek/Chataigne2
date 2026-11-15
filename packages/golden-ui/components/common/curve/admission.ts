import { canEditParameterValue } from '../../../store/parameter-value-access';
import type { CurveNodes } from './model';
import type { ParamNodeRef, ParsedCurveKey } from './types';
import { DECL_OUT_POSITION, DECL_OUT_VALUE, DECL_IN_POSITION, DECL_IN_VALUE } from './constants';

/** Geometry gestures require their complete parameter group; the Inspector can edit axes separately. */
export function curveKeyParameters(key: ParsedCurveKey | undefined): ParamNodeRef[] | null {
	if (!key?.position_param || !key.value_param) return null;
	return [key.position_param, key.value_param];
}

export function curveBezierParameters(
	params: Record<string, ParamNodeRef> | undefined
): ParamNodeRef[] | null {
	if (!params) return null;
	const refs = [DECL_OUT_POSITION, DECL_OUT_VALUE, DECL_IN_POSITION, DECL_IN_VALUE].map(
		(id) => params[id]
	);
	return refs.every(Boolean) ? refs : null;
}

export function curveGeometryParameters(
	keys: readonly (ParsedCurveKey | undefined)[],
	easing: readonly (Record<string, ParamNodeRef> | null | undefined)[] = []
): ParamNodeRef[] | null {
	const groups = [
		...keys.map(curveKeyParameters),
		...easing.map((params) => curveBezierParameters(params ?? undefined))
	];
	if (groups.some((group) => group === null)) return null;
	return [
		...new Map(groups.flatMap((group) => group ?? []).map((ref) => [ref.node_id, ref])).values()
	];
}

/** Recheck current typed access and UUIDs, never a cached permission or an old ID alone. */
export function curveParametersWritable(
	nodes: CurveNodes | null,
	refs: readonly ParamNodeRef[]
): boolean {
	return (
		refs.length > 0 &&
		refs.every((ref) => {
			const current = nodes?.get(ref.node_id);
			return current?.uuid === ref.uuid && canEditParameterValue(current);
		})
	);
}
