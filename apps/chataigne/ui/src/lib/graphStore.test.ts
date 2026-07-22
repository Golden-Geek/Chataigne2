import { describe, expect, it } from 'vitest';
import { createGraphStore } from '../../../../../packages/golden-ui/store/graph.svelte';
import type { UiNodeDto, UiSnapshot } from '../../../../../packages/golden-ui/types';

const eventTime = (seq: number) => ({ tick: 1, micro: 0, seq });

const parameterNode = (): UiNodeDto => ({
	node_id: 1,
	uuid: '00000000-0000-0000-0000-000000000001',
	decl_id: 'value',
	node_type: 'int',
	meta: {
		label: 'Value',
		short_name: 'value',
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
	data: {
		kind: 'parameter',
		param: {
			value: { kind: 'int', value: 0 },
			default_value: { kind: 'int', value: 0 },
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	},
	user_role: 'regular',
	user_item_kind: '',
	accepted_user_item_kinds: [],
	creatable_user_items: [],
	children: []
});

const snapshot = (): UiSnapshot =>
	({
		protocol_version: '0.2.0',
		scope: { kind: 'wholeGraph' },
		at: eventTime(0),
		nodes: [parameterNode()],
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
		project_file: { display_name: 'Test', extension: 'noisette', current_path: null }
	}) as UiSnapshot;

describe('graph store scaling', () => {
	it('patches a live parameter without copying the complete graph indexes', () => {
		const store = createGraphStore();
		store.loadSnapshot(snapshot());
		const previousState = store.state;
		const nodesById = previousState.nodesById;
		const childrenById = previousState.childrenById;
		const parentById = previousState.parentById;
		const paramsById = previousState.paramsById;

		store.applyBatch({
			from: eventTime(0),
			to: eventTime(1),
			events: [
				{
					time: eventTime(1),
					kind: {
						kind: 'paramChanged',
						param: 1,
						old_value: { kind: 'int', value: 0 },
						new_value: { kind: 'int', value: 42 }
					}
				}
			]
		});

		expect(store.state).not.toBe(previousState);
		expect(store.state.nodesById).toBe(nodesById);
		expect(store.state.childrenById).toBe(childrenById);
		expect(store.state.parentById).toBe(parentById);
		expect(store.state.paramsById).toBe(paramsById);
		expect(store.state.paramsById.get(1)?.value).toEqual({ kind: 'int', value: 42 });
	});
});
