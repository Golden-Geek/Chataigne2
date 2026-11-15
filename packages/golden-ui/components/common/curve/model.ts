import type { NodeId, UiNodeDto } from '../../../types';
import type {
	ParamNodeRef,
	ParsedEasing,
	ParsedCurveKey,
	DragPreview,
	BezierEasingPreview
} from './types';
import {
	CURVE_NODE_TYPE,
	KEY_NODE_TYPE,
	EASING_NODE_TYPE,
	DECL_POSITION,
	DECL_VALUE,
	DECL_EASING,
	DECL_KIND,
	DECL_OUT_POSITION,
	DECL_OUT_VALUE,
	DECL_IN_POSITION,
	DECL_IN_VALUE
} from './constants';
import {
	param_number_value,
	param_string_value,
	normalize_easing_kind,
	to_param_ref
} from './values';

export type CurveNodes = Pick<ReadonlyMap<NodeId, UiNodeDto>, 'get'>;

const override_numeric_param_ref = (
	ref: ParamNodeRef | undefined,
	value: number | undefined
): ParamNodeRef | undefined => {
	if (!ref || value === undefined || !Number.isFinite(value)) {
		return ref;
	}
	if (ref.value.kind === 'int') {
		return {
			...ref,
			value: { kind: 'int', value: Math.round(value) }
		};
	}
	if (ref.value.kind === 'float') {
		return {
			...ref,
			value: { kind: 'float', value }
		};
	}
	return ref;
};

export const readCurveKeys = (
	nodes_by_id: CurveNodes | null,
	liveNode: UiNodeDto,
	drag_preview_by_key_id: ReadonlyMap<NodeId, DragPreview>,
	easing_drag_preview_by_key_id: ReadonlyMap<NodeId, BezierEasingPreview>
): ParsedCurveKey[] => {
	if (!nodes_by_id || liveNode.node_type !== CURVE_NODE_TYPE) {
		return [];
	}

	const parsed: ParsedCurveKey[] = [];
	for (let source_index = 0; source_index < liveNode.children.length; source_index += 1) {
		const child_id = liveNode.children[source_index];
		const key_node = nodes_by_id.get(child_id);
		if (!key_node || key_node.node_type !== KEY_NODE_TYPE) {
			continue;
		}

		let position_param: ParamNodeRef | undefined;
		let value_param: ParamNodeRef | undefined;
		let easing: ParsedEasing | undefined;

		for (const key_child_id of key_node.children) {
			const key_child = nodes_by_id.get(key_child_id);
			if (!key_child) {
				continue;
			}
			const decl_id = key_child.decl_id;
			if (decl_id === DECL_POSITION) {
				position_param = to_param_ref(key_child);
				continue;
			}
			if (decl_id === DECL_VALUE) {
				value_param = to_param_ref(key_child);
				continue;
			}
			if (decl_id === DECL_EASING && key_child.node_type === EASING_NODE_TYPE) {
				const easing_params: Record<string, ParamNodeRef> = {};
				for (const easing_child_id of key_child.children) {
					const easing_child = nodes_by_id.get(easing_child_id);
					const param_ref = to_param_ref(easing_child);
					if (!easing_child || !param_ref) {
						continue;
					}
					easing_params[easing_child.decl_id] = param_ref;
				}
				easing = {
					node_id: key_child.node_id,
					kind: normalize_easing_kind(
						param_string_value(easing_params[DECL_KIND]?.value, 'linear')
					),
					params: easing_params
				};
			}
		}

		const easing_preview = easing_drag_preview_by_key_id.get(key_node.node_id);
		if (easing && easing.kind === 'bezier' && easing_preview) {
			const next_params: Record<string, ParamNodeRef> = { ...easing.params };
			const next_out_position = override_numeric_param_ref(
				easing.params[DECL_OUT_POSITION],
				easing_preview.out_position
			);
			if (next_out_position) {
				next_params[DECL_OUT_POSITION] = next_out_position;
			}
			const next_out_value = override_numeric_param_ref(
				easing.params[DECL_OUT_VALUE],
				easing_preview.out_value
			);
			if (next_out_value) {
				next_params[DECL_OUT_VALUE] = next_out_value;
			}
			const next_in_position = override_numeric_param_ref(
				easing.params[DECL_IN_POSITION],
				easing_preview.in_position
			);
			if (next_in_position) {
				next_params[DECL_IN_POSITION] = next_in_position;
			}
			const next_in_value = override_numeric_param_ref(
				easing.params[DECL_IN_VALUE],
				easing_preview.in_value
			);
			if (next_in_value) {
				next_params[DECL_IN_VALUE] = next_in_value;
			}
			easing = {
				...easing,
				params: next_params
			};
		}

		let position = param_number_value(position_param?.value, 0);
		let value = param_number_value(value_param?.value, 0);
		const drag_preview = drag_preview_by_key_id.get(key_node.node_id);
		if (drag_preview) {
			position = drag_preview.position;
			value = drag_preview.value;
		}

		parsed.push({
			node_id: key_node.node_id,
			position,
			value,
			source_index,
			position_param,
			value_param,
			easing
		});
	}

	parsed.sort(
		(left, right) => left.position - right.position || left.source_index - right.source_index
	);
	return parsed;
};
