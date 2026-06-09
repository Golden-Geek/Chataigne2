import type { GraphEdge, GraphNode, GraphSocket } from 'golden_alchemist_ui';

export interface AuthoredGraphNode {
	id: string;
	typeId: string;
	label: string;
	x: number;
	y: number;
	width?: number;
	height?: number;
	inputs: GraphSocket[];
	outputs: GraphSocket[];
}

export interface AuthoredGraphEdge {
	id: string;
	from: {
		nodeId: string;
		socketId: string;
	};
	to: {
		nodeId: string;
		socketId: string;
	};
}

export interface AuthoredGraphDocument {
	version: 1;
	nodes: AuthoredGraphNode[];
	edges: AuthoredGraphEdge[];
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const finiteNumber = (value: unknown): number | undefined =>
	typeof value === 'number' && Number.isFinite(value) ? value : undefined;

const parseSocket = (value: unknown): GraphSocket | null => {
	if (!isRecord(value) || typeof value.id !== 'string' || typeof value.label !== 'string') {
		return null;
	}
	return {
		id: value.id,
		label: value.label,
		valueType: typeof value.valueType === 'string' ? value.valueType : undefined,
		compatible: typeof value.compatible === 'boolean' ? value.compatible : undefined,
		color: typeof value.color === 'string' ? value.color : undefined
	};
};

const parseSockets = (value: unknown): GraphSocket[] | null => {
	if (!Array.isArray(value)) {
		return null;
	}
	const sockets = value.map(parseSocket);
	return sockets.every((socket): socket is GraphSocket => socket !== null) ? sockets : null;
};

const parseNode = (value: unknown): AuthoredGraphNode | null => {
	if (
		!isRecord(value) ||
		typeof value.id !== 'string' ||
		typeof value.typeId !== 'string' ||
		typeof value.label !== 'string'
	) {
		return null;
	}
	const x = finiteNumber(value.x);
	const y = finiteNumber(value.y);
	const inputs = parseSockets(value.inputs);
	const outputs = parseSockets(value.outputs);
	if (x === undefined || y === undefined || inputs === null || outputs === null) {
		return null;
	}
	return {
		id: value.id,
		typeId: value.typeId,
		label: value.label,
		x,
		y,
		width: finiteNumber(value.width),
		height: finiteNumber(value.height),
		inputs,
		outputs
	};
};

const parseSocketRef = (
	value: unknown
): {
	nodeId: string;
	socketId: string;
} | null => {
	if (!isRecord(value) || typeof value.nodeId !== 'string' || typeof value.socketId !== 'string') {
		return null;
	}
	return {
		nodeId: value.nodeId,
		socketId: value.socketId
	};
};

const parseEdge = (value: unknown, index: number): AuthoredGraphEdge | null => {
	if (!isRecord(value)) {
		return null;
	}
	const from = parseSocketRef(value.from);
	const to = parseSocketRef(value.to);
	if (!from || !to) {
		return null;
	}
	return {
		id: typeof value.id === 'string' ? value.id : `edge-${index}`,
		from,
		to
	};
};

export const parseAuthoredGraph = (source: string): AuthoredGraphDocument | null => {
	try {
		const value: unknown = JSON.parse(source);
		if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.nodes)) {
			return null;
		}
		if (!Array.isArray(value.edges)) {
			return null;
		}
		const nodes = value.nodes.map(parseNode);
		const edges = value.edges.map(parseEdge);
		if (
			!nodes.every((node): node is AuthoredGraphNode => node !== null) ||
			!edges.every((edge): edge is AuthoredGraphEdge => edge !== null)
		) {
			return null;
		}
		return {
			version: 1,
			nodes,
			edges
		};
	} catch {
		return null;
	}
};

export const serializeAuthoredGraph = (document: AuthoredGraphDocument): string =>
	JSON.stringify(document, null, 2);

export const toGraphNodes = (document: AuthoredGraphDocument): GraphNode[] =>
	document.nodes.map((node) => ({
		id: node.id,
		label: node.label,
		subtitle: node.typeId,
		x: node.x,
		y: node.y,
		width: node.width,
		height: node.height,
		resizable: true,
		inputs: node.inputs,
		outputs: node.outputs
	}));

export const toGraphEdges = (document: AuthoredGraphDocument): GraphEdge[] =>
	document.edges.map((edge) => ({
		id: edge.id,
		from: edge.from,
		to: edge.to
	}));
