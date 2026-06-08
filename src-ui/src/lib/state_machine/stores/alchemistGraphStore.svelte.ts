import type { GraphConnectionRequest, GraphNodePosition } from 'golden_alchemist_ui';
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

	select(nodeIds: string[]): void {
		this.selectedNodeIds = new Set(nodeIds);
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

	moveNode(nodeId: string, position: GraphNodePosition): AlchemistNodeDto | null {
		const node = this.nodesById.get(nodeId);
		if (!node) return null;
		node.x = position.x;
		node.y = position.y;
		return node;
	}

	connect(connection: GraphConnectionRequest): AlchemistEdgeDto {
		const edge: AlchemistEdgeDto = {
			from_node: connection.from.nodeId,
			from_socket: connection.from.socketId,
			to_node: connection.to.nodeId,
			to_socket: connection.to.socketId
		};
		this.edges = [...this.edges, edge];
		return edge;
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
