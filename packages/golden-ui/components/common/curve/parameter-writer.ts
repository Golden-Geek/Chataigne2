import { sendSetParamIntent } from '../../../store/ui-intents';
import type { NodeId, ParamValue } from '../../../types';
import type { CurveNodes } from './model';
import type { CurveOperation } from './operations';
import type { ParamNodeRef, ParsedCurveKey, DragPreview, BezierEasingPreview } from './types';
import { curveParametersWritable } from './admission';
import { to_param_ref, coerce_param_value_for_ref, param_number_value } from './values';
import { DECL_OUT_POSITION, DECL_OUT_VALUE, DECL_IN_POSITION, DECL_IN_VALUE } from './constants';

/** Typed writes share current access checks and retain only their unsettled operations. */
export function createCurveParameterWriter(context: {
	readonly nodes: CurveNodes | null;
	key(id: NodeId): ParsedCurveKey | undefined;
	bezier(id: NodeId): Record<string, ParamNodeRef> | null;
}) {
	const pending = new Set<Promise<void>>();
	const write = async (
		ref: ParamNodeRef | undefined,
		value: ParamValue,
		scope?: CurveOperation
	): Promise<void> => {
		const operation = (async () => {
			if (
				(scope && !scope.documentCurrent()) ||
				!ref ||
				!curveParametersWritable(context.nodes, [ref])
			)
				return;
			const current = to_param_ref(context.nodes?.get(ref.node_id));
			if (!current) return;
			const coerced = coerce_param_value_for_ref(current, value);
			if (
				!coerced ||
				((coerced.kind === 'float' || coerced.kind === 'int') && !Number.isFinite(coerced.value))
			)
				return;
			await sendSetParamIntent(ref.node_id, coerced, 'Coalesce');
		})();
		pending.add(operation);
		void operation.then(
			() => pending.delete(operation),
			() => pending.delete(operation)
		);
		await operation;
	};
	const numeric = async (
		ref: ParamNodeRef | undefined,
		value: number,
		scope?: CurveOperation
	): Promise<void> => {
		if (Number.isFinite(value)) await write(ref, { kind: 'float', value }, scope);
	};
	return {
		write,
		numeric,
		pending: () => [...pending],
		keys(targets: Map<NodeId, DragPreview>) {
			for (const [id, target] of targets) {
				const key = context.key(id);
				if (!key) continue;
				void numeric(key.position_param, target.position);
				void numeric(key.value_param, target.value);
			}
		},
		bezier(targets: Map<NodeId, BezierEasingPreview>) {
			const changed = (ref: ParamNodeRef | undefined, value: number) => {
				if (
					ref &&
					Number.isFinite(value) &&
					Math.abs(param_number_value(ref.value, value) - value) > 1e-9
				)
					void numeric(ref, value);
			};
			for (const [id, preview] of targets) {
				const params = context.bezier(id);
				if (!params) continue;
				changed(params[DECL_OUT_POSITION], preview.out_position);
				changed(params[DECL_OUT_VALUE], preview.out_value);
				changed(params[DECL_IN_POSITION], preview.in_position);
				changed(params[DECL_IN_VALUE], preview.in_value);
			}
		}
	};
}
