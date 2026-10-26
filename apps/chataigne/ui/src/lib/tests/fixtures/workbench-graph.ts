import { UI_PROTOCOL_VERSION } from 'golden_ui/generated/rust_protocol/protocol-version.ts';
import type { ParamValue, UiNodeDto, UiSnapshot } from 'golden_ui/types';

export const node = (nodeId: number): UiNodeDto =>
	({
		node_id: nodeId,
		uuid: `00000000-0000-0000-0000-${nodeId.toString().padStart(12, '0')}`,
		decl_id: 'root',
		node_type: 'folder',
		meta: {
			label: 'Root',
			short_name: 'root',
			enabled: true,
			can_be_disabled: false,
			user_permissions: {
				can_edit_name: false,
				can_remove_and_duplicate: false,
				can_edit_constraints: false,
				can_edit_tags: false,
				can_edit_color: false
			},
			tags: []
		},
		data: { kind: 'node', node_type: 'folder' },
		user_role: 'regular',
		user_item_kind: '',
		accepted_user_item_kinds: [],
		creatable_user_items: [],
		children: []
	}) as UiNodeDto;

export const parameterNode = (id: number, declaration: string, value: ParamValue): UiNodeDto => ({
	...node(id),
	decl_id: declaration,
	data: {
		kind: 'parameter',
		param: {
			value,
			default_value: value,
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	}
});

export const snapshot = (nodeId: number, seq: number): UiSnapshot =>
	({
		project_generation: 1,
		protocol_version: UI_PROTOCOL_VERSION,
		scope: { kind: 'wholeGraph' },
		at: { tick: 1, micro: 0, seq },
		nodes: [node(nodeId)],
		schema: { node_types: [], declared_descriptions: [], enums: [] },
		history: {
			can_undo: false,
			can_redo: false,
			undo_len: 0,
			redo_len: 0,
			active_edit_session: false,
			current_history_state_id: 0
		},
		logger: { max_entries: 100, records: [] },
		project_file: {
			display_name: 'Test',
			extension: 'noisette',
			current_path: null
		}
	}) as UiSnapshot;
