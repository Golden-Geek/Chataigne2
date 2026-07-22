import type { GraphState } from '../../../store/graph.svelte';
import type {
	NodeId,
	ParamValue,
	UiNodeDto,
	UiParamDto,
	UiParamValueProjection
} from '../../../types';
import { cssValueFromParamValue, formatCssValue, type CssValueData } from '../../../css-value';

export type DashboardLayoutKind =
	| 'free'
	| 'horizontal'
	| 'vertical'
	| 'grid'
	| 'accordion'
	| 'tabs';

export type DashboardGridDirection = 'row' | 'column';

export type DashboardLabelPlacement = 'left' | 'right' | 'top' | 'bottom' | 'inside';

export interface DashboardPlacement {
	labelPlacement: DashboardLabelPlacement | null;
	position: { x: number; y: number };
	anchor: DashboardAnchor;
	size: { width: CssValueData; height: CssValueData };
	sizeEnabled: { width: boolean; height: boolean };
}

export type DashboardAnchor =
	| 'top-left'
	| 'top-center'
	| 'top-right'
	| 'center-left'
	| 'center'
	| 'center-right'
	| 'bottom-left'
	| 'bottom-center'
	| 'bottom-right';

export interface DashboardGap {
	x: CssValueData;
	y: CssValueData;
}

export interface DashboardPageSize {
	enabled: boolean;
	widthPx: number;
	heightPx: number;
}

export interface DashboardSnapGrid {
	enabled: boolean;
	step: number;
}

export interface DashboardGridSettings {
	direction: DashboardGridDirection;
	lineCount: number;
}

export const getLiveNode = (graph: GraphState | null, node: UiNodeDto): UiNodeDto => {
	return graph?.nodesById.get(node.node_id) ?? node;
};

export const getDirectChildren = (
	graph: GraphState | null,
	node: UiNodeDto | null
): UiNodeDto[] => {
	if (!graph || !node) {
		return [];
	}
	return node.children
		.map((childId) => graph.nodesById.get(childId))
		.filter((child): child is UiNodeDto => child !== undefined);
};

export const getDirectItemChildren = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	nodeType?: string
): UiNodeDto[] => {
	return getDirectChildren(graph, node).filter((child) => {
		if (child.user_role !== 'itemRoot') {
			return false;
		}
		if (!nodeType) {
			return true;
		}
		return child.node_type === nodeType;
	});
};

export const getDirectParamNode = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	declId: string
): UiNodeDto | null => {
	if (!graph || !node) {
		return null;
	}

	const exactDeclId = declId.trim();
	if (exactDeclId.length === 0) {
		return null;
	}

	const stack = getDirectChildren(graph, node).filter((child) => child.user_role !== 'itemRoot');
	let basenameMatch: UiNodeDto | null = null;
	while (stack.length > 0) {
		const child = stack.shift();
		if (!child) {
			continue;
		}
		if (child.data.kind === 'parameter') {
			if (child.decl_id === exactDeclId) {
				return child;
			}
			if (basenameMatch === null && child.decl_id.split('/').at(-1) === exactDeclId) {
				basenameMatch = child;
			}
		}
		for (const structuralChild of getDirectChildren(graph, child)) {
			if (structuralChild.user_role !== 'itemRoot') {
				stack.unshift(structuralChild);
			}
		}
	}

	return basenameMatch;
};

export const getDirectParam = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	declId: string
): UiParamDto | null => {
	const paramNode = getDirectParamNode(graph, node, declId);
	if (!paramNode || paramNode.data.kind !== 'parameter') {
		return null;
	}
	return paramNode.data.param;
};

export const getDashboardLayoutKind = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	fallback: DashboardLayoutKind = 'free'
): DashboardLayoutKind => {
	const param = getDirectParam(graph, node, 'layout_kind');
	if (!param || param.value.kind !== 'enum') {
		return fallback;
	}
	const value = param.value.value as DashboardLayoutKind;
	if (
		value === 'free' ||
		value === 'horizontal' ||
		value === 'vertical' ||
		value === 'grid' ||
		value === 'accordion' ||
		value === 'tabs'
	) {
		return value;
	}
	return fallback;
};

const asNumber = (value: ParamValue | undefined, fallback: number): number => {
	if (!value) {
		return fallback;
	}
	if (value.kind === 'float' || value.kind === 'int') {
		return Number(value.value);
	}
	return fallback;
};

const asDashboardLabelPlacement = (
	value: ParamValue | undefined,
	fallback: DashboardLabelPlacement
): DashboardLabelPlacement => {
	if (
		value?.kind === 'enum' &&
		(value.value === 'left' ||
			value.value === 'right' ||
			value.value === 'top' ||
			value.value === 'bottom' ||
			value.value === 'inside')
	) {
		return value.value;
	}
	return fallback;
};

export const getDashboardPlacement = (
	graph: GraphState | null,
	node: UiNodeDto | null
): DashboardPlacement => {
	const anchorParam = getDirectParam(graph, node, 'anchor');
	const labelPlacementNode = getDirectParamNode(graph, node, 'label_placement');
	const widthNode = getDirectParamNode(graph, node, 'width');
	const heightNode = getDirectParamNode(graph, node, 'height');
	const anchor =
		anchorParam?.value.kind === 'enum' &&
		[
			'top-left',
			'top-center',
			'top-right',
			'center-left',
			'center',
			'center-right',
			'bottom-left',
			'bottom-center',
			'bottom-right'
		].includes(anchorParam.value.value)
			? (anchorParam.value.value as DashboardAnchor)
			: 'top-left';
	const positionParam = getDirectParam(graph, node, 'position');
	return {
		labelPlacement:
			labelPlacementNode?.meta.enabled && labelPlacementNode.data.kind === 'parameter'
				? asDashboardLabelPlacement(labelPlacementNode.data.param.value, 'top')
				: null,
		position: {
			x: positionParam?.value.kind === 'vec2' ? Number(positionParam.value.value[0] ?? 0) : 0,
			y: positionParam?.value.kind === 'vec2' ? Number(positionParam.value.value[1] ?? 0) : 0
		},
		anchor,
		size: {
			width: cssValueFromParamValue(
				widthNode?.data.kind === 'parameter' ? widthNode.data.param.value : undefined,
				{
					value: 12,
					unit: 'rem'
				}
			),
			height: cssValueFromParamValue(
				heightNode?.data.kind === 'parameter' ? heightNode.data.param.value : undefined,
				{
					value: 4,
					unit: 'rem'
				}
			)
		},
		sizeEnabled: {
			width: widthNode?.meta.enabled ?? false,
			height: heightNode?.meta.enabled ?? false
		}
	};
};

export const getGap = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	fallback: DashboardGap = {
		x: { value: 1, unit: 'rem' },
		y: { value: 1, unit: 'rem' }
	}
): DashboardGap => {
	return {
		x: cssValueFromParamValue(getDirectParam(graph, node, 'gap_x')?.value, fallback.x),
		y: cssValueFromParamValue(getDirectParam(graph, node, 'gap_y')?.value, fallback.y)
	};
};

export const getDashboardPageSize = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	fallback: [number, number] = [1920, 1080]
): DashboardPageSize => {
	const pageSizeNode = getDirectParamNode(graph, node, 'layout/page_size');
	const pageSizeParam = pageSizeNode?.data.kind === 'parameter' ? pageSizeNode.data.param : null;
	const pageSizeValue = pageSizeParam?.value.kind === 'vec2' ? pageSizeParam.value.value : fallback;
	return {
		enabled: pageSizeNode?.meta.enabled ?? false,
		widthPx: Math.max(1, Math.round(Number(pageSizeValue[0] ?? fallback[0]))),
		heightPx: Math.max(1, Math.round(Number(pageSizeValue[1] ?? fallback[1])))
	};
};

export const getDashboardSnapGrid = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	fallback = 1
): DashboardSnapGrid => {
	const paramNode = getDirectParamNode(graph, node, 'snap_grid');
	const step =
		paramNode?.data.kind === 'parameter'
			? asNumber(paramNode.data.param.value, fallback)
			: fallback;
	return {
		enabled: (paramNode?.meta.enabled ?? false) && step > 0,
		step: Math.max(0, step)
	};
};

export const getDashboardGridSettings = (
	graph: GraphState | null,
	node: UiNodeDto | null,
	fallback: DashboardGridSettings = { direction: 'row', lineCount: 3 }
): DashboardGridSettings => {
	const directionParam = getDirectParam(graph, node, 'grid_direction');
	const direction =
		directionParam?.value.kind === 'enum' &&
		(directionParam.value.value === 'row' || directionParam.value.value === 'column')
			? directionParam.value.value
			: fallback.direction;
	const lineCount = Math.max(
		1,
		Math.round(asNumber(getDirectParam(graph, node, 'grid_line_count')?.value, fallback.lineCount))
	);
	return { direction, lineCount };
};

export const formatParamValue = (value: ParamValue | null | undefined): string => {
	if (!value) {
		return 'Unbound';
	}

	switch (value.kind) {
		case 'trigger':
			return 'Trigger';
		case 'int':
		case 'float':
			return String(value.value);
		case 'str':
		case 'file':
		case 'enum':
			return value.value;
		case 'bool':
			return value.value ? 'True' : 'False';
		case 'css_value':
			return formatCssValue(value);
		case 'vec2':
			return `${value.value[0]}, ${value.value[1]}`;
		case 'vec3':
			return `${value.value[0]}, ${value.value[1]}, ${value.value[2]}`;
		case 'color':
			return `${value.value.map((channel) => channel.toFixed(2)).join(', ')}`;
		case 'reference':
			return value.cached_name?.trim() || value.uuid.slice(0, 8);
	}
};

export const findNodeByUuid = (graph: GraphState | null, uuid: string): UiNodeDto | null => {
	if (!graph || uuid.length === 0) {
		return null;
	}
	for (const candidate of graph.nodesById.values()) {
		if (candidate.uuid === uuid) {
			return candidate;
		}
	}
	return null;
};

export const resolveReferenceTarget = (
	graph: GraphState | null,
	value: ParamValue | null | undefined
): UiNodeDto | null => {
	if (!graph || !value || value.kind !== 'reference') {
		return null;
	}
	if (typeof value.cached_id === 'number') {
		const byId = graph.nodesById.get(value.cached_id);
		if (byId) {
			return byId;
		}
	}
	return findNodeByUuid(graph, value.uuid);
};

export const nodePathFromRoot = (graph: GraphState | null, targetNodeId: NodeId): string[] => {
	if (!graph || graph.rootId === null) {
		return [];
	}
	if (targetNodeId === graph.rootId) {
		return [];
	}

	const reversed: string[] = [];
	let current: NodeId | undefined = targetNodeId;
	while (current !== undefined) {
		if (current === graph.rootId) {
			reversed.reverse();
			return reversed;
		}
		const currentNode = graph.nodesById.get(current);
		if (!currentNode) {
			return [];
		}
		reversed.push(currentNode.decl_id);
		current = graph.parentById.get(current);
	}

	return [];
};

export const createReferenceValue = (
	graph: GraphState | null,
	targetNode: UiNodeDto,
	projection?: UiParamValueProjection
): ParamValue => {
	return {
		kind: 'reference',
		uuid: targetNode.uuid,
		projection,
		cached_id: targetNode.node_id,
		cached_name: targetNode.meta.label,
		relative_path_from_root: nodePathFromRoot(graph, targetNode.node_id)
	};
};
