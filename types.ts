export interface GraphCamera {
	x: number;
	y: number;
	zoom: number;
}

export type GraphSocketDirection = 'input' | 'output';

export interface GraphSocket {
	id: string;
	label: string;
	valueType?: string;
	compatible?: boolean;
	color?: string;
	defaultParamId?: string;
	parentId?: string;
	component?: string;
	children?: GraphSocket[];
}

export interface GraphNodePosition {
	x: number;
	y: number;
}

export interface GraphNodeSize {
	width: number;
	height: number;
}

export interface GraphViewportInset {
	left?: number;
	top?: number;
	right?: number;
	bottom?: number;
}

export interface GraphWorldBounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

export interface GraphWorldContentContext {
	camera: GraphCamera;
	remPx: number;
	viewportWidth: number;
	viewportHeight: number;
}

export interface GraphNodeBypassConnection {
	inputSocketId: string;
	outputSocketId: string;
	color?: string;
}

export interface GraphNode {
	id: string;
	label: string;
	subtitle?: string;
	description?: string;
	canRename?: boolean;
	collapsed?: boolean;
	enabled?: boolean;
	canDisable?: boolean;
	color?: string;
	socketPlacement?: 'body' | 'header';
	position: GraphNodePosition;
	size?: GraphNodeSize;
	automaticSize?: GraphNodeSize;
	resizable?: boolean;
	inputs: GraphSocket[];
	outputs: GraphSocket[];
	headerInputs?: GraphSocket[];
	bypassConnections?: GraphNodeBypassConnection[];
	active?: boolean;
	invalid?: boolean;
	warning?: string;
}

export interface GraphSocketRef {
	nodeId: string;
	socketId: string;
}

export interface GraphEdge {
	id?: string;
	from: GraphSocketRef;
	to: GraphSocketRef;
	color?: string;
	targetColor?: string;
	active?: boolean;
	invalid?: boolean;
}

export interface GraphConnectionRequest {
	from: GraphSocketRef;
	to: GraphSocketRef;
}

export interface GraphNodeMove {
	nodeId: string;
	position: GraphNodePosition;
}

export type GraphNodeResize =
	| { nodeId: string; mode: 'custom'; size: GraphNodeSize }
	| { nodeId: string; mode: 'automatic' };

export interface GraphNodeCreationRequest {
	position: GraphNodePosition;
	clientX: number;
	clientY: number;
}
