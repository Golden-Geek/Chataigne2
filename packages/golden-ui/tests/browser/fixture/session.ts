import { createWorkbenchSession } from '../../../store/workbench.svelte';
import { UI_PROTOCOL_VERSION } from '../../../generated/rust_protocol/protocol-version';
import {
	wholeGraphScope,
	type UiClient,
	type UiNodeDto,
	type UiSnapshot,
	type UiLogRecord,
	type ParamValue
} from '../../../types';

export function fixtureNode(
	id: number,
	label: string,
	children: number[] = [],
	type = 'FixtureItem'
): UiNodeDto {
	return {
		node_id: id,
		uuid: `fixture-${id}`,
		decl_id: '',
		node_type: type,
		meta: {
			label,
			short_name: String(id),
			enabled: true,
			can_be_disabled: false,
			tags: [],
			user_permissions: {
				can_edit_name: true,
				can_remove_and_duplicate: true,
				can_edit_constraints: false,
				can_edit_tags: true,
				can_edit_color: true
			}
		},
		data: { kind: 'node', node_type: type },
		user_role: 'regular',
		user_item_kind: '',
		accepted_user_item_kinds: [],
		creatable_user_items: [],
		children
	};
}

export function createFixtureSession() {
	let snapshot: UiSnapshot;
	let rejectNext = false;
	let sequence = 0;
	let generation = 0;
	let intentHandler:
		| ((intent: Parameters<UiClient['sendIntent']>[0]) => ReturnType<UiClient['sendIntent']> | null)
		| null = null;
	const unused = (): never => {
		throw new Error('Unexpected fixture transport call.');
	};
	const client: UiClient = {
		snapshot: async () => snapshot,
		subscribe: unused,
		sendIntents: async (intents) => Promise.all(intents.map((intent) => client.sendIntent(intent))),
		replay: unused,
		referenceTargets: unused,
		paramControlInfo: unused,
		scriptState: unused,
		setScriptConfig: unused,
		reloadScript: unused,
		projectNew: unused,
		projectSave: unused,
		projectLoad: unused,
		projectUploadLoad: unused,
		sendIntent: async (intent) => {
			const handled = intentHandler?.(intent);
			if (handled) return handled;
			if (rejectNext) {
				rejectNext = false;
				return { success: false, status: 'rejected', error_message: 'Fixture rejection' };
			}
			if (intent.kind !== 'patchMeta') throw new Error(`Unexpected fixture intent ${intent.kind}.`);
			session.graph.applyEvent({
				time: { tick: ++sequence, micro: 0, seq: 0 },
				kind: { kind: 'metaChanged', node: intent.node, patch: intent.patch }
			});
			return { success: true, status: 'applied' };
		}
	};
	const session = createWorkbenchSession({
		transportFactory: () => client,
		enableGlobalShortcuts: false
	});
	const load = (count: number) => {
		const nodes = [
			fixtureNode(0, 'Project', [1], 'FixtureRoot'),
			fixtureNode(
				1,
				'Manager',
				Array.from({ length: count }, (_, row) => row + 2),
				'FixtureManager'
			),
			...Array.from({ length: count }, (_, row) =>
				fixtureNode(row + 2, `Item ${String(row).padStart(6, '0')}`)
			)
		];
		nodes[2] = {
			...nodes[2],
			node_type: 'FixtureFolder',
			data: { kind: 'node', node_type: 'FixtureFolder' },
			children: [count + 2],
			meta: { ...nodes[2].meta, presentation: { collapsed: true } }
		};
		for (let depth = 0; depth < 8; depth += 1)
			nodes.push(
				fixtureNode(
					count + 2 + depth,
					`Nested ${depth}`,
					depth < 7 ? [count + 3 + depth] : [],
					'FixtureFolder'
				)
			);
		loadNodes(nodes);
		return { rows: count, authoredNodes: nodes.length };
	};
	const loadNodes = (nodes: UiNodeDto[]) => {
		generation += 1;
		const references: Extract<ParamValue, { kind: 'reference' }>[] = [];
		for (const node of nodes) {
			node.uuid = `fixture-${generation}-${node.node_id}`;
			if (node.data.kind !== 'parameter') continue;
			for (const value of [node.data.param.value, node.data.param.default_value])
				if (value.kind === 'reference') references.push(value);
		}
		if (references.length) {
			const uuids = new Map(nodes.map((node) => [node.node_id, node.uuid]));
			for (const reference of references) {
				const uuid = reference.cached_id === undefined ? undefined : uuids.get(reference.cached_id);
				if (uuid) reference.uuid = uuid;
			}
		}
		snapshot = {
			protocol_version: UI_PROTOCOL_VERSION,
			scope: wholeGraphScope,
			at: { tick: 0, micro: 0, seq: 0 },
			nodes,
			schema: { node_types: [], declared_descriptions: [], enums: [] },
			history: {
				can_undo: false,
				can_redo: false,
				undo_len: 0,
				redo_len: 0,
				active_edit_session: false,
				current_history_state_id: 0
			},
			logger: { max_entries: 128, records: [] },
			project_file: { display_name: 'Fixture', extension: 'fixture', current_path: null }
		};
		session.graph.loadSnapshot(snapshot);
	};
	return {
		editAcknowledgement(active?: boolean): Awaited<ReturnType<UiClient['sendIntent']>> {
			if (active !== undefined)
				snapshot = { ...snapshot, history: { ...snapshot.history, active_edit_session: active } };
			return { success: true, status: 'applied', history: snapshot.history };
		},
		handleIntent(handler: typeof intentHandler) {
			intentHandler = handler;
		},
		session,
		load,
		loadNodes,
		loadLogger: async (records: UiLogRecord[], maxEntries = records.length) => {
			snapshot = { ...snapshot, logger: { max_entries: maxEntries, records } };
			if (!(await session.refreshSnapshot()))
				throw new Error('Logger fixture snapshot was rejected.');
		},
		rejectNext: () => {
			rejectNext = true;
		}
	};
}
