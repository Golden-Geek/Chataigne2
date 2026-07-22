import type {
	GraphEdge,
	GraphNode,
	GraphPresentationDocument,
	GraphRevision
} from 'golden_graph_ui';

export const STATECHART_INCOMING_PORT_ID = 'transition-input';
export const STATECHART_OUTGOING_PORT_ID = 'transition-output';

const INITIAL_REVISION: GraphRevision = {
	sequence: 0,
	topology: 0,
	payload: 0,
	presentation: 0
};

export interface StatechartPresentationSnapshot {
	readonly states: readonly GraphNode[];
	readonly transitions: readonly GraphEdge[];
}

/**
 * Statechart-specific presentation boundary for the shared Golden graph canvas.
 *
 * The current product transport invalidates all graph planes together. This class
 * keeps that conservative policy isolated from the canvas; it never emits commands
 * or evaluates statechart semantics.
 */
export class StatechartDocumentView {
	#states: readonly GraphNode[] | null = null;
	#transitions: readonly GraphEdge[] | null = null;
	#document: GraphPresentationDocument = {
		revision: INITIAL_REVISION,
		nodes: [],
		edges: []
	};

	update(snapshot: StatechartPresentationSnapshot): GraphPresentationDocument {
		if (snapshot.states === this.#states && snapshot.transitions === this.#transitions) {
			return this.#document;
		}
		this.#states = snapshot.states;
		this.#transitions = snapshot.transitions;
		const sequence = this.#document.revision.sequence + 1;
		this.#document = {
			revision: {
				sequence,
				topology: sequence,
				payload: sequence,
				presentation: sequence
			},
			nodes: snapshot.states,
			edges: snapshot.transitions
		};
		return this.#document;
	}
}
