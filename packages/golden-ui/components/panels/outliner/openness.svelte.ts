import { untrack } from 'svelte';
import type { GraphState } from '../../../store/graph.svelte';
import { sendPatchMetaIntent } from '../../../store/ui-intents';
import type { NodeId } from '../../../types';

interface OpennessEntry {
	uuid: string;
	expanded: boolean;
	observedCollapsed: boolean;
	waiting: boolean;
	token: object;
}

/** Openness survives row unmounts; backend presentation changes and rejected edits remain authoritative. */
export function createNodeOpenness(
	graph: () => GraphState | null,
	context: () => unknown = () => null
) {
	let entries = $state.raw<Record<string, OpennessEntry>>({});
	let scope = untrack(context);
	const replace = (id: string, token: object, value: OpennessEntry | undefined) => {
		if (entries[id]?.token !== token) return;
		const next = { ...entries };
		if (value) next[id] = value;
		else delete next[id];
		entries = next;
	};
	$effect(() => {
		const nextProducer = graph()?.nodesById;
		const nextScope = context();
		if (nextScope !== scope) {
			scope = nextScope;
			entries = {};
		}
		for (const [id, entry] of Object.entries(entries)) {
			const node = nextProducer?.get(Number(id));
			const collapsed = node?.meta.presentation?.collapsed === true;
			if (
				!node ||
				node.uuid !== entry.uuid ||
				(!entry.waiting && collapsed !== entry.observedCollapsed)
			)
				untrack(() => replace(id, entry.token, undefined));
			else if (entry.waiting && collapsed === !entry.expanded)
				untrack(() =>
					replace(id, entry.token, { ...entry, waiting: false, observedCollapsed: collapsed })
				);
		}
	});
	let overrides = $derived(
		Object.fromEntries(Object.entries(entries).map(([id, entry]) => [id, entry.expanded]))
	);
	const setExpanded = (nodeId: NodeId, expanded: boolean): void => {
		const node = graph()?.nodesById.get(nodeId);
		if (!node) return;
		const id = String(nodeId),
			previous = entries[id],
			token = {};
		entries = {
			...entries,
			[id]: {
				uuid: node.uuid,
				expanded,
				observedCollapsed: node.meta.presentation?.collapsed === true,
				waiting: true,
				token
			}
		};
		void sendPatchMetaIntent(nodeId, {
			presentation: { ...(node.meta.presentation ?? {}), collapsed: !expanded }
		}).then(
			(accepted) => {
				if (!accepted) replace(id, token, previous);
			},
			() => replace(id, token, previous)
		);
	};
	return {
		get overrides() {
			return overrides;
		},
		setExpanded
	};
}
