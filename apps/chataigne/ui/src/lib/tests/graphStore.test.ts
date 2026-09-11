import { describe, expect, it } from 'vitest';
import { UI_PROTOCOL_VERSION } from '../../../../../../packages/golden-ui/generated/rust_protocol/protocol-version';
import {
	VersionedNodeMap,
	graphIndexMutationCopies
} from '../../../../../../packages/golden-ui/store/graph-index';
import { createGraphStore } from '../../../../../../packages/golden-ui/store/graph.svelte';
import type { UiNodeDto, UiSnapshot } from '../../../../../../packages/golden-ui/types';

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
		protocol_version: UI_PROTOCOL_VERSION,
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
	it('forks numeric indexes without copying prior entries', () => {
		const original = new VersionedNodeMap<string>(
			Array.from({ length: 10_000 }, (_, index) => [index, `value-${index}`] as const)
		);
		const fork = original.fork();

		fork.set(5_000, 'updated');
		fork.set(10_000, 'added');
		fork.delete(7_500);

		expect(original.size).toBe(10_000);
		expect(original.get(5_000)).toBe('value-5000');
		expect(original.get(10_000)).toBeUndefined();
		expect(original.get(7_500)).toBe('value-7500');
		expect(fork.size).toBe(10_000);
		expect(fork.get(5_000)).toBe('updated');
		expect(fork.get(10_000)).toBe('added');
		expect(fork.get(7_500)).toBeUndefined();
		expect([...fork.keys()]).toHaveLength(10_000);
		expect(graphIndexMutationCopies(fork)).toBeLessThanOrEqual(30);
	});

	it('retains repeated versions with bounded changed-path copies', () => {
		const retained = [
			new VersionedNodeMap<string>(
				Array.from({ length: 10_000 }, (_, index) => [index, 'version-0'] as const)
			)
		];
		let copiedTrieNodes = 0;

		for (let version = 1; version <= 1_000; version += 1) {
			const next = retained[version - 1]?.fork();
			if (!next) {
				throw new Error('previous graph index version is missing');
			}
			next.set(5_000, `version-${version}`);
			copiedTrieNodes += graphIndexMutationCopies(next) ?? Number.MAX_VALUE;
			retained.push(next);
		}

		expect(retained[0]?.get(5_000)).toBe('version-0');
		expect(retained[500]?.get(5_000)).toBe('version-500');
		expect(retained[1_000]?.get(5_000)).toBe('version-1000');
		expect(retained.every((version) => version.size === 10_000)).toBe(true);
		expect(copiedTrieNodes).toBeLessThanOrEqual(9_000);
	});

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
		expect(store.state.nodesById).not.toBe(nodesById);
		expect(store.state.childrenById).not.toBe(childrenById);
		expect(store.state.parentById).not.toBe(parentById);
		expect(store.state.paramsById).not.toBe(paramsById);
		expect(paramsById.get(1)?.value).toEqual({ kind: 'int', value: 0 });
		expect(store.state.paramsById.get(1)?.value).toEqual({ kind: 'int', value: 42 });
		expect(graphIndexMutationCopies(store.state.nodesById)).toBeLessThanOrEqual(9);
		expect(graphIndexMutationCopies(store.state.paramsById)).toBeLessThanOrEqual(9);
		expect(graphIndexMutationCopies(store.state.childrenById)).toBe(0);
		expect(graphIndexMutationCopies(store.state.parentById)).toBe(0);
	});

	it('does not invalidate graph consumers for preview-only custom events', () => {
		const store = createGraphStore();
		store.loadSnapshot(snapshot());
		const previousState = store.state;

		const changed = store.applyBatch({
			from: eventTime(0),
			to: eventTime(1),
			events: [
				{
					time: eventTime(1),
					kind: {
						kind: 'custom',
						topic: 'chataigne.state_machine.runtime_preview',
						payload: { lanes: [] },
						retention: 'transient'
					}
				}
			]
		});

		expect(changed).toBe(false);
		expect(store.state).toBe(previousState);
		expect(store.state.lastEventTime).toEqual(eventTime(0));
	});

	it('only invalidates for an applicable transport resync event', () => {
		const store = createGraphStore();
		store.loadSnapshot(snapshot());
		const previousState = store.state;

		const ignored = store.applyBatch({
			from: eventTime(0),
			to: eventTime(1),
			events: [
				{
					time: eventTime(1),
					kind: {
						kind: 'custom',
						topic: '__transport.resync_required',
						payload: { reason: 'script_reload_requested' },
						retention: 'transient'
					}
				}
			]
		});

		expect(ignored).toBe(false);
		expect(store.state).toBe(previousState);
		expect(store.state.lastEventTime).toEqual(eventTime(0));

		const changed = store.applyBatch({
			from: eventTime(1),
			to: eventTime(2),
			events: [
				{
					time: eventTime(2),
					kind: {
						kind: 'custom',
						topic: '__transport.resync_required',
						payload: { reason: 'cursor_out_of_retention_window' },
						retention: 'transient'
					}
				}
			]
		});

		expect(changed).toBe(true);
		expect(store.state).not.toBe(previousState);
		expect(store.state.requiresResync).toBe(true);
		expect(store.state.lastEventTime).toEqual(eventTime(2));
	});

	it('only invalidates for an applicable processor-manager projection', () => {
		const store = createGraphStore();
		store.loadSnapshot(snapshot());
		const previousState = store.state;

		const ignored = store.applyBatch({
			from: eventTime(0),
			to: eventTime(1),
			events: [
				{
					time: eventTime(1),
					kind: {
						kind: 'custom',
						topic: 'state_processor_manager_items_changed',
						origin: 999,
						payload: [],
						retention: 'latest'
					}
				}
			]
		});

		expect(ignored).toBe(false);
		expect(store.state).toBe(previousState);

		const changed = store.applyBatch({
			from: eventTime(1),
			to: eventTime(2),
			events: [
				{
					time: eventTime(2),
					kind: {
						kind: 'custom',
						topic: 'state_processor_manager_items_changed',
						origin: 1,
						payload: [],
						retention: 'latest'
					}
				}
			]
		});

		expect(changed).toBe(true);
		expect(store.state).not.toBe(previousState);
		expect(store.state.lastEventTime).toEqual(eventTime(2));
	});
});
