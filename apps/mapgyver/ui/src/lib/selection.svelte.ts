import { getContext, setContext, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';

const key = Symbol('mapgyver-document-selection');

/** Resolve committed UUIDs when Golden's graph delta arrives; no polling or document mutations. */
export function provideDocumentSelection() {
	let request = $state.raw<{
		session: WorkbenchSession;
		generation: number;
		uuid: string;
		previous: number | null;
	} | null>(null);
	$effect(() => {
		const pending = request,
			session = appState.session;
		if (!pending) return;
		if (
			session !== pending.session ||
			session.snapshotGeneration !== pending.generation ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.selectedNodeId !== pending.previous
		) {
			request = null;
			return;
		}
		const node = [...session.graph.state.nodesById.values()].find(
			(node) => node.uuid === pending.uuid
		);
		if (node)
			untrack(() => {
				session.selectNode(node.node_id, 'REPLACE');
				request = null;
			});
	});
	const owner = {
		select(uuid: string) {
			const session = appState.session;
			if (session)
				request = {
					session,
					generation: session.snapshotGeneration,
					uuid,
					previous: session.selectedNodeId
				};
		}
	};
	setContext(key, owner);
	return owner;
}

export const useDocumentSelection = () =>
	getContext<ReturnType<typeof provideDocumentSelection>>(key);
