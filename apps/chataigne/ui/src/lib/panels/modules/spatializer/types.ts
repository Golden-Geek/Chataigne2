import type { UiNodeDto } from 'golden_ui';
import type { GraphNodePosition } from 'golden_graph_ui';
export type EndpointKind = 'source' | 'target';
export type PositionKind = 'vec2' | 'vec3';
export type ValueLayout = 'sourceCentric' | 'targetCentric';
export type RadiusDragKind = 'radius' | 'freezeRadius';

export interface Point2 {
	x: number;
	y: number;
}

export interface SpatialEndpoint {
	key: string;
	kind: EndpointKind;
	node: UiNodeDto;
	valueDeclId: string;
	positionParam: UiNodeDto | null;
	radiusParam: UiNodeDto | null;
	freezeRadiusParam: UiNodeDto | null;
	positionKind: PositionKind;
	middle: number;
	x: number;
	y: number;
	radius: number | null;
	freezeRadius: number | null;
	color: string;
	enabled: boolean;
	positionWritable: boolean;
	radiusWritable: boolean;
	freezeRadiusWritable: boolean;
}

export interface VoronoiCell {
	key: string;
	color: string;
	points: Point2[];
}

export interface RelatedValue {
	key: string;
	endpoint: SpatialEndpoint;
	valueParam: UiNodeDto | null;
}

export interface DebugConnection {
	key: string;
	source: SpatialEndpoint;
	target: SpatialEndpoint;
	weight: number;
	role: 'current' | 'neighbor' | 'frozen' | 'direct';
}

export interface DebugVoronoiGuide {
	key: string;
	source: SpatialEndpoint;
	current: SpatialEndpoint;
	neighbor: SpatialEndpoint;
	edgeStart: Point2;
	edgeEnd: Point2;
	closestPoint: Point2;
	neighborMeasurePoint: Point2;
	edgeDistance: number;
	neighborDistance: number;
	weight: number;
}

export interface DebugVoronoiComputation {
	key: string;
	source: SpatialEndpoint;
	current: SpatialEndpoint | null;
	frozenTargets: SpatialEndpoint[];
	cellPoints: Point2[];
	guides: DebugVoronoiGuide[];
}

export interface DeadzoneHole {
	key: string;
	x: number;
	y: number;
	radius: number;
	color: string;
}

export interface SpatializerCanvas {
	clientToWorld(clientX: number, clientY: number): GraphNodePosition;
	frameSelection(): boolean;
	home(): boolean;
	focus(): void;
}
