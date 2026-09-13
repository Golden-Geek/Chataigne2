import type { GraphNode, GraphNodePosition } from 'golden_graph_ui';

export const DEFAULT_STATE_WIDTH_REM = 13;
export const DEFAULT_STATE_HEIGHT_REM = 8;

const STATE_PLACEMENT_CLEARANCE_REM = 1;
const STATE_PLACEMENT_STEP_REM = 2;
const STATE_PLACEMENT_INDEX_CELL_REM = 8;
const STATE_PLACEMENT_MAX_RING = 128;

const rectanglesOverlap = (
	left: GraphNodePosition,
	right: GraphNodePosition,
	rightWidth: number,
	rightHeight: number
): boolean =>
	left.x < right.x + rightWidth + STATE_PLACEMENT_CLEARANCE_REM &&
	left.x + DEFAULT_STATE_WIDTH_REM + STATE_PLACEMENT_CLEARANCE_REM > right.x &&
	left.y < right.y + rightHeight + STATE_PLACEMENT_CLEARANCE_REM &&
	left.y + DEFAULT_STATE_HEIGHT_REM + STATE_PLACEMENT_CLEARANCE_REM > right.y;

const placementCellKey = (x: number, y: number): string => `${x}:${y}`;

const statePlacementIndex = (
	nodes: readonly GraphNode[],
	nodeWidth: (node: GraphNode) => number,
	nodeHeight: (node: GraphNode) => number
): Map<string, GraphNode[]> => {
	const index = new Map<string, GraphNode[]>();
	for (const node of nodes) {
		const width = nodeWidth(node);
		const height = nodeHeight(node);
		const firstX = Math.floor(
			(node.position.x - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		const lastX = Math.floor(
			(node.position.x + width + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		const firstY = Math.floor(
			(node.position.y - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		const lastY = Math.floor(
			(node.position.y + height + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		for (let cellX = firstX; cellX <= lastX; cellX += 1) {
			for (let cellY = firstY; cellY <= lastY; cellY += 1) {
				const key = placementCellKey(cellX, cellY);
				const occupants = index.get(key);
				if (occupants) {
					occupants.push(node);
				} else {
					index.set(key, [node]);
				}
			}
		}
	}
	return index;
};

const placementIsFree = (
	candidate: GraphNodePosition,
	index: Map<string, GraphNode[]>,
	nodeWidth: (node: GraphNode) => number,
	nodeHeight: (node: GraphNode) => number
): boolean => {
	const firstX = Math.floor(
		(candidate.x - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
	);
	const lastX = Math.floor(
		(candidate.x + DEFAULT_STATE_WIDTH_REM + STATE_PLACEMENT_CLEARANCE_REM) /
			STATE_PLACEMENT_INDEX_CELL_REM
	);
	const firstY = Math.floor(
		(candidate.y - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
	);
	const lastY = Math.floor(
		(candidate.y + DEFAULT_STATE_HEIGHT_REM + STATE_PLACEMENT_CLEARANCE_REM) /
			STATE_PLACEMENT_INDEX_CELL_REM
	);
	const nearbyNodes = new Set<GraphNode>();
	for (let cellX = firstX; cellX <= lastX; cellX += 1) {
		for (let cellY = firstY; cellY <= lastY; cellY += 1) {
			for (const node of index.get(placementCellKey(cellX, cellY)) ?? []) {
				nearbyNodes.add(node);
			}
		}
	}
	for (const node of nearbyNodes) {
		if (rectanglesOverlap(candidate, node.position, nodeWidth(node), nodeHeight(node))) {
			return false;
		}
	}
	return true;
};

export const nearestFreeStatePosition = (
	center: GraphNodePosition,
	nodes: readonly GraphNode[],
	nodeWidth: (node: GraphNode) => number,
	nodeHeight: (node: GraphNode) => number
): GraphNodePosition => {
	const index = statePlacementIndex(nodes, nodeWidth, nodeHeight);
	const origin = {
		x: Math.round((center.x - DEFAULT_STATE_WIDTH_REM * 0.5) * 2) / 2,
		y: Math.round((center.y - DEFAULT_STATE_HEIGHT_REM * 0.5) * 2) / 2
	};
	if (placementIsFree(origin, index, nodeWidth, nodeHeight)) {
		return origin;
	}
	for (let ring = 1; ring <= STATE_PLACEMENT_MAX_RING; ring += 1) {
		const offsets: GraphNodePosition[] = [];
		for (let axis = -ring; axis <= ring; axis += 1) {
			offsets.push(
				{ x: axis, y: -ring },
				{ x: axis, y: ring },
				{ x: -ring, y: axis },
				{ x: ring, y: axis }
			);
		}
		offsets.sort((left, right) => left.x ** 2 + left.y ** 2 - (right.x ** 2 + right.y ** 2));
		for (const offset of offsets) {
			const candidate = {
				x: origin.x + offset.x * STATE_PLACEMENT_STEP_REM,
				y: origin.y + offset.y * STATE_PLACEMENT_STEP_REM
			};
			if (placementIsFree(candidate, index, nodeWidth, nodeHeight)) {
				return candidate;
			}
		}
	}
	return origin;
};
