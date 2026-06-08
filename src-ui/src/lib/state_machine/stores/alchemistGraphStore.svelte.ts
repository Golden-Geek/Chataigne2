import type { AlchemistEdgeDto, AlchemistNodeDto } from '../generated';

export interface GraphViewport {
	x: number;
	y: number;
	width: number;
	height: number;
}

export class AlchemistGraphStore {
	nodesById = $state(new Map<string, AlchemistNodeDto>());
	edges = $state<AlchemistEdgeDto[]>([]);
	selectedNodeIds = $state(new Set<string>());
	dragOffsets = $state(new Map<string, { x: number; y: number }>());

	replace(nodes: AlchemistNodeDto[], edges: AlchemistEdgeDto[]): void {
		this.nodesById = new Map(nodes.map((node) => [node.id, node]));
		this.edges = edges;
	}

	setDrag(nodeId: string, x: number, y: number): void {
		this.dragOffsets.set(nodeId, { x, y });
	}

	commitDrag(nodeId: string): AlchemistNodeDto | null {
		const node = this.nodesById.get(nodeId);
		const drag = this.dragOffsets.get(nodeId);
		if (!node || !drag) return null;
		node.x = drag.x;
		node.y = drag.y;
		this.dragOffsets.delete(nodeId);
		return node;
	}

	visibleNodes(viewport: GraphViewport, margin = 8): AlchemistNodeDto[] {
		return [...this.nodesById.values()].filter(
			(node) =>
				node.x >= viewport.x - margin &&
				node.x <= viewport.x + viewport.width + margin &&
				node.y >= viewport.y - margin &&
				node.y <= viewport.y + viewport.height + margin
		);
	}
}
