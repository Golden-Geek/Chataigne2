import type { UiNodeDto, UiColorDto, WorkbenchSession } from 'golden_ui';
import type { SpatialEndpoint, EndpointKind, PositionKind, ValueLayout } from './types';
import { SpatializerChildIndex } from './child-index';
import {
	POSITION_2D_DECL_ID,
	POSITION_3D_DECL_ID,
	RADIUS_DECL_ID,
	FREEZE_RADIUS_DECL_ID,
	VALUE_TARGET_DECL_PREFIX,
	VALUE_SOURCE_DECL_PREFIX,
	VALUE_LAYOUT_SOURCE_CENTRIC,
	VALUE_LAYOUT_TARGET_CENTRIC,
	ENDPOINT_PALETTE
} from './constants';

const simpleUuid = (uuid: string): string => uuid.replaceAll('-', '').toLowerCase();

const valueDeclId = (prefix: string, uuid: string): string => `${prefix}_${simpleUuid(uuid)}`;

export const enumValue = (node: UiNodeDto | null): string =>
	node?.data.kind === 'parameter' && node.data.param.value.kind === 'enum'
		? node.data.param.value.value
		: '';

const enumToken = (value: string): string =>
	value
		.trim()
		.toLowerCase()
		.replace(/[\s_-]/g, '');

export const valueLayoutFromEnum = (value: string): ValueLayout =>
	enumToken(value) === 'targetcentric' ? VALUE_LAYOUT_TARGET_CENTRIC : VALUE_LAYOUT_SOURCE_CENTRIC;

export const enumLabel = (node: UiNodeDto | null): string => {
	if (node?.data.kind !== 'parameter' || node.data.param.value.kind !== 'enum') {
		return '';
	}
	const value = node.data.param.value.value;
	return (
		node.data.param.constraints.enum_options.find((option) => option.variant_id === value)?.label ??
		value
	);
};

export const floatValue = (node: UiNodeDto | null): number | null =>
	node?.data.kind === 'parameter' && node.data.param.value.kind === 'float'
		? node.data.param.value.value
		: null;

export const clampPositive = (value: number): number =>
	Math.max(0, Number.isFinite(value) ? value : 0);

export const clamp01 = (value: number | null): number =>
	Math.min(1, Math.max(0, Number.isFinite(value ?? Number.NaN) ? (value ?? 0) : 0));

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

export const relatedValueAmount = (node: UiNodeDto | null): number => clamp01(floatValue(node));
export function createSpatializerProjection(
	getGraph: () => WorkbenchSession['graph']['state'] | null,
	getGeneration: () => unknown
) {
	let graph = $derived(getGraph());
	const childIndex = new SpatializerChildIndex();
	let indexedNodes = $derived.by(() => {
		const snapshot = graph;
		childIndex.update(snapshot, getGeneration());
		return snapshot?.nodesById;
	});

	const childByKey = (node: UiNodeDto | null, key: string): UiNodeDto | null => {
		indexedNodes;
		return childIndex.child(node, key);
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
	return { childByKey, childByType, childNodes, endpointFromNode };
}
