import { type CurveOperation } from './operations';
import { curveParametersWritable } from './admission';
import { DECL_KIND, DEFAULT_EASING_KIND_ORDER, EASING_KIND_LABELS } from './constants';
import { param_string_value, normalize_easing_kind } from './values';
import { sample_compiled_curve } from './evaluation';
import { createUiEditSession, sendRemoveNodesIntent } from '../../../store/ui-intents';
import { curveClipboardState, type CurveClipboardKey } from '../../../store/curve-clipboard.svelte';
import type { NodeId, ParamValue } from '../../../types';
import type { ContextMenuItem } from '../context-menu';
import type {
	CurveEasingKind,
	ParsedCurveKey,
	EasingKindOption,
	BezierHandleRef,
	PendingSplitEasing,
	AddKeyOptions,
	CurveSegmentHit,
	CurveContextMenuTarget
} from './types';
import type {
	CurveEditorServices,
	CurveEditorDocument,
	CurveEditorBindings
} from './editor-context';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorSelection } from './editor-selection.svelte';
import type { createCurveEditorInsertion } from './editor-insertion';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorCamera } from './editor-camera';
interface Context {
	readonly services: CurveEditorServices;
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly selection: ReturnType<typeof createCurveEditorSelection>;
	readonly doc: CurveEditorDocument;
	readonly insertion: ReturnType<typeof createCurveEditorInsertion>;
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly bindings: CurveEditorBindings;
	readonly camera: ReturnType<typeof createCurveEditorCamera>;
}
/** Menu choices observe the same selection and edit paths as keyboard commands. */
export function createCurveEditorCommands(context: Context) {
	const easing_kind_options_for_key = (key: ParsedCurveKey | undefined): EasingKindOption[] => {
		const kind_param = key?.easing?.params[DECL_KIND];
		const enum_options = kind_param?.constraints.enum_options ?? [];
		if (enum_options.length === 0) {
			return DEFAULT_EASING_KIND_ORDER.map((value) => ({
				value,
				label: EASING_KIND_LABELS[value]
			}));
		}
		const options: EasingKindOption[] = [];
		const seen = new Set<CurveEasingKind>();
		for (const option of enum_options) {
			const value = normalize_easing_kind(param_string_value(option.value, option.variant_id));
			if (seen.has(value)) {
				continue;
			}
			seen.add(value);
			options.push({
				value,
				label: option.label.trim().length > 0 ? option.label : EASING_KIND_LABELS[value]
			});
		}
		return options.length > 0
			? options
			: DEFAULT_EASING_KIND_ORDER.map((value) => ({
					value,
					label: EASING_KIND_LABELS[value]
				}));
	};
	const apply_easing_param_values = async (
		key: ParsedCurveKey | undefined,
		values: Record<string, ParamValue>,
		scope?: CurveOperation
	): Promise<void> => {
		if (!key?.easing || Object.keys(values).length === 0) {
			return;
		}
		const params = key.easing.params;
		const next_kind = values[DECL_KIND];
		if (next_kind) {
			await context.services.parameterWriter.write(params[DECL_KIND], next_kind, scope);
		}
		for (const [decl_id, value] of Object.entries(values)) {
			if (decl_id === DECL_KIND) {
				continue;
			}
			await context.services.parameterWriter.write(params[decl_id], value, scope);
		}
	};
	const easing_target_key_ids = (target: CurveContextMenuTarget | null): NodeId[] => {
		if (!target) {
			return [];
		}
		if (target.kind === 'all') {
			return context.model.compiled_curve.keys
				.slice(0, Math.max(0, context.model.compiled_curve.keys.length - 1))
				.map((key) => key.node_id);
		}
		return context.model.parsed_key_by_id.has(target.key_id) ? [target.key_id] : [];
	};
	const shared_easing_kind_for_key_ids = (key_ids: readonly NodeId[]): CurveEasingKind | null => {
		let shared_kind: CurveEasingKind | null = null;
		for (const key_id of key_ids) {
			const kind = context.model.parsed_key_by_id.get(key_id)?.easing?.kind ?? null;
			if (kind === null) {
				return null;
			}
			if (shared_kind === null) {
				shared_kind = kind;
				continue;
			}
			if (shared_kind !== kind) {
				return null;
			}
		}
		return shared_kind;
	};
	const apply_easing_kind_to_key_ids = async (
		key_ids: readonly NodeId[],
		easing_kind: CurveEasingKind,
		label: string
	): Promise<boolean> => {
		const ordered_key_ids = context.selection.ordered_node_ids(key_ids);
		const target_keys = ordered_key_ids
			.map((key_id) => context.model.parsed_key_by_id.get(key_id))
			.filter(
				(key): key is ParsedCurveKey =>
					key !== undefined &&
					key.easing !== undefined &&
					key.easing.params[DECL_KIND] !== undefined &&
					key.easing.kind !== easing_kind
			);
		if (
			target_keys.length === 0 ||
			target_keys.some(
				(key) =>
					!curveParametersWritable(context.doc.graphNodesById, [key.easing!.params[DECL_KIND]])
			)
		) {
			return false;
		}
		const scope = context.services.curveOperations.capture();
		if (!scope) return false;
		const edit_session = createUiEditSession(label, 'curve-easing-kind', scope.host);
		try {
			await edit_session.begin();
			if (!scope.current() || !edit_session.active) return false;
			for (const key of target_keys) {
				await apply_easing_param_values(
					key,
					{
						[DECL_KIND]: context.insertion.enum_param_value(easing_kind)
					},
					scope
				);
			}
			return scope.documentCurrent();
		} finally {
			try {
				await edit_session.end();
			} finally {
				scope.release();
			}
		}
	};
	const cycle_easing_kind = async (key_id: NodeId, direction: 1 | -1): Promise<boolean> => {
		const key = context.model.parsed_key_by_id.get(key_id);
		if (!key?.easing || !key.easing.params[DECL_KIND]) {
			return false;
		}
		const options = easing_kind_options_for_key(key);
		if (options.length === 0) {
			return false;
		}
		let current_index = options.findIndex((option) => option.value === key.easing?.kind);
		if (current_index < 0) {
			current_index = 0;
		}
		const next_index = (current_index + direction + options.length) % Math.max(1, options.length);
		const next_kind = options[next_index]?.value;
		if (!next_kind) {
			return false;
		}
		return apply_easing_kind_to_key_ids([key_id], next_kind, 'Change Curve Easing');
	};
	const open_curve_context_menu = (
		target: CurveContextMenuTarget,
		screen_x: number,
		screen_y: number
	): void => {
		context.state.curve_context_menu_target = target;
		context.state.curve_context_menu_anchor = {
			kind: 'point',
			x: screen_x,
			y: screen_y
		};
		context.state.curve_context_menu_open = easing_target_key_ids(target).length > 0;
	};
	const close_curve_context_menu = (): void => {
		context.state.curve_context_menu_open = false;
		context.state.curve_context_menu_anchor = null;
		context.state.curve_context_menu_target = null;
	};
	let curve_context_menu_items = $derived.by((): ContextMenuItem[] => {
		const target = context.state.curve_context_menu_target;
		if (!target) {
			return [];
		}
		const key_ids = easing_target_key_ids(target);
		if (key_ids.length === 0) {
			return [];
		}
		const options = easing_kind_options_for_key(context.model.parsed_key_by_id.get(key_ids[0]));
		if (options.length === 0) {
			return [];
		}
		const shared_kind = shared_easing_kind_for_key_ids(key_ids);
		const action_label = target.kind === 'all' ? 'Change All Curve Easings' : 'Change Curve Easing';
		return [
			{
				id: target.kind === 'all' ? 'change-all-easings' : 'change-easing',
				label: target.kind === 'all' ? 'Change All Easings' : 'Change Easing',
				submenu: options.map((option) => ({
					id: `curve-easing-${target.kind}-${option.value}`,
					label: option.label,
					hint: shared_kind === option.value ? 'Current' : undefined,
					action: () => {
						void apply_easing_kind_to_key_ids(key_ids, option.value, action_label);
					}
				}))
			}
		];
	});
	const trigger_canvas_key_add = (
		curve_point: {
			position: number;
			value: number;
		},
		curve_hit: CurveSegmentHit | null
	): void => {
		void add_key_at(curve_point.position, curve_point.value, {
			preserve_curve_shape: curve_hit !== null,
			preferred_owner_key_id: curve_hit?.owner_key_id ?? null
		});
		context.state.hover_key_id = null;
		context.state.hover_curve_owner_key_id = curve_hit?.owner_key_id ?? null;
		context.state.hover_bezier_handle = null;
	};
	const handle_canvas_double_activation = (
		key_id: NodeId | null,
		bezier_handle: BezierHandleRef | null,
		curve_hit: CurveSegmentHit | null,
		curve_point: {
			position: number;
			value: number;
		} | null
	): boolean => {
		if (key_id !== null) {
			void remove_key_ids([key_id]);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			return true;
		}
		if (bezier_handle || !curve_point) {
			return false;
		}
		trigger_canvas_key_add(curve_point, curve_hit);
		return true;
	};
	const handle_canvas_shortcut_click = (
		key_id: NodeId | null,
		curve_hit: CurveSegmentHit | null,
		curve_point: {
			position: number;
			value: number;
		} | null,
		shift_key: boolean
	): boolean => {
		if (key_id !== null) {
			void remove_key_ids([key_id]);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			return true;
		}
		if (curve_hit) {
			context.selection.set_single_selected_curve_owner_key(curve_hit.owner_key_id);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = curve_hit.owner_key_id;
			context.state.hover_bezier_handle = null;
			void cycle_easing_kind(curve_hit.owner_key_id, shift_key ? -1 : 1);
			return true;
		}
		if (!shift_key && curve_point) {
			trigger_canvas_key_add(curve_point, null);
			return true;
		}
		return false;
	};
	const add_key_at = async (
		position: number,
		value: number,
		options: AddKeyOptions = {},
		existingScope?: CurveOperation
	): Promise<ParsedCurveKey | null> => {
		const scope = existingScope ?? context.services.curveOperations.capture(),
			item = context.model.key_creatable_item;
		if (!scope || !item || context.services.creation.busy || !scope.current()) {
			if (!existingScope) scope?.release();
			return null;
		}
		try {
			const clamped = context.constraints.clamp_curve_point_to_active_range(position, value);
			let target_value = clamped.value;
			let split_easing: PendingSplitEasing | null = null;
			let easing_values = options.easing_values ?? null;
			let existing_easing_updates = options.existing_easing_updates ?? [];
			if (options.preserve_curve_shape) {
				const split_source = context.insertion.find_split_source_segment(
					clamped.position,
					options.preferred_owner_key_id ?? null
				);
				if (split_source) {
					const sampled_value = sample_compiled_curve(
						context.model.compiled_curve,
						clamped.position
					);
					if (sampled_value !== null && Number.isFinite(sampled_value)) {
						target_value = context.constraints.clamp_curve_value_to_active_range(sampled_value);
					}
					split_easing = context.insertion.build_split_easing(
						split_source.start,
						split_source.end,
						clamped.position,
						target_value
					);
				}
			} else {
				const split_source = context.insertion.find_split_source_segment(
					clamped.position,
					options.preferred_owner_key_id ?? null
				);
				if (split_source) {
					split_easing = context.insertion.build_middle_insert_easing(
						split_source.start,
						split_source.end,
						clamped.position,
						target_value
					);
				} else {
					const extension = context.insertion.build_endpoint_extension_easing(
						clamped.position,
						target_value
					);
					if (extension.new_key_easing_values) {
						easing_values = extension.new_key_easing_values;
					}
					if (extension.existing_easing_updates.length > 0) {
						existing_easing_updates = extension.existing_easing_updates;
					}
				}
			}
			return await context.services.creation.create(
				scope,
				item,
				{
					position: clamped.position,
					value: target_value,
					split_easing,
					easing_values,
					existing_easing_updates
				},
				existingScope === undefined
			);
		} finally {
			if (!existingScope) scope.release();
		}
	};
	const clone_param_value = (value: ParamValue): ParamValue => {
		return JSON.parse(JSON.stringify(value)) as ParamValue;
	};
	const remove_key_ids = async (key_ids: readonly NodeId[]): Promise<boolean> => {
		const ids = context.selection.ordered_node_ids(key_ids);
		if (
			ids.length === 0 ||
			ids.some(
				(id) => !context.doc.graphNodesById?.get(id)?.meta.user_permissions.can_remove_and_duplicate
			)
		) {
			return false;
		}
		const id_set = new Set(ids);
		const removed = await sendRemoveNodesIntent(ids);
		if (!removed) {
			return false;
		}
		context.bindings.selected_curve_owner_key_ids =
			context.bindings.selected_curve_owner_key_ids.filter((key_id) => !id_set.has(key_id));
		const remaining_selected_keys = context.bindings.selected_key_ids.filter(
			(key_id) => !id_set.has(key_id)
		);
		if (remaining_selected_keys.length > 0) {
			context.bindings.selected_key_ids = remaining_selected_keys;
			if (
				context.bindings.selected_key_id === null ||
				id_set.has(context.bindings.selected_key_id)
			) {
				context.bindings.selected_key_id = remaining_selected_keys[0] ?? null;
			}
			return true;
		}
		if (
			context.bindings.selected_key_id !== null &&
			!id_set.has(context.bindings.selected_key_id)
		) {
			context.bindings.selected_key_ids = [context.bindings.selected_key_id];
			return true;
		}
		const fallback =
			context.model.parsed_keys.find((entry) => !id_set.has(entry.node_id))?.node_id ?? null;
		context.selection.set_single_selected_key(fallback);
		return true;
	};
	const remove_selected_keys = async (): Promise<boolean> => {
		return remove_key_ids(context.selection.selected_key_ids_ordered());
	};
	const key_anchor_point = (): {
		position: number;
		value: number;
	} => {
		return context.state.hover_curve_position ?? context.camera.axis_midpoint();
	};
	const easing_values_for_key = (key: ParsedCurveKey): Record<string, ParamValue> => {
		const values: Record<string, ParamValue> = {};
		if (!key.easing) {
			return values;
		}
		for (const [decl_id, ref] of Object.entries(key.easing.params)) {
			values[decl_id] = clone_param_value(ref.value);
		}
		return values;
	};
	const selected_keys_as_clipboard_entries = (): CurveClipboardKey[] => {
		const ordered_ids = context.selection.selected_key_ids_ordered();
		if (ordered_ids.length === 0) {
			return [];
		}
		const ordered_keys = ordered_ids
			.map((key_id) => context.model.parsed_key_by_id.get(key_id))
			.filter((key): key is ParsedCurveKey => key !== undefined);
		const anchor = ordered_keys[0];
		if (!anchor) {
			return [];
		}
		return ordered_keys.map((key) => ({
			position_offset: key.position - anchor.position,
			value_offset: key.value - anchor.value,
			easing_values: easing_values_for_key(key)
		}));
	};
	const apply_clipboard_entries_at_anchor = async (
		entries: CurveClipboardKey[]
	): Promise<boolean> => {
		if (entries.length === 0) return false;
		const scope = context.services.curveOperations.capture();
		if (!scope) return false;
		const anchor = key_anchor_point(),
			copied = JSON.parse(JSON.stringify(entries)) as CurveClipboardKey[];
		let last: ParsedCurveKey | null = null;
		try {
			for (const entry of copied) {
				if (!scope.current()) return false;
				last = await add_key_at(
					anchor.position + entry.position_offset,
					anchor.value + entry.value_offset,
					{ easing_values: entry.easing_values },
					scope
				);
				if (!last) return false;
			}
			if (last && scope.selectionCurrent()) context.services.creation.select(scope, last);
			return scope.documentCurrent();
		} finally {
			scope.release();
		}
	};
	const copy_selected_keys_to_clipboard = (): boolean => {
		const entries = selected_keys_as_clipboard_entries();
		curveClipboardState.sourceCurveNodeId = context.doc.liveNode.node_id;
		curveClipboardState.keys = entries;
		return entries.length > 0;
	};
	const cut_selected_keys_to_clipboard = async (): Promise<boolean> => {
		const copied = copy_selected_keys_to_clipboard();
		if (!copied) {
			return false;
		}
		await remove_selected_keys();
		return true;
	};
	const duplicate_selected_keys = async (): Promise<boolean> => {
		if (
			context.selection
				.selected_key_ids_ordered()
				.some(
					(id) =>
						!context.doc.graphNodesById?.get(id)?.meta.user_permissions.can_remove_and_duplicate
				)
		)
			return false;
		return apply_clipboard_entries_at_anchor(selected_keys_as_clipboard_entries());
	};
	const paste_clipboard_keys = async (): Promise<boolean> => {
		return apply_clipboard_entries_at_anchor(curveClipboardState.keys);
	};
	const remove_selected_key = async (): Promise<void> => {
		await remove_selected_keys();
	};
	return {
		easing_kind_options_for_key,
		apply_easing_param_values,
		easing_target_key_ids,
		shared_easing_kind_for_key_ids,
		apply_easing_kind_to_key_ids,
		cycle_easing_kind,
		open_curve_context_menu,
		close_curve_context_menu,
		get curve_context_menu_items() {
			return curve_context_menu_items;
		},
		trigger_canvas_key_add,
		handle_canvas_double_activation,
		handle_canvas_shortcut_click,
		add_key_at,
		clone_param_value,
		remove_key_ids,
		remove_selected_keys,
		key_anchor_point,
		easing_values_for_key,
		selected_keys_as_clipboard_entries,
		apply_clipboard_entries_at_anchor,
		copy_selected_keys_to_clipboard,
		cut_selected_keys_to_clipboard,
		duplicate_selected_keys,
		paste_clipboard_keys,
		remove_selected_key
	};
}
