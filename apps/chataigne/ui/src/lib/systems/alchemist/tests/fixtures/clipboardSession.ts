import { createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
import { UI_PROTOCOL_VERSION } from 'golden_ui/generated/rust_protocol/protocol-version.ts';
import type { UiAck, UiClient, UiEventBatch, UiNodeDto, UiSnapshot } from 'golden_ui';

export const clipboardNode = (node_id: number, uuid: string): UiNodeDto => ({
	node_id,
	uuid,
	decl_id: '',
	node_type: 'folder',
	meta: {
		label: uuid,
		short_name: uuid,
		enabled: true,
		can_be_disabled: false,
		user_permissions: {
			can_edit_name: true,
			can_remove_and_duplicate: true,
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
});

export const clipboardSession = async () => {
	const time = (seq: number) => ({ tick: 1, micro: 0, seq });
	let subscribed!: () => void;
	const ready = new Promise<void>((resolve) => {
		subscribed = resolve;
	});
	let respond!: (ack: UiAck) => void;
	const ack = new Promise<UiAck>((resolve) => {
		respond = resolve;
	});
	let onBatch!: (batch: UiEventBatch) => void;
	let seq = 0;
	const children: number[] = [];
	let snapshot: UiSnapshot = {
		protocol_version: UI_PROTOCOL_VERSION,
		scope: { kind: 'wholeGraph' },
		at: time(0),
		nodes: [clipboardNode(1, 'project-root')],
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
	};
	const client = {
		snapshot: async () => snapshot,
		subscribe(_scope: unknown, _from: unknown, listener: (batch: UiEventBatch) => void) {
			onBatch = listener;
			subscribed();
			return () => {};
		},
		sendIntent: () => ack
	} as unknown as UiClient;
	const session = createWorkbenchSession({
		enableGlobalShortcuts: false,
		transportFactory: () => client
	});
	const dispose = session.mount();
	await ready;
	return {
		session,
		dispose,
		respond,
		async replaceProject(nodes: UiNodeDto[]) {
			snapshot = { ...snapshot, at: time(++seq), nodes };
			await session.runProjectGraphTransition(async (control) => {
				control.markRuntimeGraphReplaced();
				await session.refreshSnapshotAfterCurrentSynchronization();
			});
		},
		emit(node: UiNodeDto) {
			children.push(node.node_id);
			const from = seq++;
			onBatch({
				from: time(from),
				to: time(seq),
				events: [
					{
						time: time(seq),
						kind: {
							kind: 'graphTransaction',
							tx_id: seq,
							epoch: 1,
							base_graph_version: from,
							next_graph_version: seq,
							ops: [
								{
									kind: 'subtreeInserted',
									root: node.node_id,
									parent: 1,
									nodes: [node],
									parent_children_after: [...children]
								}
							]
						}
					}
				]
			});
		}
	};
};
