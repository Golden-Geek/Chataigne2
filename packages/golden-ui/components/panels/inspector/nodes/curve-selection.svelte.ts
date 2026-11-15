import { untrack } from 'svelte';
import type { WorkbenchSession } from '../../../../store/workbench.svelte';
import type { SelectionGraph } from '../../../../store/selection-hierarchy';
import type { UiNodeDto, NodeId } from '../../../../types';
import {
	createCurveSelectionProjection,
	curveSelectionNodes,
	curveSelectionSignature,
	type CurveSelection
} from './curve-selection';

export interface CurveSelectionHost {
	readonly status: WorkbenchSession['status'];
	readonly loading: { readonly visible: boolean };
	readonly projectGeneration: number | null;
	readonly snapshotGeneration: number;
	readonly selectedNodesIds: readonly NodeId[];
	readonly graph: { readonly state: SelectionGraph };
	selectNodes: WorkbenchSession['selectNodes'];
}
interface Context {
	readonly session: CurveSelectionHost | null;
	readonly owner: UiNodeDto;
	read(): CurveSelection;
	write(selection: CurveSelection): void;
}

/** Publish only input-owned selection changes; model cleanup and stale scopes cannot steal focus. */
export function createCurveInspectorSelection(context: Context) {
	const projection = createCurveSelectionProjection();
	let lastProjection: CurveSelection | null | undefined;
	let projectedSession: CurveSelectionHost | null = null;
	let signature = '';
	let input: {
		session: CurveSelectionHost;
		project: number | null;
		snapshot: number;
		owner: string;
		selected: readonly NodeId[];
	} | null = null;
	$effect(() => {
		const session = context.session,
			owner = context.owner;
		const selected = projection.read(
			session?.graph.state ?? null,
			session?.selectedNodesIds ?? [],
			owner.node_id,
			owner.uuid
		);
		if (selected === lastProjection && session === projectedSession) return;
		lastProjection = selected;
		projectedSession = session;
		if (!selected) return;
		untrack(() => {
			const current = context.read();
			const next = {
				...selected,
				key:
					current.key !== null && selected.keys.includes(current.key) ? current.key : selected.key
			};
			signature = curveSelectionSignature(next);
			if (signature !== curveSelectionSignature(current)) context.write(next);
		});
	});
	$effect(() => {
		const selected = context.read(),
			session = context.session,
			owner = context.owner;
		untrack(() => {
			const next = curveSelectionSignature(selected);
			if (
				next === signature ||
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
				return;
			const nodes = curveSelectionNodes(session.graph.state, owner.node_id, selected);
			if (!nodes) return;
			signature = next;
			session.selectNodes(nodes, 'REPLACE');
			input.selected = session.selectedNodesIds;
		});
	});
	return {
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
