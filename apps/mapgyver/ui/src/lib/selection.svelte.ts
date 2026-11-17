import { getContext, setContext, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { captureSelectionGuard } from './selection-guard';
import type { SelectionMode } from 'golden_ui/store/session/types';

const key = Symbol('mapgyver-document-selection');

/** Select committed UUIDs through Golden's index; defer only nodes awaiting their graph delta. */
export function provideDocumentSelection() {
	let request = $state.raw<{
		session: WorkbenchSession;
		uuid: string;
		mode: SelectionMode;
		current(session: WorkbenchSession | null): boolean;
	} | null>(null);
	$effect(() => {
		const pending = request,
			session = appState.session;
		if (!pending) return;
		if (session !== pending.session || !pending.current(session)) {
			request = null;
			return;
		}
		const node = session.graph.state.nodesById.getByUuid(pending.uuid);
		if (node)
			untrack(() => {
				session.selectNode(node.node_id, pending.mode);
				request = null;
			});
	});
	function capture() {
		const session = appState.session;
		const current = captureSelectionGuard(session);
		return (uuid: string, mode: SelectionMode = 'REPLACE') => {
			if (!session || !current(appState.session)) return false;
			const node = session.graph.state.nodesById.getByUuid(uuid);
			if (node) {
				request = null;
				session.selectNode(node.node_id, mode);
			} else request = { session, uuid, mode, current };
			return true;
		};
	}
	const owner = { capture, select: (uuid: string, mode?: SelectionMode) => capture()(uuid, mode) };
	setContext(key, owner);
	return owner;
}

export const useDocumentSelection = () =>
	getContext<ReturnType<typeof provideDocumentSelection>>(key);
