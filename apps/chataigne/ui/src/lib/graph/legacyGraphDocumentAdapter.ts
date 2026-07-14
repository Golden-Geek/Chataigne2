import type {
	GraphEdge,
	GraphNode,
	GraphPresentationDocument,
	GraphRevision
} from 'golden_graph_ui';

const INITIAL_REVISION: GraphRevision = {
	sequence: 0,
	topology: 0,
	payload: 0,
	presentation: 0
};

/**
 * Pure temporary Phase 3 bridge from the product's pre-cutover DTO graph to the revisioned graph view.
 * It invalidates every plane conservatively because the old transport does not expose partitioned
 * graph revisions. It has no command, trigger, effect, or device authority. Delete it when the
 * generated graph protocol becomes the production source.
 */
export class LegacyGraphDocumentAdapter {
	#nodes: readonly GraphNode[] | null = null;
	#edges: readonly GraphEdge[] | null = null;
	#document: GraphPresentationDocument = {
		revision: INITIAL_REVISION,
		nodes: [],
		edges: []
	};

	update(nodes: readonly GraphNode[], edges: readonly GraphEdge[]): GraphPresentationDocument {
		if (nodes === this.#nodes && edges === this.#edges) {
			return this.#document;
		}
		this.#nodes = nodes;
		this.#edges = edges;
		const sequence = this.#document.revision.sequence + 1;
		this.#document = {
			revision: {
				sequence,
				topology: sequence,
				payload: sequence,
				presentation: sequence
			},
			nodes,
			edges
		};
		return this.#document;
	}
}
