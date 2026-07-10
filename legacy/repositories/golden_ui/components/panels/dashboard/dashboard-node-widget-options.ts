import type { GraphState } from '../../../store/graph.svelte';
import type { ParamValue, UiNodeDto, UiRangeConstraint } from '../../../types';
import { getDirectParam } from './dashboard-model';

const getWidgetParamValue = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string
): ParamValue | null => {
	if (!widgetNode) {
		return null;
	}
	return getDirectParam(graph, widgetNode, declId)?.value ?? null;
};

const getWidgetNode = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string
): UiNodeDto | null => {
	if (!graph || !widgetNode) {
		return null;
	}

	const exactDeclId = declId.trim();
	if (exactDeclId.length === 0) {
		return null;
	}

	const stack = [...widgetNode.children];
	let basenameMatch: UiNodeDto | null = null;
	while (stack.length > 0) {
		const childId = stack.shift();
		if (childId === undefined) {
			continue;
		}
		const child = graph.nodesById.get(childId);
		if (!child) {
			continue;
		}
		if (child.decl_id === exactDeclId) {
			return child;
		}
		if (basenameMatch === null && child.decl_id.split('/').at(-1) === exactDeclId) {
			basenameMatch = child;
		}
		stack.unshift(...child.children);
	}

	return basenameMatch;
};

const getEnabledWidgetParamValue = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string
): ParamValue | null => {
	const paramNode = getWidgetNode(graph, widgetNode, declId);
	if (!paramNode?.meta.enabled || paramNode.data.kind !== 'parameter') {
		return null;
	}
	return paramNode.data.param.value;
};

const getEnabledWidgetNode = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string
): UiNodeDto | null => {
	const node = getWidgetNode(graph, widgetNode, declId);
	return node?.meta.enabled ? node : null;
};

const asFiniteNumber = (value: ParamValue | null): number | undefined => {
	if (!value) {
		return undefined;
	}
	if (value.kind === 'int' || value.kind === 'float') {
		return Number.isFinite(value.value) ? value.value : undefined;
	}
	return undefined;
};

const asFiniteVector = (value: ParamValue | null, dimensions: 2 | 3): number[] | undefined => {
	if (!value) {
		return undefined;
	}
	let vectorValue: number[] | null = null;
	if (dimensions === 2 && value.kind === 'vec2') {
		vectorValue = value.value;
	} else if (dimensions === 3 && value.kind === 'vec3') {
		vectorValue = value.value;
	}
	if (!vectorValue) {
		return undefined;
	}
	const result = new Array<number>(dimensions);
	for (let index = 0; index < dimensions; index += 1) {
		const component = Number(vectorValue[index]);
		if (!Number.isFinite(component)) {
			return undefined;
		}
		result[index] = component;
	}
	return result;
};

const actualScalarBounds = (
	targetNode: UiNodeDto
): {
	min: number | undefined;
	max: number | undefined;
} => {
	if (targetNode.data.kind !== 'parameter') {
		return { min: undefined, max: undefined };
	}
	const range = targetNode.data.param.constraints.range;
	if (range?.kind !== 'uniform') {
		return { min: undefined, max: undefined };
	}
	return {
		min: typeof range.min === 'number' && Number.isFinite(range.min) ? range.min : undefined,
		max: typeof range.max === 'number' && Number.isFinite(range.max) ? range.max : undefined
	};
};

const actualVectorComponentBounds = (
	targetNode: UiNodeDto,
	index: number
): {
	min: number | undefined;
	max: number | undefined;
} => {
	if (targetNode.data.kind !== 'parameter') {
		return { min: undefined, max: undefined };
	}
	const range = targetNode.data.param.constraints.range;
	if (!range) {
		return { min: undefined, max: undefined };
	}
	if (range.kind === 'uniform') {
		return {
			min: typeof range.min === 'number' && Number.isFinite(range.min) ? range.min : undefined,
			max: typeof range.max === 'number' && Number.isFinite(range.max) ? range.max : undefined
		};
	}
	return {
		min:
			typeof range.min?.[index] === 'number' && Number.isFinite(range.min[index])
				? range.min[index]
				: undefined,
		max:
			typeof range.max?.[index] === 'number' && Number.isFinite(range.max[index])
				? range.max[index]
				: undefined
	};
};

const adaptBoundsToActualRange = (
	overrideMin: number | undefined,
	overrideMax: number | undefined,
	actualMin: number | undefined,
	actualMax: number | undefined
): {
	min: number | undefined;
	max: number | undefined;
} => {
	const hasUsableActualPair =
		actualMin !== undefined && actualMax !== undefined && actualMax > actualMin;

	let min = overrideMin;
	let max = overrideMax;

	if (actualMin !== undefined) {
		min = min === undefined ? actualMin : Math.max(actualMin, min);
	}
	if (actualMax !== undefined) {
		max = max === undefined ? actualMax : Math.min(actualMax, max);
	}
	if (min !== undefined && actualMax !== undefined) {
		min = Math.min(min, actualMax);
	}
	if (max !== undefined && actualMin !== undefined) {
		max = Math.max(max, actualMin);
	}
	if (min !== undefined && max !== undefined && min >= max) {
		return {
			min: hasUsableActualPair ? actualMin : undefined,
			max: hasUsableActualPair ? actualMax : undefined
		};
	}
	return { min, max };
};

export const getWidgetBoolOption = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string,
	fallback: boolean
): boolean => {
	const value = getWidgetParamValue(graph, widgetNode, declId);
	return value?.kind === 'bool' ? value.value : fallback;
};

export const getWidgetIntOption = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string,
	fallback: number
): number => {
	const value = getWidgetParamValue(graph, widgetNode, declId);
	if (value?.kind === 'int' || value?.kind === 'float') {
		return value.value;
	}
	return fallback;
};

export const getEnabledWidgetNumberOption = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string
): number | null => asFiniteNumber(getEnabledWidgetParamValue(graph, widgetNode, declId)) ?? null;

export const getWidgetEnumOption = <T extends string>(
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	declId: string,
	fallback: T,
	allowed: readonly T[]
): T => {
	const value = getWidgetParamValue(graph, widgetNode, declId);
	const rawValue = value?.kind === 'enum' || value?.kind === 'str' ? value.value : null;
	return allowed.includes(rawValue as T) ? (rawValue as T) : fallback;
};

export const clampWidgetMaxDecimals = (value: number | undefined, fallback: number): number => {
	if (value === undefined || !Number.isFinite(value)) {
		return fallback;
	}
	return Math.max(0, Math.min(8, Math.round(value)));
};

export const getEffectiveWidgetScalarRange = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	targetNode: UiNodeDto
): UiRangeConstraint | null => {
	if (
		targetNode.data.kind !== 'parameter' ||
		(targetNode.data.param.value.kind !== 'int' && targetNode.data.param.value.kind !== 'float')
	) {
		return null;
	}

	const customRange = asFiniteVector(
		getEnabledWidgetParamValue(graph, widgetNode, 'custom_range'),
		2
	);
	const overrideMin = customRange?.[0];
	const overrideMax = customRange?.[1];
	const actual = actualScalarBounds(targetNode);
	const { min, max } = adaptBoundsToActualRange(overrideMin, overrideMax, actual.min, actual.max);

	if (min === undefined && max === undefined) {
		return null;
	}

	return {
		kind: 'uniform',
		...(min !== undefined ? { min } : {}),
		...(max !== undefined ? { max } : {})
	};
};

export const getEffectiveWidgetVectorRange = (
	graph: GraphState | null,
	widgetNode: UiNodeDto | null,
	targetNode: UiNodeDto,
	dimensions: 2 | 3
): UiRangeConstraint | null => {
	if (
		targetNode.data.kind !== 'parameter' ||
		(dimensions === 2 && targetNode.data.param.value.kind !== 'vec2') ||
		(dimensions === 3 && targetNode.data.param.value.kind !== 'vec3')
	) {
		return null;
	}

	const customRangeNode = getEnabledWidgetNode(graph, widgetNode, 'custom_range');
	const overrideMin = customRangeNode
		? asFiniteVector(
				getEnabledWidgetParamValue(graph, widgetNode, 'custom_range/range_min'),
				dimensions
			)
		: undefined;
	const overrideMax = customRangeNode
		? asFiniteVector(
				getEnabledWidgetParamValue(graph, widgetNode, 'custom_range/range_max'),
				dimensions
			)
		: undefined;

	const minValues = new Array<number>(dimensions);
	const maxValues = new Array<number>(dimensions);
	let hasMin = false;
	let hasMax = false;

	for (let index = 0; index < dimensions; index += 1) {
		const actual = actualVectorComponentBounds(targetNode, index);
		const { min, max } = adaptBoundsToActualRange(
			overrideMin?.[index],
			overrideMax?.[index],
			actual.min,
			actual.max
		);
		if (min !== undefined) {
			minValues[index] = min;
			hasMin = true;
		}
		if (max !== undefined) {
			maxValues[index] = max;
			hasMax = true;
		}
	}

	if (!hasMin && !hasMax) {
		return null;
	}

	return {
		kind: 'components',
		...(hasMin ? { min: minValues } : {}),
		...(hasMax ? { max: maxValues } : {})
	};
};
