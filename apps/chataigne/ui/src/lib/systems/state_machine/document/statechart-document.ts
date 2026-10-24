import {
	GraphPresentationAdapter,
	type GraphEdge,
	type GraphNode,
	type GraphPresentationDocument
} from 'golden_graph_ui';

export const STATECHART_INCOMING_PORT_ID = 'transition-input';
export const STATECHART_OUTGOING_PORT_ID = 'transition-output';

export interface StatechartPresentationSnapshot {
	readonly states: readonly GraphNode[];
	readonly transitions: readonly GraphEdge[];
}

/**
 * Statechart-specific presentation boundary for the shared Golden graph canvas.
 *
 * Adapts state/transition vocabulary; Golden owns presentation invalidation.
 * It never emits commands or evaluates statechart semantics.
 */
export class StatechartDocumentView {
	#presentation = new GraphPresentationAdapter();

	update(snapshot: StatechartPresentationSnapshot): GraphPresentationDocument {
		return this.#presentation.update(snapshot.states, snapshot.transitions);
	}
}
