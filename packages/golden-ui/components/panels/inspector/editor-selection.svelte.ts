import { untrack } from 'svelte';
import type { WorkbenchSession } from '../../../store/workbench.svelte';
import type { SelectionGraph } from '../../../store/selection-hierarchy';
import type { UiNodeDto, NodeId } from '../../../types';

export interface InspectorEditorSelectionHost {
	readonly status: WorkbenchSession['status'];
	readonly loading: { readonly visible: boolean };
	readonly projectGeneration: number | null;
	readonly snapshotGeneration: number;
	readonly selectedNodesIds: readonly NodeId[];
	readonly graph: { readonly state: SelectionGraph };
	selectNodes: WorkbenchSession['selectNodes'];
}
interface Context<T> {
	readonly session: InspectorEditorSelectionHost | null;
	readonly owner: UiNodeDto;
	readonly publishToWorkbench?: boolean;
	read(): T;
	write(selection: T): void;
	empty(): T;
	signature(selection: T): string;
	project(
		graph: SelectionGraph | null,
		ids: readonly NodeId[],
		owner: UiNodeDto
	): T | null | undefined;
	resolve(graph: SelectionGraph, owner: NodeId, selection: T): NodeId[] | null;
	retainPrimary?(selected: T, current: T): T;
}

/** Publish only input-owned selection changes; model cleanup and stale scopes cannot steal focus. */
export function createInspectorEditorSelection<T>(context: Context<T>) {
	let lastProjection: T | null | undefined;
	let projectedSession: InspectorEditorSelectionHost | null = null;
	let signature = '';
	let input: {
		session: InspectorEditorSelectionHost;
		project: number | null;
		snapshot: number;
		owner: string;
		selected: readonly NodeId[];
	} | null = null;
	const publish = (
		selected: T,
		session: InspectorEditorSelectionHost | null,
		owner: UiNodeDto,
		explicit = false
	) => {
		const next = context.signature(selected);
		if (
			(!explicit && next === signature) ||
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null ||
			!input ||
			input.session !== session ||
			input.project !== session.projectGeneration ||
			input.snapshot !== session.snapshotGeneration ||
			input.owner !== owner.uuid ||
			input.selected !== session.selectedNodesIds
		)
			return false;
		const nodes = context.resolve(session.graph.state, owner.node_id, selected);
		if (!nodes) return false;
		signature = next;
		if (explicit) context.write(selected);
		if (context.publishToWorkbench !== false) {
			session.selectNodes(nodes, 'REPLACE');
			input.selected = session.selectedNodesIds;
		}
		return true;
	};
	$effect(() => {
		const session = context.session,
			owner = context.owner;
		// A graph publication must not replay the pre-input selection over an admitted local edit.
		untrack(() => publish(context.read(), session, owner));
		const selected = context.project(
			session?.graph.state ?? null,
			session?.selectedNodesIds ?? [],
			owner
		);
		if (selected === lastProjection && session === projectedSession) return;
		lastProjection = selected;
		projectedSession = session;
		if (selected === null || selected === undefined) return;
		untrack(() => {
			const current = context.read();
			const next = context.retainPrimary?.(selected, current) ?? selected;
			signature = context.signature(next);
			if (signature !== context.signature(current)) context.write(next);
		});
	});
	$effect(() => {
		const selected = context.read(),
			session = context.session,
			owner = context.owner;
		untrack(() => publish(selected, session, owner));
	});
	return {
		// A field and its parent can project to the same editor item. Input still chooses exact nodes.
		select(selected: T) {
			return publish(selected, context.session, context.owner, true);
		},
		clear() {
			const session = context.session,
				owner = context.owner;
			if (
				!session ||
				session.status !== 'connected' ||
				session.loading.visible ||
				session.projectGeneration === null ||
				session.graph.state.nodesById.get(owner.node_id)?.uuid !== owner.uuid
			)
				return;
			const empty = context.empty();
			signature = context.signature(empty);
			context.write(empty);
			if (context.publishToWorkbench !== false) {
				session.selectNodes([owner.node_id], 'REPLACE');
				if (input) input.selected = session.selectedNodesIds;
			}
		},
		arm(event: Event) {
			const session = context.session;
			if (
				!session ||
				session.projectGeneration === null ||
				session.loading.visible ||
				session.status !== 'connected' ||
				(event.target !== null &&
					event.target instanceof Element &&
					event.target.closest('input, textarea, select, [contenteditable="true"]'))
			)
				return;
			input = {
				session,
				project: session.projectGeneration,
				snapshot: session.snapshotGeneration,
				owner: context.owner.uuid,
				selected: session.selectedNodesIds
			};
		}
	};
}
