<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import Slider from 'golden_ui/components/common/Slider.svelte';
	import {
		GraphCanvas,
		type GraphCamera,
		type GraphNodePosition,
		type GraphWorldBounds,
		type GraphWorldContentContext
	} from 'golden_graph_ui';
	import { GraphDocumentAdapter } from '../../graph/graphDocumentAdapter';
	import type {
		NodeId,
		PanelProps,
		PanelState,
		ParamValue,
		UiColorDto,
		UiCreatableUserItem,
		UiCreateUserItemInitialParam,
		UiNodeDto
	} from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import {
		createUiEditSession,
		sendCreateUserItemByTypeIntent,
		sendSetParamIntent
	} from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';

	type EditorParams = {
		moduleNodeId?: NodeId;
	};

	interface SpatializerEditorPersistedState {
		camera?: GraphCamera;
		debugView?: boolean;
		spatialUnitRem?: number;
	}

	type EndpointKind = 'source' | 'target';
	type PositionKind = 'vec2' | 'vec3';
	type ValueLayout = 'sourceCentric' | 'targetCentric';
	type RadiusDragKind = 'radius' | 'freezeRadius';
	type UiEditSession = ReturnType<typeof createUiEditSession>;

	interface Point2 {
		x: number;
		y: number;
	}

	interface SpatialEndpoint {
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

	interface VoronoiCell {
		key: string;
		color: string;
		points: Point2[];
	}

	interface RelatedValue {
		key: string;
		endpoint: SpatialEndpoint;
		valueParam: UiNodeDto | null;
	}

	interface DebugConnection {
		key: string;
		source: SpatialEndpoint;
		target: SpatialEndpoint;
		weight: number;
		role: 'current' | 'neighbor' | 'frozen' | 'direct';
	}

	interface DebugVoronoiGuide {
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

	interface DebugVoronoiComputation {
		key: string;
		source: SpatialEndpoint;
		current: SpatialEndpoint | null;
		frozenTargets: SpatialEndpoint[];
		cellPoints: Point2[];
		guides: DebugVoronoiGuide[];
	}

	interface DeadzoneHole {
		key: string;
		x: number;
		y: number;
		radius: number;
		color: string;
	}

	type DragGesture =
		| {
				kind: 'position';
				pointerId: number;
				endpoint: SpatialEndpoint;
				editSession: UiEditSession;
				updateTail: Promise<void>;
				sendFrame: number | null;
				sendInFlight: boolean;
				sendDirty: boolean;
				latestPosition: Point2;
				startPointer: Point2;
				startPosition: Point2;
		  }
		| {
				kind: RadiusDragKind;
				pointerId: number;
				endpoint: SpatialEndpoint;
				editSession: UiEditSession;
				updateTail: Promise<void>;
				sendFrame: number | null;
				sendInFlight: boolean;
				sendDirty: boolean;
				latestRadius: number;
				startRadius: number;
				startDistance: number;
				center: Point2;
		  };

	const SPATIALIZER_MODULE_TYPE = 'spatializer_module';
	const SOURCE_LIST_TYPE = 'spatializer_source_list';
	const TARGET_LIST_TYPE = 'spatializer_target_list';
	const SOURCE_TYPE = 'spatializer_source';
	const TARGET_TYPE = 'spatializer_target';
	const POSITION_2D_DECL_ID = 'position_2d';
	const POSITION_3D_DECL_ID = 'position_3d';
	const RADIUS_DECL_ID = 'radius';
	const FREEZE_RADIUS_DECL_ID = 'freeze_radius';
	const VALUE_TARGET_DECL_PREFIX = 'spatializer_target';
	const VALUE_SOURCE_DECL_PREFIX = 'spatializer_source';
	const VALUE_LAYOUT_DECL_ID = 'value_layout';
	const VALUE_LAYOUT_SOURCE_CENTRIC: ValueLayout = 'sourceCentric';
	const VALUE_LAYOUT_TARGET_CENTRIC: ValueLayout = 'targetCentric';
	const MODE_VORONOI = 'voronoi';
	const MODE_TARGET_RADIUS = 'targetRadius';
	const MODE_SOURCE_RADIUS = 'sourceRadius';
	const MODE_OVERLAP = 'overlap';
	const VORONOI_TIE_EPSILON = 1.0e-9;
	const VORONOI_EDGE_EPSILON = 1.0e-6;
	const DEBUG_WEIGHT_EPSILON = 0.000001;
	const HIGHLIGHT_FADE_MS = 220;
	const CAMERA_PERSIST_DELAY_MS = 150;
	const SPATIALIZER_UNIT_REM = 4;
	const SPATIALIZER_MIN_ZOOM = 0.05;
	const DEFAULT_SPATIAL_BOUNDS: GraphWorldBounds = { left: -5, top: -5, right: 5, bottom: 5 };
	const TARGET_ZONE_PADDING_UNITS = 1;
	const ENDPOINT_BOUNDS_PADDING_UNITS = 1;
	const MIN_INSPECTOR_WIDTH = 160;
	const MAX_INSPECTOR_WIDTH = 520;
	const DEFAULT_INSPECTOR_WIDTH = 272;
	const ENDPOINT_PALETTE = [
		'#48cae4',
		'#f72585',
		'#80ed99',
		'#ffd166',
		'#90be6d',
		'#f9844a',
		'#b8c0ff',
		'#43aa8b'
	];
	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	const persistedDebugView = (params: PanelState['params']): boolean =>
		readPanelPersistedState<SpatializerEditorPersistedState>(params).debugView === true;

	let panelRoot: HTMLElement | null = $state(null);
	let worldSvg: SVGSVGElement | null = $state(null);
	let graphCanvas: {
		clientToWorld: (clientX: number, clientY: number) => GraphNodePosition;
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
	} | null = $state(null);
	const graphDocument = new GraphDocumentAdapter().update([], []);

	let activeDrag = $state<DragGesture | null>(null);
	let positionPreviews = $state<Record<string, Point2>>({});
	let radiusPreviews = $state<Record<string, number>>({});
	let freezeRadiusPreviews = $state<Record<string, number>>({});
	let debugView = $state(false);
	let appliedPersistedDebugView = $state(false);
	let inspectorVisible = $state(true);
	let inspectorWidth = $state(DEFAULT_INSPECTOR_WIDTH);
	let pendingCamera: GraphCamera | null = null;
	let cameraPersistenceTimer: ReturnType<typeof setTimeout> | null = null;

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let panelParams = $derived((panelState.params ?? {}) as EditorParams);

	const finiteNumber = (value: unknown): value is number =>
		typeof value === 'number' && Number.isFinite(value);

	const graphCamera = (value: unknown): GraphCamera | undefined => {
		if (typeof value !== 'object' || value === null) {
			return undefined;
		}
		const candidate = value as Record<string, unknown>;
		if (!finiteNumber(candidate.x) || !finiteNumber(candidate.y) || !finiteNumber(candidate.zoom)) {
			return undefined;
		}
		return { x: candidate.x, y: candidate.y, zoom: candidate.zoom };
	};

	const camerasMatch = (left: GraphCamera | undefined, right: GraphCamera): boolean =>
		left !== undefined &&
		Math.abs(left.x - right.x) < 0.0001 &&
		Math.abs(left.y - right.y) < 0.0001 &&
		Math.abs(left.zoom - right.zoom) < 0.0001;

	const persistedCameraForScale = (params: PanelState['params']): GraphCamera | undefined => {
		const persisted = readPanelPersistedState<SpatializerEditorPersistedState>(params);
		return persisted.spatialUnitRem === SPATIALIZER_UNIT_REM
			? graphCamera(persisted.camera)
			: undefined;
	};

	let initialCamera = $derived.by(() => persistedCameraForScale(panelState.params));

	$effect(() => {
		const persisted = persistedDebugView(panelState.params);
		if (persisted !== appliedPersistedDebugView) {
			appliedPersistedDebugView = persisted;
			debugView = persisted;
		}
	});

	const declLast = (declId: string): string => {
		const slash = declId.lastIndexOf('/');
		return slash >= 0 ? declId.slice(slash + 1) : declId;
	};

	const simpleUuid = (uuid: string): string => uuid.replaceAll('-', '').toLowerCase();

	const valueDeclId = (prefix: string, uuid: string): string => `${prefix}_${simpleUuid(uuid)}`;

	const childByKey = (node: UiNodeDto | null, key: string): UiNodeDto | null => {
		if (!graph || !node) {
			return null;
		}
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (!child) {
				continue;
			}
			if (child.decl_id === key || declLast(child.decl_id) === key || child.meta.label === key) {
				return child;
			}
		}
		return null;
	};

	const childByType = (node: UiNodeDto | null, nodeType: string): UiNodeDto | null => {
		if (!graph || !node) {
			return null;
		}
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (child?.node_type === nodeType) {
				return child;
			}
		}
		return null;
	};

	const childNodes = (node: UiNodeDto | null): UiNodeDto[] => {
		if (!graph || !node) {
			return [];
		}
		const result: UiNodeDto[] = [];
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (child?.data.kind === 'node') {
				result.push(child);
			}
		}
		return result;
	};

	const parameterChild = (node: UiNodeDto, declId: string): UiNodeDto | null => {
		const child = childByKey(node, declId);
		return child?.data.kind === 'parameter' ? child : null;
	};

	const enumValue = (node: UiNodeDto | null): string =>
		node?.data.kind === 'parameter' && node.data.param.value.kind === 'enum'
			? node.data.param.value.value
			: '';

	const enumToken = (value: string): string =>
		value
			.trim()
			.toLowerCase()
			.replace(/[\s_-]/g, '');

	const valueLayoutFromEnum = (value: string): ValueLayout =>
		enumToken(value) === 'targetcentric'
			? VALUE_LAYOUT_TARGET_CENTRIC
			: VALUE_LAYOUT_SOURCE_CENTRIC;

	const enumLabel = (node: UiNodeDto | null): string => {
		if (node?.data.kind !== 'parameter' || node.data.param.value.kind !== 'enum') {
			return '';
		}
		const value = node.data.param.value.value;
		return (
			node.data.param.constraints.enum_options.find((option) => option.variant_id === value)
				?.label ?? value
		);
	};

	const floatValue = (node: UiNodeDto | null): number | null =>
		node?.data.kind === 'parameter' && node.data.param.value.kind === 'float'
			? node.data.param.value.value
			: null;

	const clampPositive = (value: number): number => Math.max(0, Number.isFinite(value) ? value : 0);

	const clamp01 = (value: number | null): number =>
		Math.min(1, Math.max(0, Number.isFinite(value ?? Number.NaN) ? (value ?? 0) : 0));

	const clampInspectorWidth = (value: number): number =>
		Math.max(MIN_INSPECTOR_WIDTH, Math.min(MAX_INSPECTOR_WIDTH, value));

	const metadataColor = (color: UiColorDto | null | undefined): string | null => {
		if (!color) {
			return null;
		}
		const red = Math.round(Math.min(1, Math.max(0, color.r)) * 255);
		const green = Math.round(Math.min(1, Math.max(0, color.g)) * 255);
		const blue = Math.round(Math.min(1, Math.max(0, color.b)) * 255);
		const alpha = Math.min(1, Math.max(0, color.a));
		return `rgb(${red} ${green} ${blue} / ${alpha})`;
	};

	const endpointColor = (node: UiNodeDto, index: number): string =>
		metadataColor(node.meta.presentation?.color ?? node.meta.presentation?.default_color) ??
		ENDPOINT_PALETTE[index % ENDPOINT_PALETTE.length];

	const positionFromParam = (
		positionParam: UiNodeDto | null
	): { kind: PositionKind; x: number; y: number; middle: number } => {
		if (positionParam?.data.kind === 'parameter') {
			const value = positionParam.data.param.value;
			if (value.kind === 'vec3') {
				return { kind: 'vec3', x: value.value[0], y: value.value[2], middle: value.value[1] };
			}
			if (value.kind === 'vec2') {
				return { kind: 'vec2', x: value.value[0], y: value.value[1], middle: 0 };
			}
		}
		return { kind: 'vec2', x: 0, y: 0, middle: 0 };
	};

	const writableParameter = (node: UiNodeDto | null): boolean =>
		node?.data.kind === 'parameter' && !node.data.param.read_only;

	const endpointValueDeclId = (node: UiNodeDto, kind: EndpointKind): string =>
		valueDeclId(kind === 'source' ? VALUE_SOURCE_DECL_PREFIX : VALUE_TARGET_DECL_PREFIX, node.uuid);

	const endpointFromNode = (
		node: UiNodeDto,
		kind: EndpointKind,
		index: number,
		prefer3d: boolean
	): SpatialEndpoint => {
		const preferredPosition = prefer3d ? POSITION_3D_DECL_ID : POSITION_2D_DECL_ID;
		const fallbackPosition = prefer3d ? POSITION_2D_DECL_ID : POSITION_3D_DECL_ID;
		const positionParam =
			parameterChild(node, preferredPosition) ?? parameterChild(node, fallbackPosition);
		const position = positionFromParam(positionParam);
		const radiusParam = parameterChild(node, RADIUS_DECL_ID);
		const radius = floatValue(radiusParam);
		const freezeRadiusParam = parameterChild(node, FREEZE_RADIUS_DECL_ID);
		const freezeRadius = floatValue(freezeRadiusParam);
		return {
			key: `${kind}:${node.node_id}`,
			kind,
			node,
			valueDeclId: endpointValueDeclId(node, kind),
			positionParam,
			radiusParam,
			freezeRadiusParam,
			positionKind: position.kind,
			middle: position.middle,
			x: position.x,
			y: position.y,
			radius: radius === null ? null : clampPositive(radius),
			freezeRadius: freezeRadius === null ? null : clampPositive(freezeRadius),
			color: endpointColor(node, index),
			enabled: node.meta.enabled,
			positionWritable: writableParameter(positionParam),
			radiusWritable: writableParameter(radiusParam),
			freezeRadiusWritable: writableParameter(freezeRadiusParam)
		};
	};

	let modules = $derived.by((): UiNodeDto[] => {
		if (!graph) {
			return [];
		}
		return [...graph.nodesById.values()]
			.filter((candidate) => candidate.node_type === SPATIALIZER_MODULE_TYPE)
			.sort((left, right) => left.meta.label.localeCompare(right.meta.label));
	});

	let activeModule = $derived.by((): UiNodeDto | null => {
		if (modules.length === 0) {
			return null;
		}
		return modules.find((module) => module.node_id === panelParams.moduleNodeId) ?? modules[0];
	});

	let parametersFolder = $derived(childByKey(activeModule, 'parameters'));
	let valuesFolder = $derived(childByKey(activeModule, 'values'));
	let dimensionsParam = $derived(childByKey(parametersFolder, 'dimensions'));
	let modeParam = $derived(childByKey(parametersFolder, 'mode'));
	let valueLayoutParam = $derived(childByKey(parametersFolder, VALUE_LAYOUT_DECL_ID));
	let sourceList = $derived(childByType(parametersFolder, SOURCE_LIST_TYPE));
	let targetList = $derived(childByType(parametersFolder, TARGET_LIST_TYPE));
	let dimensionMode = $derived(enumValue(dimensionsParam) || '2d');
	let spatializerMode = $derived(enumValue(modeParam) || MODE_VORONOI);
	let valueLayout = $derived.by((): ValueLayout =>
		valueLayoutFromEnum(enumValue(valueLayoutParam))
	);
	let prefer3d = $derived(dimensionMode === '3d');

	let rawEndpoints = $derived.by((): SpatialEndpoint[] => {
		const sources = childNodes(sourceList)
			.filter((node) => node.node_type === SOURCE_TYPE)
			.map((node, index) => endpointFromNode(node, 'source', index, prefer3d));
		const targets = childNodes(targetList)
			.filter((node) => node.node_type === TARGET_TYPE)
			.map((node, index) => endpointFromNode(node, 'target', index, prefer3d));
		return [...targets, ...sources];
	});

	let endpoints = $derived(
		rawEndpoints.map((endpoint) => ({
			...endpoint,
			...(positionPreviews[endpoint.key] ?? {}),
			radius: radiusPreviews[endpoint.key] ?? endpoint.radius,
			freezeRadius: freezeRadiusPreviews[endpoint.key] ?? endpoint.freezeRadius
		}))
	);

	let targets = $derived(endpoints.filter((endpoint) => endpoint.kind === 'target'));
	let sources = $derived(endpoints.filter((endpoint) => endpoint.kind === 'source'));
	let enabledTargets = $derived(targets.filter((endpoint) => endpoint.enabled));

	let selectedEndpoint = $derived.by((): SpatialEndpoint | null => {
		const selectedNodeId = session?.selectedNodeId;
		if (selectedNodeId === null || selectedNodeId === undefined) {
			return null;
		}
		return endpoints.find((endpoint) => endpoint.node.node_id === selectedNodeId) ?? null;
	});

	let dimensionLabel = $derived(prefer3d ? 'XZ' : 'XY');
	let modeLabel = $derived(enumLabel(modeParam) || 'Voronoi');

	const valueParameterInFolder = (folderDeclId: string, valueDeclId: string): UiNodeDto | null => {
		const folder = childByKey(valuesFolder, folderDeclId);
		if (!folder) {
			return null;
		}
		const valueNode = childByKey(folder, valueDeclId);
		return valueNode?.data.kind === 'parameter' ? valueNode : null;
	};

	const valueParameterForPair = (
		target: SpatialEndpoint,
		source: SpatialEndpoint
	): UiNodeDto | null => {
		const targetCentricValue = (): UiNodeDto | null =>
			valueParameterInFolder(target.valueDeclId, source.valueDeclId);
		const sourceCentricValue = (): UiNodeDto | null =>
			valueParameterInFolder(source.valueDeclId, target.valueDeclId);
		return valueLayout === VALUE_LAYOUT_TARGET_CENTRIC
			? (targetCentricValue() ?? sourceCentricValue())
			: (sourceCentricValue() ?? targetCentricValue());
	};

	let relatedValues = $derived.by((): RelatedValue[] => {
		if (!selectedEndpoint) {
			return [];
		}
		if (selectedEndpoint.kind === 'source') {
			return targets.map((target) => ({
				key: `${selectedEndpoint.key}:${target.key}`,
				endpoint: target,
				valueParam: valueParameterForPair(target, selectedEndpoint)
			}));
		}
		return sources.map((source) => ({
			key: `${selectedEndpoint.key}:${source.key}`,
			endpoint: source,
			valueParam: valueParameterForPair(selectedEndpoint, source)
		}));
	});

	const endpointDistance = (left: SpatialEndpoint, right: SpatialEndpoint): number =>
		Math.hypot(right.x - left.x, right.y - left.y);

	const pointBetween = (start: Point2, end: Point2, fraction: number): Point2 => ({
		x: start.x + (end.x - start.x) * fraction,
		y: start.y + (end.y - start.y) * fraction
	});

	const nearestTargetTo = (endpoint: SpatialEndpoint): SpatialEndpoint | null => {
		let best: SpatialEndpoint | null = null;
		let bestDistance = Infinity;
		for (const target of enabledTargets) {
			const distance = endpointDistance(endpoint, target);
			if (distance < bestDistance) {
				best = target;
				bestDistance = distance;
			}
		}
		return best;
	};

	let spatialWorldBounds = $derived.by((): GraphWorldBounds => {
		let left = DEFAULT_SPATIAL_BOUNDS.left;
		let top = DEFAULT_SPATIAL_BOUNDS.top;
		let right = DEFAULT_SPATIAL_BOUNDS.right;
		let bottom = DEFAULT_SPATIAL_BOUNDS.bottom;
		for (const endpoint of endpoints) {
			const radius = Math.max(endpoint.radius ?? 0, endpoint.freezeRadius ?? 0);
			left = Math.min(left, endpoint.x - radius);
			top = Math.min(top, endpoint.y - radius);
			right = Math.max(right, endpoint.x + radius);
			bottom = Math.max(bottom, endpoint.y + radius);
		}
		return {
			left: left - ENDPOINT_BOUNDS_PADDING_UNITS,
			top: top - ENDPOINT_BOUNDS_PADDING_UNITS,
			right: right + ENDPOINT_BOUNDS_PADDING_UNITS,
			bottom: bottom + ENDPOINT_BOUNDS_PADDING_UNITS
		};
	});

	const spatialBoundsToGraphBounds = (bounds: GraphWorldBounds): GraphWorldBounds => ({
		left: bounds.left * SPATIALIZER_UNIT_REM,
		top: bounds.top * SPATIALIZER_UNIT_REM,
		right: bounds.right * SPATIALIZER_UNIT_REM,
		bottom: bounds.bottom * SPATIALIZER_UNIT_REM
	});

	let graphWorldBounds = $derived(spatialBoundsToGraphBounds(spatialWorldBounds));

	const rectanglePolygon = (bounds: GraphWorldBounds): Point2[] => [
		{ x: bounds.left, y: bounds.top },
		{ x: bounds.right, y: bounds.top },
		{ x: bounds.right, y: bounds.bottom },
		{ x: bounds.left, y: bounds.bottom }
	];

	const closerHalfPlaneValue = (point: Point2, site: Point2, other: Point2): number => {
		const dx = other.x - site.x;
		const dy = other.y - site.y;
		return (
			2 * (dx * point.x + dy * point.y) -
			(other.x * other.x + other.y * other.y - site.x * site.x - site.y * site.y)
		);
	};

	const clipVoronoiCell = (polygon: Point2[], target: Point2, other: Point2): Point2[] => {
		const dx = other.x - target.x;
		const dy = other.y - target.y;
		if (Math.hypot(dx, dy) < VORONOI_EDGE_EPSILON) {
			return polygon;
		}
		const valueAt = (point: Point2): number => closerHalfPlaneValue(point, target, other);
		const inside = (point: Point2): boolean => valueAt(point) <= VORONOI_TIE_EPSILON;
		const intersect = (start: Point2, end: Point2): Point2 => {
			const startValue = valueAt(start);
			const endValue = valueAt(end);
			const denominator = startValue - endValue;
			if (Math.abs(denominator) <= VORONOI_TIE_EPSILON) {
				return end;
			}
			const t = Math.min(1, Math.max(0, startValue / denominator));
			return {
				x: start.x + (end.x - start.x) * t,
				y: start.y + (end.y - start.y) * t
			};
		};

		const result: Point2[] = [];
		for (let index = 0; index < polygon.length; index += 1) {
			const current = polygon[index];
			const previous = polygon[(index + polygon.length - 1) % polygon.length];
			const currentInside = inside(current);
			const previousInside = inside(previous);
			if (currentInside && !previousInside) {
				result.push(intersect(previous, current));
			}
			if (currentInside) {
				result.push(current);
			} else if (previousInside) {
				result.push(intersect(previous, current));
			}
		}
		return result;
	};

	const voronoiBoundsForSource = (source: SpatialEndpoint): GraphWorldBounds => {
		let left = source.x;
		let right = source.x;
		let top = source.y;
		let bottom = source.y;
		for (const target of enabledTargets) {
			left = Math.min(left, target.x);
			right = Math.max(right, target.x);
			top = Math.min(top, target.y);
			bottom = Math.max(bottom, target.y);
		}
		const padding = Math.max(Math.abs(right - left), Math.abs(bottom - top), 1) * 4;
		return {
			left: left - padding,
			top: top - padding,
			right: right + padding,
			bottom: bottom + padding
		};
	};

	const voronoiCellPolygon = (target: SpatialEndpoint, bounds: GraphWorldBounds): Point2[] => {
		let polygon = rectanglePolygon(bounds);
		for (const other of enabledTargets) {
			if (other.key === target.key) {
				continue;
			}
			polygon = clipVoronoiCell(polygon, target, other);
			if (polygon.length < 3) {
				break;
			}
		}
		return polygon;
	};

	const distancesTie = (left: number, right: number): boolean =>
		Number.isFinite(left) &&
		Number.isFinite(right) &&
		Math.abs(left - right) <= VORONOI_TIE_EPSILON;

	const targetWeightForSource = (target: SpatialEndpoint, source: SpatialEndpoint): number =>
		relatedValueAmount(valueParameterForPair(target, source));

	const sourceWeightTotal = (source: SpatialEndpoint): number =>
		targets.reduce((total, target) => total + targetWeightForSource(target, source), 0);

	const activeRadiusForTarget = (source: SpatialEndpoint, target: SpatialEndpoint): number => {
		if (spatializerMode === MODE_TARGET_RADIUS) {
			return Math.max(0, target.radius ?? 0);
		}
		if (spatializerMode === MODE_SOURCE_RADIUS) {
			return Math.max(0, source.radius ?? 0);
		}
		if (spatializerMode === MODE_OVERLAP) {
			return Math.max(0, source.radius ?? 0) + Math.max(0, target.radius ?? 0);
		}
		return 0;
	};

	const deadzoneHolesForSource = (source: SpatialEndpoint): DeadzoneHole[] =>
		enabledTargets.flatMap((target) => {
			const radius = activeRadiusForTarget(source, target);
			return radius > 0
				? [{ key: target.key, x: target.x, y: target.y, radius, color: target.color }]
				: [];
		});

	const selectedDeadzoneSource = (): SpatialEndpoint | null =>
		selectedEndpoint?.kind === 'source' &&
		selectedEndpoint.enabled &&
		spatializerMode !== MODE_VORONOI &&
		sourceWeightTotal(selectedEndpoint) <= DEBUG_WEIGHT_EPSILON
			? selectedEndpoint
			: null;

	const selectedVoronoiDeadzoneSource = (): SpatialEndpoint | null =>
		selectedEndpoint?.kind === 'source' &&
		selectedEndpoint.enabled &&
		spatializerMode === MODE_VORONOI &&
		frozenTargetsForSource(selectedEndpoint).length > 0
			? selectedEndpoint
			: null;

	const highlightedVoronoiCell = (cell: VoronoiCell): boolean => {
		if (!selectedEndpoint || spatializerMode !== MODE_VORONOI) {
			return false;
		}
		if (selectedEndpoint.kind === 'target') {
			return cell.key === selectedEndpoint.key;
		}
		return nearestTargetTo(selectedEndpoint)?.key === cell.key;
	};

	const frozenTargetsForSource = (source: SpatialEndpoint): SpatialEndpoint[] => {
		const frozenDistances = enabledTargets.flatMap((target) => {
			const freezeRadius = Math.max(0, target.freezeRadius ?? 0);
			const distance = endpointDistance(source, target);
			return distance <= freezeRadius + VORONOI_TIE_EPSILON ? [distance] : [];
		});
		const frozenDistance = frozenDistances.reduce(
			(best, distance) => Math.min(best, distance),
			Infinity
		);
		if (!Number.isFinite(frozenDistance)) {
			return [];
		}
		return enabledTargets.filter((target) =>
			distancesTie(endpointDistance(source, target), frozenDistance)
		);
	};

	const closestPointOnSegment = (point: Point2, start: Point2, end: Point2): Point2 => {
		const dx = end.x - start.x;
		const dy = end.y - start.y;
		const lengthSquared = dx * dx + dy * dy;
		if (lengthSquared <= VORONOI_TIE_EPSILON) {
			return start;
		}
		const t = Math.min(
			1,
			Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared)
		);
		return { x: start.x + dx * t, y: start.y + dy * t };
	};

	const closestCellEdgeForNeighbor = (
		source: SpatialEndpoint,
		current: SpatialEndpoint,
		neighbor: SpatialEndpoint,
		cell: Point2[]
	): Omit<DebugVoronoiGuide, 'key' | 'source' | 'current' | 'neighbor' | 'weight'> | null => {
		let best: {
			edgeStart: Point2;
			edgeEnd: Point2;
			closestPoint: Point2;
			neighborMeasurePoint: Point2;
			edgeDistance: number;
			neighborDistance: number;
		} | null = null;
		for (let edgeIndex = 0; edgeIndex < cell.length; edgeIndex += 1) {
			const edgeStart = cell[edgeIndex];
			const edgeEnd = cell[(edgeIndex + 1) % cell.length];
			const startValue = Math.abs(closerHalfPlaneValue(edgeStart, current, neighbor));
			const endValue = Math.abs(closerHalfPlaneValue(edgeEnd, current, neighbor));
			if (startValue > VORONOI_EDGE_EPSILON || endValue > VORONOI_EDGE_EPSILON) {
				continue;
			}
			const closestPoint = closestPointOnSegment(source, edgeStart, edgeEnd);
			const edgeDistance = Math.hypot(closestPoint.x - source.x, closestPoint.y - source.y);
			if (best && edgeDistance >= best.edgeDistance) {
				continue;
			}
			const rawNeighborDistance = Math.hypot(
				closestPoint.x - neighbor.x,
				closestPoint.y - neighbor.y
			);
			const neighborDistance = Math.max(
				0,
				rawNeighborDistance - Math.max(0, neighbor.freezeRadius ?? 0)
			);
			const neighborFraction =
				rawNeighborDistance <= VORONOI_TIE_EPSILON ? 0 : neighborDistance / rawNeighborDistance;
			const neighborMeasurePoint = pointBetween(closestPoint, neighbor, neighborFraction);
			best = {
				edgeStart,
				edgeEnd,
				closestPoint,
				neighborMeasurePoint,
				edgeDistance,
				neighborDistance
			};
		}
		return best;
	};

	const voronoiViewBounds = (world: GraphWorldContentContext): GraphWorldBounds => {
		const zoom = Math.max(0.000001, world.camera.zoom);
		const left = -world.camera.x / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const top = -world.camera.y / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const right =
			(world.viewportWidth - world.camera.x) / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const bottom =
			(world.viewportHeight - world.camera.y) / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		return {
			left: left - TARGET_ZONE_PADDING_UNITS,
			top: top - TARGET_ZONE_PADDING_UNITS,
			right: right + TARGET_ZONE_PADDING_UNITS,
			bottom: bottom + TARGET_ZONE_PADDING_UNITS
		};
	};

	const deadzonePath = (
		bounds: GraphWorldBounds,
		holes: DeadzoneHole[],
		world: GraphWorldContentContext
	): string => {
		const left = pointPx(bounds.left, world);
		const top = pointPx(bounds.top, world);
		const right = pointPx(bounds.right, world);
		const bottom = pointPx(bounds.bottom, world);
		const rect = `M ${left} ${top} H ${right} V ${bottom} H ${left} Z`;
		const circles = holes
			.map((hole) => {
				const radius = pointPx(Math.max(0, hole.radius), world);
				const x = pointPx(hole.x, world);
				const y = pointPx(hole.y, world);
				return radius > 0
					? `M ${x - radius} ${y} a ${radius} ${radius} 0 1 0 ${radius * 2} 0 a ${radius} ${radius} 0 1 0 ${-radius * 2} 0`
					: '';
			})
			.filter(Boolean)
			.join(' ');
		return `${rect} ${circles}`;
	};

	const voronoiCellsForBounds = (bounds: GraphWorldBounds): VoronoiCell[] => {
		if (spatializerMode !== MODE_VORONOI || enabledTargets.length === 0) {
			return [];
		}
		return enabledTargets.flatMap((target) => {
			let polygon = rectanglePolygon(bounds);
			for (const other of enabledTargets) {
				if (other.key === target.key) {
					continue;
				}
				polygon = clipVoronoiCell(polygon, target, other);
				if (polygon.length === 0) {
					break;
				}
			}
			return polygon.length > 0 ? [{ key: target.key, color: target.color, points: polygon }] : [];
		});
	};

	const debugSources = (): SpatialEndpoint[] => {
		if (!debugView || spatializerMode !== MODE_VORONOI || !selectedEndpoint) {
			return [];
		}
		if (selectedEndpoint.kind === 'source') {
			return selectedEndpoint.enabled ? [selectedEndpoint] : [];
		}
		return sources.filter((source) => source.enabled);
	};

	const debugVoronoiComputationForSource = (
		source: SpatialEndpoint
	): DebugVoronoiComputation | null => {
		const frozenTargets = frozenTargetsForSource(source);
		const current = nearestTargetTo(source);
		if (!current) {
			return null;
		}
		const bounds = voronoiBoundsForSource(source);
		const cellPoints = voronoiCellPolygon(current, bounds);
		const guides =
			cellPoints.length < 3
				? []
				: enabledTargets.flatMap((neighbor) => {
						if (neighbor.key === current.key) {
							return [];
						}
						const edge = closestCellEdgeForNeighbor(source, current, neighbor, cellPoints);
						if (!edge) {
							return [];
						}
						return [
							{
								key: `${source.key}:${current.key}:${neighbor.key}`,
								source,
								current,
								neighbor,
								weight: targetWeightForSource(neighbor, source),
								...edge
							}
						];
					});
		return {
			key: source.key,
			source,
			current,
			frozenTargets,
			cellPoints,
			guides
		};
	};

	const computationIncludesTarget = (
		computation: DebugVoronoiComputation,
		target: SpatialEndpoint
	): boolean =>
		computation.current?.key === target.key ||
		computation.frozenTargets.some((frozenTarget) => frozenTarget.key === target.key) ||
		computation.guides.some((guide) => guide.neighbor.key === target.key);

	let debugVoronoiComputations = $derived.by((): DebugVoronoiComputation[] =>
		debugSources().flatMap((source) => {
			const computation = debugVoronoiComputationForSource(source);
			if (!computation) {
				return [];
			}
			if (
				selectedEndpoint?.kind === 'target' &&
				!computationIncludesTarget(computation, selectedEndpoint)
			) {
				return [];
			}
			return [computation];
		})
	);

	let debugVoronoiGuides = $derived.by((): DebugVoronoiGuide[] =>
		debugVoronoiComputations.flatMap((computation) => computation.guides)
	);

	let debugConnections = $derived.by((): DebugConnection[] => {
		if (!debugView || !selectedEndpoint) {
			return [];
		}
		if (spatializerMode === MODE_VORONOI) {
			return debugVoronoiComputations.flatMap((computation): DebugConnection[] => {
				if (computation.frozenTargets.length > 0) {
					return computation.frozenTargets.map((target): DebugConnection => ({
						key: `${computation.source.key}:${target.key}:frozen`,
						source: computation.source,
						target,
						weight: targetWeightForSource(target, computation.source),
						role: 'frozen' as const
					}));
				}
				const currentConnections: DebugConnection[] = computation.current
					? [
							{
								key: `${computation.source.key}:${computation.current.key}:current`,
								source: computation.source,
								target: computation.current,
								weight: targetWeightForSource(computation.current, computation.source),
								role: 'current' as const
							}
						]
					: [];
				const neighborConnections: DebugConnection[] = computation.guides.flatMap(
					(guide): DebugConnection[] => [
						{
							key: `${guide.source.key}:${guide.neighbor.key}:neighbor`,
							source: guide.source,
							target: guide.neighbor,
							weight: guide.weight,
							role: 'neighbor' as const
						}
					]
				);
				return [...currentConnections, ...neighborConnections];
			});
		}
		if (selectedEndpoint.kind === 'source') {
			return relatedValues.map((item) => ({
				key: item.key,
				source: selectedEndpoint,
				target: item.endpoint,
				weight: relatedValueAmount(item.valueParam),
				role: 'direct' as const
			}));
		}
		return relatedValues.map((item) => ({
			key: item.key,
			source: item.endpoint,
			target: selectedEndpoint,
			weight: relatedValueAmount(item.valueParam),
			role: 'direct' as const
		}));
	});

	$effect(() => {
		const rawByKey = new Map(rawEndpoints.map((endpoint) => [endpoint.key, endpoint]));
		let nextPositions = positionPreviews;
		let positionChanged = false;
		for (const [key, preview] of Object.entries(positionPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && Math.abs(raw.x - preview.x) < 0.0001 && Math.abs(raw.y - preview.y) < 0.0001) {
				if (!positionChanged) {
					nextPositions = { ...nextPositions };
					positionChanged = true;
				}
				delete nextPositions[key];
			}
		}
		if (positionChanged) {
			positionPreviews = nextPositions;
		}

		let nextRadii = radiusPreviews;
		let radiusChanged = false;
		for (const [key, preview] of Object.entries(radiusPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && raw.radius !== null && Math.abs(raw.radius - preview) < 0.0001) {
				if (!radiusChanged) {
					nextRadii = { ...nextRadii };
					radiusChanged = true;
				}
				delete nextRadii[key];
			}
		}
		if (radiusChanged) {
			radiusPreviews = nextRadii;
		}

		let nextFreezeRadii = freezeRadiusPreviews;
		let freezeRadiusChanged = false;
		for (const [key, preview] of Object.entries(freezeRadiusPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && raw.freezeRadius !== null && Math.abs(raw.freezeRadius - preview) < 0.0001) {
				if (!freezeRadiusChanged) {
					nextFreezeRadii = { ...nextFreezeRadii };
					freezeRadiusChanged = true;
				}
				delete nextFreezeRadii[key];
			}
		}
		if (freezeRadiusChanged) {
			freezeRadiusPreviews = nextFreezeRadii;
		}
	});

	$effect(() => {
		const nextTitle = activeModule
			? `Spatializer: ${activeModule.meta.label}`
			: 'Spatializer Editor';
		if (panelState.title !== nextTitle) {
			props.panelApi.setTitle(nextTitle);
		}
	});

	const panelOwnsFocus = (): boolean =>
		panelRoot !== null &&
		document.activeElement !== null &&
		panelRoot.contains(document.activeElement);

	const flushCameraPersistence = (): void => {
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
			cameraPersistenceTimer = null;
		}
		const nextCamera = pendingCamera;
		pendingCamera = null;
		if (!nextCamera) {
			return;
		}
		const currentCamera = persistedCameraForScale(props.panelApi.getParams());
		if (camerasMatch(currentCamera, nextCamera)) {
			return;
		}
		writePanelPersistedState(props.panelApi, {
			camera: nextCamera,
			spatialUnitRem: SPATIALIZER_UNIT_REM
		});
	};

	const persistCamera = (camera: GraphCamera): void => {
		pendingCamera = { ...camera };
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
		}
		cameraPersistenceTimer = setTimeout(flushCameraPersistence, CAMERA_PERSIST_DELAY_MS);
	};

	const setDebugView = (event: Event): void => {
		const enabled = (event.currentTarget as HTMLInputElement).checked;
		debugView = enabled;
		appliedPersistedDebugView = enabled;
		writePanelPersistedState(props.panelApi, { debugView: enabled });
	};

	const selectModule = (event: Event): void => {
		const value = Number((event.currentTarget as HTMLSelectElement).value);
		if (!Number.isSafeInteger(value)) {
			return;
		}
		props.panelApi.updateParams({ ...panelParams, moduleNodeId: value });
		session?.selectNode(value, 'REPLACE');
	};

	const selectEndpoint = (endpoint: SpatialEndpoint): void => {
		session?.selectNode(endpoint.node.node_id, 'REPLACE');
	};

	const startInspectorResize = (event: PointerEvent): void => {
		event.preventDefault();
		event.stopPropagation();
		const startX = event.clientX;
		const startWidth = inspectorWidth;
		const handle = event.currentTarget as HTMLElement;
		handle.setPointerCapture(event.pointerId);
		handle.onpointermove = (moveEvent) => {
			inspectorWidth = clampInspectorWidth(startWidth + moveEvent.clientX - startX);
		};
		handle.onpointerup = handle.onpointercancel = (endEvent) => {
			if (handle.hasPointerCapture(endEvent.pointerId)) {
				handle.releasePointerCapture(endEvent.pointerId);
			}
			handle.onpointermove = null;
			handle.onpointerup = null;
			handle.onpointercancel = null;
		};
	};

	const initialParam = (decl_id: string, value: ParamValue): UiCreateUserItemInitialParam => ({
		decl_id,
		value
	});

	const graphPositionToSpatialPosition = (position: GraphNodePosition): Point2 => ({
		x: position.x / SPATIALIZER_UNIT_REM,
		y: position.y / SPATIALIZER_UNIT_REM
	});

	const targetPositionValue = (position: Point2): ParamValue =>
		prefer3d
			? { kind: 'vec3', value: [position.x, 0, position.y] }
			: { kind: 'vec2', value: [position.x, position.y] };

	const targetCreationItem = (): UiCreatableUserItem | null =>
		targetList?.creatable_user_items.find((item) => item.node_type === TARGET_TYPE) ?? null;

	const createTargetAt = async (event: MouseEvent, position: GraphNodePosition): Promise<void> => {
		const item = targetCreationItem();
		if (!targetList || !item) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		graphCanvas?.focus();
		const spatialPosition = graphPositionToSpatialPosition(position);
		const result = await sendCreateUserItemByTypeIntent(
			targetList.node_id,
			item.node_type,
			item.label,
			{
				select_when_created: item.select_when_created,
				initial_params: [
					...(item.initial_params ?? []),
					initialParam(
						prefer3d ? POSITION_3D_DECL_ID : POSITION_2D_DECL_ID,
						targetPositionValue(spatialPosition)
					)
				]
			}
		);
		if (result.selectWhenCreated && result.createdNodeId !== null) {
			session?.selectNode(result.createdNodeId, 'REPLACE');
		}
	};

	const clearPositionPreview = (key: string): void => {
		const next = { ...positionPreviews };
		delete next[key];
		positionPreviews = next;
	};

	const clearRadiusPreview = (key: string): void => {
		const next = { ...radiusPreviews };
		delete next[key];
		radiusPreviews = next;
	};

	const clearFreezeRadiusPreview = (key: string): void => {
		const next = { ...freezeRadiusPreviews };
		delete next[key];
		freezeRadiusPreviews = next;
	};

	const setEndpointPosition = (endpoint: SpatialEndpoint, position: Point2): Promise<boolean> => {
		if (
			endpoint.positionParam?.data.kind !== 'parameter' ||
			endpoint.positionParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const value: ParamValue =
			endpoint.positionKind === 'vec3'
				? { kind: 'vec3', value: [position.x, endpoint.middle, position.y] }
				: { kind: 'vec2', value: [position.x, position.y] };
		positionPreviews = { ...positionPreviews, [endpoint.key]: position };
		return sendSetParamIntent(
			endpoint.positionParam.node_id,
			value,
			endpoint.positionParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearPositionPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearPositionPreview(endpoint.key);
				return false;
			});
	};

	const setEndpointRadius = (endpoint: SpatialEndpoint, radius: number): Promise<boolean> => {
		if (
			endpoint.radiusParam?.data.kind !== 'parameter' ||
			endpoint.radiusParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const nextRadius = clampPositive(radius);
		radiusPreviews = { ...radiusPreviews, [endpoint.key]: nextRadius };
		return sendSetParamIntent(
			endpoint.radiusParam.node_id,
			{ kind: 'float', value: nextRadius },
			endpoint.radiusParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearRadiusPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearRadiusPreview(endpoint.key);
				return false;
			});
	};

	const setEndpointFreezeRadius = (endpoint: SpatialEndpoint, radius: number): Promise<boolean> => {
		if (
			endpoint.freezeRadiusParam?.data.kind !== 'parameter' ||
			endpoint.freezeRadiusParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const nextRadius = clampPositive(radius);
		freezeRadiusPreviews = { ...freezeRadiusPreviews, [endpoint.key]: nextRadius };
		return sendSetParamIntent(
			endpoint.freezeRadiusParam.node_id,
			{ kind: 'float', value: nextRadius },
			endpoint.freezeRadiusParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearFreezeRadiusPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearFreezeRadiusPreview(endpoint.key);
				return false;
			});
	};

	const pointerWorld = (event: PointerEvent): Point2 =>
		graphPositionToSpatialPosition(
			graphCanvas?.clientToWorld(event.clientX, event.clientY) ?? { x: 0, y: 0 }
		);

	const dragEditSession = (
		endpoint: SpatialEndpoint,
		kind: 'position' | RadiusDragKind
	): UiEditSession => {
		const verb =
			kind === 'position'
				? 'Move'
				: kind === 'radius'
					? 'Edit radius for'
					: 'Edit freeze radius for';
		return createUiEditSession(`${verb} ${endpoint.node.meta.label}`, 'spatializer-drag');
	};

	const sendDragLatest = (drag: DragGesture): Promise<boolean> => {
		if (drag.kind === 'position') {
			return setEndpointPosition(drag.endpoint, drag.latestPosition);
		}
		if (drag.kind === 'radius') {
			return setEndpointRadius(drag.endpoint, drag.latestRadius);
		}
		return setEndpointFreezeRadius(drag.endpoint, drag.latestRadius);
	};

	const flushDragUpdate = (drag: DragGesture): void => {
		if (drag.sendInFlight || !drag.sendDirty) {
			return;
		}
		drag.sendDirty = false;
		drag.sendInFlight = true;
		drag.updateTail = drag.updateTail
			.then(() => sendDragLatest(drag))
			.then(() => undefined)
			.catch(() => undefined)
			.finally(() => {
				drag.sendInFlight = false;
				if (drag.sendDirty && activeDrag === drag) {
					scheduleDragUpdate(drag);
				}
			});
	};

	const scheduleDragUpdate = (drag: DragGesture): void => {
		drag.sendDirty = true;
		if (drag.sendFrame !== null || drag.sendInFlight) {
			return;
		}
		drag.sendFrame = requestAnimationFrame(() => {
			drag.sendFrame = null;
			flushDragUpdate(drag);
		});
	};

	const drainDragUpdates = async (drag: DragGesture): Promise<void> => {
		if (drag.sendFrame !== null) {
			cancelAnimationFrame(drag.sendFrame);
			drag.sendFrame = null;
		}
		await drag.updateTail;
		if (drag.sendDirty) {
			drag.sendDirty = false;
			await sendDragLatest(drag);
		}
	};

	const startEndpointDrag = (
		event: PointerEvent,
		endpoint: SpatialEndpoint,
		requestedKind: 'position' | RadiusDragKind
	): void => {
		if (event.button !== 0) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		graphCanvas?.focus();
		selectEndpoint(endpoint);
		const pointer = pointerWorld(event);
		const altRadiusKind: RadiusDragKind | null =
			endpoint.radiusWritable && endpoint.radius !== null
				? 'radius'
				: endpoint.freezeRadiusWritable && endpoint.freezeRadius !== null
					? 'freezeRadius'
					: null;
		const radiusKind: RadiusDragKind | null =
			requestedKind === 'position' ? (event.altKey ? altRadiusKind : null) : requestedKind;
		const radius =
			radiusKind === 'radius'
				? endpoint.radius
				: radiusKind === 'freezeRadius'
					? endpoint.freezeRadius
					: null;
		const radiusWritable =
			radiusKind === 'radius'
				? endpoint.radiusWritable
				: radiusKind === 'freezeRadius'
					? endpoint.freezeRadiusWritable
					: false;
		if (radiusKind && radiusWritable && radius !== null) {
			const editSession = dragEditSession(endpoint, radiusKind);
			const updateTail = editSession.begin();
			worldSvg?.setPointerCapture(event.pointerId);
			activeDrag = {
				kind: radiusKind,
				pointerId: event.pointerId,
				endpoint,
				editSession,
				updateTail,
				sendFrame: null,
				sendInFlight: false,
				sendDirty: false,
				latestRadius: radius,
				startRadius: radius,
				startDistance: Math.hypot(pointer.x - endpoint.x, pointer.y - endpoint.y),
				center: { x: endpoint.x, y: endpoint.y }
			};
			return;
		}
		if (!endpoint.positionWritable) {
			return;
		}
		const editSession = dragEditSession(endpoint, 'position');
		const updateTail = editSession.begin();
		worldSvg?.setPointerCapture(event.pointerId);
		activeDrag = {
			kind: 'position',
			pointerId: event.pointerId,
			endpoint,
			editSession,
			updateTail,
			sendFrame: null,
			sendInFlight: false,
			sendDirty: false,
			latestPosition: { x: endpoint.x, y: endpoint.y },
			startPointer: pointer,
			startPosition: { x: endpoint.x, y: endpoint.y }
		};
	};

	const selectEndpointWithKeyboard = (event: KeyboardEvent, endpoint: SpatialEndpoint): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectEndpoint(endpoint);
	};

	const handleWorldPointerMove = (event: PointerEvent): void => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		event.preventDefault();
		const pointer = pointerWorld(event);
		if (activeDrag.kind === 'position') {
			const drag = activeDrag;
			const nextPosition = {
				x: activeDrag.startPosition.x + pointer.x - activeDrag.startPointer.x,
				y: activeDrag.startPosition.y + pointer.y - activeDrag.startPointer.y
			};
			drag.latestPosition = nextPosition;
			positionPreviews = { ...positionPreviews, [drag.endpoint.key]: nextPosition };
			scheduleDragUpdate(drag);
			return;
		}
		const distance = Math.hypot(pointer.x - activeDrag.center.x, pointer.y - activeDrag.center.y);
		const nextRadius = clampPositive(activeDrag.startRadius + distance - activeDrag.startDistance);
		if (activeDrag.kind === 'radius') {
			const drag = activeDrag;
			drag.latestRadius = nextRadius;
			radiusPreviews = { ...radiusPreviews, [drag.endpoint.key]: nextRadius };
			scheduleDragUpdate(drag);
		} else {
			const drag = activeDrag;
			drag.latestRadius = nextRadius;
			freezeRadiusPreviews = { ...freezeRadiusPreviews, [drag.endpoint.key]: nextRadius };
			scheduleDragUpdate(drag);
		}
	};

	const finishWorldPointer = async (event: PointerEvent): Promise<void> => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		event.preventDefault();
		const drag = activeDrag;
		activeDrag = null;
		if (worldSvg?.hasPointerCapture(event.pointerId)) {
			worldSvg.releasePointerCapture(event.pointerId);
		}
		await drainDragUpdates(drag);
		await drag.editSession.end();
	};

	const cancelWorldPointer = async (event: PointerEvent): Promise<void> => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		const drag = activeDrag;
		const key = drag.endpoint.key;
		if (drag.kind === 'position') {
			clearPositionPreview(key);
		} else if (drag.kind === 'radius') {
			clearRadiusPreview(key);
		} else {
			clearFreezeRadiusPreview(key);
		}
		activeDrag = null;
		await drainDragUpdates(drag);
		await drag.editSession.end();
	};

	const isSelected = (endpoint: SpatialEndpoint): boolean =>
		session?.selectedNodeId === endpoint.node.node_id;

	const endpointKindLabel = (endpoint: SpatialEndpoint): string =>
		endpoint.kind === 'source' ? 'Source' : 'Target';

	const relatedValuesLabel = (endpoint: SpatialEndpoint): string =>
		endpoint.kind === 'source' ? 'Targets' : 'Sources';

	const relatedValueAmount = (node: UiNodeDto | null): number => clamp01(floatValue(node));

	const debugLineOpacity = (weight: number): number =>
		weight <= DEBUG_WEIGHT_EPSILON ? 0.055 : 0.16 + clamp01(weight) * 0.74;

	const screenPx = (value: number, world: GraphWorldContentContext): number =>
		(value * world.remPx) / Math.max(0.000001, world.camera.zoom);

	const dashArray = (world: GraphWorldContentContext, ...values: number[]): string =>
		values.map((value) => `${screenPx(value, world)}`).join(' ');

	const debugConnectionDashArray = (
		role: DebugConnection['role'],
		world: GraphWorldContentContext
	): string | undefined => {
		if (role === 'neighbor') {
			return dashArray(world, 0.36, 0.2);
		}
		if (role === 'frozen') {
			return dashArray(world, 0.12, 0.14);
		}
		return undefined;
	};

	const debugLineWidth = (weight: number, world: GraphWorldContentContext): number =>
		screenPx(0.035 + clamp01(weight) * 0.17, world);

	const debugWeightLabel = (weight: number): string => `${Math.round(clamp01(weight) * 100)}%`;

	const debugDistanceLabel = (distance: number): string =>
		Number.isFinite(distance)
			? `d ${distance < 10 ? distance.toFixed(2) : distance.toFixed(1)}`
			: '';

	const pointPx = (value: number, world: GraphWorldContentContext): number =>
		value * SPATIALIZER_UNIT_REM * world.remPx;

	const polygonPoints = (points: Point2[], world: GraphWorldContentContext): string =>
		points.map((point) => `${pointPx(point.x, world)},${pointPx(point.y, world)}`).join(' ');

	onMount(() => {
		const unregisterFrame = registerCommandHandler(
			'view.frame',
			() => (panelOwnsFocus() ? (graphCanvas?.frameSelection() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterHome = registerCommandHandler(
			'view.home',
			() => (panelOwnsFocus() ? (graphCanvas?.home() ?? false) : false),
			{ priority: 100 }
		);
		return () => {
			unregisterFrame();
			unregisterHome();
			flushCameraPersistence();
		};
	});
</script>

{#snippet spatializerWorld(world: GraphWorldContentContext)}
	<svg
		bind:this={worldSvg}
		class="spatializer-map"
		role="application"
		aria-label="Spatializer map"
		onpointermove={handleWorldPointerMove}
		onpointerup={finishWorldPointer}
		onpointercancel={cancelWorldPointer}>
		{#each voronoiCellsForBounds(voronoiViewBounds(world)) as cell (cell.key)}
			<polygon
				class="voronoi-cell"
				class:highlighted={highlightedVoronoiCell(cell)}
				points={polygonPoints(cell.points, world)}
				fill={cell.color}
				stroke={cell.color}
				stroke-width={screenPx(0.04, world)} />
		{/each}

		{#if selectedDeadzoneSource()}
			{@const deadzoneSource = selectedDeadzoneSource()}
			{@const deadzoneBounds = voronoiViewBounds(world)}
			{@const deadzoneHoles = deadzoneSource ? deadzoneHolesForSource(deadzoneSource) : []}
			<path
				class="deadzone-highlight"
				d={deadzonePath(deadzoneBounds, deadzoneHoles, world)}
				fill={deadzoneSource?.color ?? 'currentColor'}
				fill-rule="evenodd"
				transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
			{#each deadzoneHoles as hole (hole.key)}
				<circle
					class="deadzone-hole-edge"
					cx={pointPx(hole.x, world)}
					cy={pointPx(hole.y, world)}
					r={pointPx(hole.radius, world)}
					stroke={hole.color}
					stroke-width={screenPx(0.06, world)}
					stroke-dasharray={dashArray(world, 0.38, 0.22)}
					transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
			{/each}
		{/if}

		{#if selectedVoronoiDeadzoneSource()}
			{@const voronoiDeadzoneSource = selectedVoronoiDeadzoneSource()}
			{@const frozenTargets = voronoiDeadzoneSource
				? frozenTargetsForSource(voronoiDeadzoneSource)
				: []}
			{#each frozenTargets as target (target.key)}
				{#if target.freezeRadius !== null && target.freezeRadius > 0}
					<circle
						class="deadzone-freeze-highlight"
						cx={pointPx(target.x, world)}
						cy={pointPx(target.y, world)}
						r={pointPx(target.freezeRadius, world)}
						fill={target.color}
						stroke={target.color}
						stroke-width={screenPx(0.08, world)}
						stroke-dasharray={dashArray(world, 0.34, 0.2)}
						transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
				{/if}
			{/each}
		{/if}

		{#if debugView && selectedEndpoint}
			<g class="debug-layer" aria-hidden="true">
				{#each debugVoronoiComputations as computation (computation.key)}
					{#if computation.current && computation.cellPoints.length > 0}
						<polygon
							class="debug-current-cell"
							points={polygonPoints(computation.cellPoints, world)}
							fill={computation.current.color}
							stroke={computation.current.color}
							stroke-width={screenPx(0.06, world)}
							stroke-dasharray={dashArray(world, 0.46, 0.28)} />
					{/if}
				{/each}

				{#each debugVoronoiGuides as guide (guide.key)}
					{@const neighborMid = pointBetween(guide.closestPoint, guide.neighborMeasurePoint, 0.5)}
					<line
						class="debug-neighbor-edge"
						x1={pointPx(guide.edgeStart.x, world)}
						y1={pointPx(guide.edgeStart.y, world)}
						x2={pointPx(guide.edgeEnd.x, world)}
						y2={pointPx(guide.edgeEnd.y, world)}
						stroke={guide.neighbor.color}
						stroke-width={screenPx(0.08, world)} />
					<line
						class="debug-edge-distance"
						x1={pointPx(guide.source.x, world)}
						y1={pointPx(guide.source.y, world)}
						x2={pointPx(guide.closestPoint.x, world)}
						y2={pointPx(guide.closestPoint.y, world)}
						stroke={guide.current.color}
						stroke-width={screenPx(0.05, world)}
						stroke-dasharray={dashArray(world, 0.22, 0.18)} />
					<line
						class="debug-neighbor-distance"
						x1={pointPx(guide.closestPoint.x, world)}
						y1={pointPx(guide.closestPoint.y, world)}
						x2={pointPx(guide.neighborMeasurePoint.x, world)}
						y2={pointPx(guide.neighborMeasurePoint.y, world)}
						stroke={guide.neighbor.color}
						stroke-width={screenPx(0.045, world)}
						stroke-dasharray={dashArray(world, 0.12, 0.2)} />
					<circle
						class="debug-edge-dot"
						cx={pointPx(guide.closestPoint.x, world)}
						cy={pointPx(guide.closestPoint.y, world)}
						r={screenPx(0.12, world)}
						fill={guide.neighbor.color}
						stroke-width={screenPx(0.05, world)} />
					<text
						class="debug-edge-label"
						x={pointPx(guide.closestPoint.x, world) + screenPx(0.18, world)}
						y={pointPx(guide.closestPoint.y, world) + screenPx(0.42, world)}
						fill={guide.neighbor.color}
						font-size={screenPx(0.48, world)}
						stroke-width={screenPx(0.18, world)}>
						{debugDistanceLabel(guide.edgeDistance)}
					</text>
					<text
						class="debug-edge-label"
						x={pointPx(neighborMid.x, world) + screenPx(0.18, world)}
						y={pointPx(neighborMid.y, world) - screenPx(0.18, world)}
						fill={guide.neighbor.color}
						font-size={screenPx(0.48, world)}
						stroke-width={screenPx(0.18, world)}>
						{debugDistanceLabel(guide.neighborDistance)}
					</text>
				{/each}

				{#each debugConnections as connection (connection.key)}
					{@const mid = pointBetween(connection.source, connection.target, 0.5)}
					<line
						class="debug-weight-line"
						class:current={connection.role === 'current'}
						class:neighbor={connection.role === 'neighbor'}
						class:frozen={connection.role === 'frozen'}
						x1={pointPx(connection.source.x, world)}
						y1={pointPx(connection.source.y, world)}
						x2={pointPx(connection.target.x, world)}
						y2={pointPx(connection.target.y, world)}
						stroke={connection.target.color}
						stroke-opacity={debugLineOpacity(connection.weight)}
						stroke-width={debugLineWidth(connection.weight, world)}
						stroke-dasharray={debugConnectionDashArray(connection.role, world)} />
					<circle
						class="debug-weight-dot"
						class:frozen={connection.role === 'frozen'}
						cx={pointPx(mid.x, world)}
						cy={pointPx(mid.y, world)}
						r={screenPx(0.09 + connection.weight * 0.18, world)}
						fill={connection.target.color}
						fill-opacity={debugLineOpacity(connection.weight)}
						stroke-width={screenPx(connection.role === 'frozen' ? 0.1 : 0.06, world)} />
					<text
						class="debug-weight-label"
						x={pointPx(mid.x, world) + screenPx(0.25, world)}
						y={pointPx(mid.y, world) - screenPx(0.18, world)}
						fill={connection.target.color}
						font-size={screenPx(0.58, world)}
						stroke-width={screenPx(0.18, world)}>
						{debugWeightLabel(connection.weight)}
					</text>
				{/each}
			</g>
		{/if}

		{#each targets as endpoint (endpoint.key)}
			<g
				class:disabled={!endpoint.enabled}
				class:selected={isSelected(endpoint)}
				class="endpoint target"
				transform={`translate(${pointPx(endpoint.x, world)} ${pointPx(endpoint.y, world)})`}>
				{#if spatializerMode === MODE_VORONOI && endpoint.freezeRadius !== null && endpoint.freezeRadius > 0}
					<circle
						class="freeze-radius-fill"
						r={pointPx(endpoint.freezeRadius, world)}
						fill={endpoint.color}
						stroke={endpoint.color}
						stroke-width={screenPx(0.06, world)}
						stroke-dasharray={dashArray(world, 0.42, 0.26)} />
					{#if endpoint.freezeRadiusWritable}
						<circle
							class="radius-hit freeze-radius-hit"
							role="button"
							tabindex="0"
							aria-label={`Resize ${endpoint.node.meta.label} freeze radius`}
							r={pointPx(endpoint.freezeRadius, world)}
							stroke-width={screenPx(0.55, world)}
							onkeydown={(event) => selectEndpointWithKeyboard(event, endpoint)}
							onpointerdown={(event) => startEndpointDrag(event, endpoint, 'freezeRadius')} />
					{/if}
				{/if}
				{#if endpoint.radius !== null && endpoint.radius > 0}
					<circle
						class="radius-fill"
						r={pointPx(endpoint.radius, world)}
						fill={endpoint.color}
						stroke={endpoint.color}
						stroke-width={screenPx(0.06, world)} />
					{#if endpoint.radiusWritable}
						<circle
							class="radius-hit"
							role="button"
							tabindex="0"
							aria-label={`Resize ${endpoint.node.meta.label} radius`}
							r={pointPx(endpoint.radius, world)}
							stroke-width={screenPx(0.55, world)}
							onkeydown={(event) => selectEndpointWithKeyboard(event, endpoint)}
							onpointerdown={(event) => startEndpointDrag(event, endpoint, 'radius')} />
					{/if}
				{/if}
				<line
					x1={-screenPx(0.42, world)}
					y1={-screenPx(0.42, world)}
					x2={screenPx(0.42, world)}
					y2={screenPx(0.42, world)}
					stroke={endpoint.color}
					stroke-width={screenPx(0.16, world)}
					stroke-linecap="round" />
				<line
					x1={-screenPx(0.42, world)}
					y1={screenPx(0.42, world)}
					x2={screenPx(0.42, world)}
					y2={-screenPx(0.42, world)}
					stroke={endpoint.color}
					stroke-width={screenPx(0.16, world)}
					stroke-linecap="round" />
				<circle
					class="endpoint-hit"
					role="button"
					tabindex="0"
					aria-label={`Move ${endpoint.node.meta.label}`}
					r={screenPx(0.72, world)}
					onkeydown={(event) => selectEndpointWithKeyboard(event, endpoint)}
					onpointerdown={(event) => startEndpointDrag(event, endpoint, 'position')} />
			</g>
		{/each}

		{#each sources as endpoint (endpoint.key)}
			<g
				class:disabled={!endpoint.enabled}
				class:selected={isSelected(endpoint)}
				class="endpoint source"
				transform={`translate(${pointPx(endpoint.x, world)} ${pointPx(endpoint.y, world)})`}>
				{#if endpoint.radius !== null && endpoint.radius > 0}
					<circle
						class="radius-fill"
						r={pointPx(endpoint.radius, world)}
						fill={endpoint.color}
						stroke={endpoint.color}
						stroke-width={screenPx(0.06, world)} />
					{#if endpoint.radiusWritable}
						<circle
							class="radius-hit"
							role="button"
							tabindex="0"
							aria-label={`Resize ${endpoint.node.meta.label} radius`}
							r={pointPx(endpoint.radius, world)}
							stroke-width={screenPx(0.55, world)}
							onkeydown={(event) => selectEndpointWithKeyboard(event, endpoint)}
							onpointerdown={(event) => startEndpointDrag(event, endpoint, 'radius')} />
					{/if}
				{/if}
				<circle
					class="source-dot"
					r={screenPx(0.36, world)}
					fill={endpoint.color}
					stroke={endpoint.color}
					stroke-width={screenPx(0.12, world)} />
				<circle
					class="endpoint-hit"
					role="button"
					tabindex="0"
					aria-label={`Move ${endpoint.node.meta.label}`}
					r={screenPx(0.72, world)}
					onkeydown={(event) => selectEndpointWithKeyboard(event, endpoint)}
					onpointerdown={(event) => startEndpointDrag(event, endpoint, 'position')} />
			</g>
		{/each}
	</svg>
{/snippet}

<section bind:this={panelRoot} class="spatializer-editor-panel" aria-label={panelState.title}>
	<header class="editor-toolbar">
		<label class="module-picker">
			<span>Module</span>
			<select value={String(activeModule?.node_id ?? '')} onchange={selectModule}>
				{#each modules as module (module.node_id)}
					<option value={module.node_id}>{module.meta.label}</option>
				{/each}
			</select>
		</label>
		<span class="toolbar-chip">{dimensionLabel}</span>
		<span class="toolbar-chip">{modeLabel}</span>
		<label class="debug-toggle">
			<input type="checkbox" checked={debugView} onchange={setDebugView} />
			<span>Debug</span>
		</label>
	</header>

	{#if activeModule}
		<div class="editor-body">
			<div class="canvas-region">
				<GraphCanvas
					bind:this={graphCanvas}
					{graphDocument}
					worldContent={spatializerWorld}
					worldBounds={graphWorldBounds}
					{initialCamera}
					minZoom={SPATIALIZER_MIN_ZOOM}
					onCameraChange={persistCamera}
					onBackgroundDoubleClick={createTargetAt}
					viewportInset={{ left: inspectorVisible ? inspectorWidth : 0 }}
					autoHomeOnMount={true}
					emptyLabel="" />
			</div>

			<aside
				class="endpoint-inspector"
				class:visible={inspectorVisible}
				style:width={`${inspectorWidth}px`}
				aria-label="Spatializer preview">
				<div class="endpoint-inspector-heading">
					<button
						type="button"
						class="endpoint-inspector-toggle"
						aria-pressed={inspectorVisible}
						title="Hide preview panel"
						onclick={() => (inspectorVisible = false)}>
						<span class="endpoint-inspector-chevron">‹</span>
						<span class="endpoint-inspector-label">Preview</span>
					</button>
				</div>
				<div class="endpoint-inspector-body">
					{#if selectedEndpoint}
						<div class="endpoint-summary">
							<span class="endpoint-swatch" style:background={selectedEndpoint.color}></span>
							<div>
								<strong>{selectedEndpoint.node.meta.label}</strong>
								<span>{endpointKindLabel(selectedEndpoint)}</span>
							</div>
						</div>

						<section class="related-values" aria-label="Computed values">
							<header>
								<span>{relatedValuesLabel(selectedEndpoint)}</span>
								<span>{relatedValues.length}</span>
							</header>
							<div class="related-value-list">
								{#each relatedValues as item (item.key)}
									<div class="related-value-row" class:disabled={!item.endpoint.enabled}>
										<div class="related-value-label">
											<span class="related-value-swatch" style:background={item.endpoint.color}
											></span>
											<span>{item.endpoint.node.meta.label}</span>
										</div>
										<div class="related-value-control">
											{#if item.valueParam}
												<div class="related-value-slider">
													<Slider
														value={relatedValueAmount(item.valueParam)}
														min={0}
														max={1}
														step={0.001}
														readOnly={true}
														disabled={!item.endpoint.enabled}
														fgColor={item.endpoint.color}
														showValue={true} />
												</div>
											{:else}
												<span class="related-value-missing">-</span>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						</section>
					{:else}
						<p class="empty-selection">No selection.</p>
					{/if}
				</div>
				<div
					class="endpoint-resize-handle"
					role="separator"
					aria-label="Resize preview panel"
					aria-orientation="vertical"
					onpointerdown={startInspectorResize}>
				</div>
			</aside>
			<button
				type="button"
				class="endpoint-show-tab"
				class:panel-visible={inspectorVisible}
				aria-hidden={inspectorVisible}
				title="Show preview panel"
				onclick={() => (inspectorVisible = true)}>
				<span class="endpoint-inspector-chevron">›</span>
				<span class="endpoint-inspector-label">Preview</span>
			</button>
		</div>
	{:else}
		<p class="missing">No Spatializer module found.</p>
	{/if}
</section>

<style>
	.spatializer-editor-panel {
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
		--spatializer-highlight-transition: 0.22s ease;
	}

	.editor-toolbar {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex: 0 0 auto;
		min-block-size: 2.4rem;
		padding: 0.35rem 0.55rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-text) 14%, transparent);
		background: color-mix(in srgb, var(--gc-color-background) 92%, var(--gc-color-text));
	}

	.module-picker {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		min-inline-size: 0;
		font-size: 0.78rem;
		color: color-mix(in srgb, var(--gc-color-text) 76%, transparent);
	}

	.module-picker select {
		max-inline-size: 18rem;
		min-inline-size: 10rem;
		padding: 0.25rem 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-text) 20%, transparent);
		border-radius: 0.25rem;
		background: var(--gc-color-panel, var(--gc-color-background));
		color: var(--gc-color-text);
		font: inherit;
	}

	.toolbar-chip {
		display: inline-flex;
		align-items: center;
		min-block-size: 1.45rem;
		padding: 0 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-accent) 35%, transparent);
		border-radius: 0.25rem;
		color: color-mix(in srgb, var(--gc-color-accent) 82%, var(--gc-color-text));
		font-size: 0.72rem;
		font-weight: 650;
	}

	.debug-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.32rem;
		margin-inline-start: auto;
		color: color-mix(in srgb, var(--gc-color-text) 72%, transparent);
		font-size: 0.72rem;
		font-weight: 600;
	}

	.debug-toggle input {
		inline-size: 0.9rem;
		block-size: 0.9rem;
		margin: 0;
		accent-color: var(--gc-color-accent);
	}

	.editor-body {
		position: relative;
		flex: 1 1 auto;
		inline-size: 100%;
		min-block-size: 0;
		min-inline-size: 0;
		overflow: hidden;
	}

	.canvas-region {
		position: absolute;
		inset: 0;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.endpoint-inspector {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		z-index: 30;
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		min-inline-size: 0;
		max-inline-size: calc(100% - 2rem);
		min-block-size: 0;
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		box-shadow: 0.5rem 0 2rem color-mix(in srgb, black 32%, transparent);
		transform: translateX(-100%);
		transition: transform 0.22s cubic-bezier(0.2, 0, 0.13, 1);
		pointer-events: none;
	}

	.endpoint-inspector.visible {
		transform: translateX(0);
		pointer-events: auto;
	}

	.endpoint-inspector-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.28rem 0.35rem 0.28rem 0.5rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}

	.endpoint-inspector-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.18rem 0.2rem 0.18rem 0.3rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition: background 0.12s;
	}

	.endpoint-inspector-toggle:hover {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 14%, transparent);
	}

	.endpoint-inspector-chevron {
		font-size: 0.9rem;
		line-height: 1;
		color: color-mix(in srgb, var(--gc-color-text) 65%, transparent);
	}

	.endpoint-inspector-label {
		font-size: 0.74rem;
		font-weight: 600;
	}

	.endpoint-inspector-body {
		display: flex;
		flex-direction: column;
		gap: 0.8rem;
		min-inline-size: 0;
		min-block-size: 0;
		padding: 0.75rem;
		overflow: auto;
	}

	.endpoint-show-tab {
		position: absolute;
		top: 0;
		left: 0;
		z-index: 35;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.55rem 0.35rem 0.4rem;
		border: none;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-end-end-radius: 0.4rem;
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition:
			opacity 0.18s,
			background 0.12s;
	}

	.endpoint-show-tab.panel-visible {
		opacity: 0;
		pointer-events: none;
	}

	.endpoint-show-tab:not(.panel-visible):hover {
		background: color-mix(
			in srgb,
			var(--gc-color-accent, #66a6ff) 18%,
			var(--gc-color-background-soft, #1a1a1a)
		);
	}

	.endpoint-resize-handle {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		z-index: 5;
		inline-size: 0.35rem;
		cursor: col-resize;
		touch-action: none;
	}

	.endpoint-resize-handle:hover,
	.endpoint-resize-handle:active {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 55%, transparent);
	}

	.endpoint-summary {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.5rem;
		min-inline-size: 0;
	}

	.endpoint-summary strong,
	.endpoint-summary span {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.endpoint-summary strong {
		font-size: 0.86rem;
	}

	.endpoint-summary span {
		font-size: 0.72rem;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
	}

	.endpoint-swatch {
		inline-size: 0.9rem;
		block-size: 0.9rem;
		border-radius: 50%;
		box-shadow: 0 0 0 0.08rem color-mix(in srgb, var(--gc-color-text) 35%, transparent);
	}

	.related-values {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-block-size: 0;
	}

	.related-values header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.72rem;
		font-weight: 650;
	}

	.related-value-list {
		display: flex;
		flex-direction: column;
		gap: 0.42rem;
		min-block-size: 0;
		overflow: visible;
	}

	.related-value-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.18rem;
		inline-size: 100%;
		min-inline-size: 0;
		padding: 0.18rem 0;
		border: 0.06rem solid transparent;
		border-radius: 0.25rem;
		background: transparent;
		color: var(--gc-color-text);
		font-size: 0.68rem;
	}

	.related-value-row.disabled {
		opacity: 0.48;
	}

	.related-value-label {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.38rem;
		inline-size: 100%;
		min-inline-size: 0;
		padding-inline: 0.08rem;
		color: color-mix(in srgb, var(--gc-color-text) 82%, transparent);
		font-weight: 550;
	}

	.related-value-label span:last-child {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.related-value-swatch {
		inline-size: 0.58rem;
		block-size: 0.58rem;
		border-radius: 50%;
		box-shadow: 0 0 0 0.06rem color-mix(in srgb, var(--gc-color-text) 28%, transparent);
	}

	.related-value-control {
		inline-size: 100%;
		min-inline-size: 0;
		overflow: hidden;
	}

	.related-value-slider {
		inline-size: 100%;
		min-inline-size: 0;
		block-size: 1rem;
	}

	.related-value-slider :global(.slider) {
		block-size: 100%;
		border-radius: 0.32rem;
	}

	.related-value-slider :global(.slider-label) {
		font-size: 0.62rem;
		font-weight: 650;
		color: var(--gc-color-text);
	}

	.related-value-missing {
		display: block;
		text-align: end;
		font-variant-numeric: tabular-nums;
		color: color-mix(in srgb, var(--gc-color-text) 52%, transparent);
	}

	.empty-selection,
	.missing {
		margin: 0;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.8rem;
	}

	.missing {
		padding: 0.8rem;
	}

	.spatializer-map {
		position: absolute;
		inset: 0 auto auto 0;
		inline-size: 0.1rem;
		block-size: 0.1rem;
		overflow: visible;
	}

	.voronoi-cell {
		fill-opacity: 0.16;
		stroke-opacity: 0.32;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition),
			filter var(--spatializer-highlight-transition);
	}

	.voronoi-cell.highlighted {
		fill-opacity: 0.2;
		stroke-opacity: 0.44;
	}

	.deadzone-highlight {
		fill-opacity: 0.09;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition);
	}

	.deadzone-hole-edge {
		fill: none;
		stroke-opacity: 0.34;
		pointer-events: none;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.deadzone-freeze-highlight {
		fill-opacity: 0.075;
		stroke-opacity: 0.38;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-layer {
		pointer-events: none;
	}

	.debug-current-cell {
		fill-opacity: 0.05;
		stroke-opacity: 0.42;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-neighbor-edge {
		stroke-opacity: 0.9;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-edge-distance {
		stroke-opacity: 0.68;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-neighbor-distance {
		stroke-opacity: 0.46;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-edge-dot {
		fill-opacity: 0.86;
		stroke: var(--gc-color-background);
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.debug-edge-label,
	.debug-weight-label {
		paint-order: stroke;
		stroke: var(--gc-color-background);
		stroke-linejoin: round;
		font-weight: 750;
		pointer-events: none;
		user-select: none;
	}

	.debug-edge-label {
		opacity: 0.82;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			opacity var(--spatializer-highlight-transition);
	}

	.debug-weight-line {
		stroke-opacity: 0.72;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.debug-weight-line.current {
		stroke-opacity: 0.9;
	}

	.debug-weight-line.frozen {
		stroke-opacity: 0.95;
	}

	.debug-weight-dot {
		stroke: color-mix(in srgb, var(--gc-color-background) 80%, transparent);
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.endpoint {
		cursor: grab;
	}

	.endpoint.disabled {
		opacity: 0.42;
	}

	.endpoint.selected .source-dot,
	.endpoint.selected .freeze-radius-fill,
	.endpoint.selected line {
		filter: drop-shadow(0 0 0.22rem var(--gc-color-accent));
	}

	.radius-fill {
		fill-opacity: 0.08;
		stroke-opacity: 0.48;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.freeze-radius-fill {
		fill-opacity: 0.04;
		stroke-opacity: 0.62;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.radius-hit {
		fill: none;
		stroke: transparent;
		cursor: ew-resize;
	}

	.source-dot {
		fill-opacity: 0.95;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition);
	}

	.endpoint-hit {
		fill: transparent;
		stroke: transparent;
	}

	@media (max-width: 54rem) {
		.endpoint-inspector {
			max-inline-size: calc(100% - 1.4rem);
		}
	}
</style>
