import type { UiColorDto, UiNodeDto } from '../types';
import nodeAddIcon from '../style/icons/node/add.svg';
import nodeDefaultIcon from '../style/icons/node/default.svg';
import folderIcon from '../style/icons/node/folder.svg';
import nodeManagerIcon from '../style/icons/node/manager.svg';
import scriptIcon from '../style/icons/node/script.svg';
import animationControlIcon from '../style/icons/parameter/control/animation.svg';
import parameterBoolIcon from '../style/icons/parameter/bool.svg';
import parameterColorIcon from '../style/icons/parameter/color.svg';
import parameterEnumIcon from '../style/icons/parameter/enum.svg';
import parameterFloatIcon from '../style/icons/parameter/float.svg';
import parameterIntIcon from '../style/icons/parameter/int.svg';
import parameterStringIcon from '../style/icons/parameter/string.svg';
import parameterTriggerIcon from '../style/icons/parameter/trigger.svg';
import parameterVec2Icon from '../style/icons/parameter/vec2.svg';
import parameterVec3Icon from '../style/icons/parameter/vec3.svg';
import parameterReferenceIcon from '../style/icons/parameter/reference.svg';
import parameterFileIcon from '../style/icons/parameter/file.svg';
import curveKeyIcon from '../style/icons/node/curve/key.svg';
import dashboardIcon from '../style/icons/node/dashboard.svg';

const detectedNodeIconModules = import.meta.glob('../../assets/icons/nodes/*.{svg,png}', {
	eager: true,
	import: 'default'
}) as Record<string, string>;

const detectedNodeIcons = Object.entries(detectedNodeIconModules).reduce<Record<string, string>>(
	(result, [path, iconUrl]) => {
		const fileName = path.split('/').at(-1) ?? '';
		const iconName = fileName.replace(/\.[^.]+$/, '');
		const extension = fileName.split('.').at(-1)?.toLowerCase();
		if (!(iconName in result) || extension === 'svg') {
			result[iconName] = iconUrl;
		}
		return result;
	},
	{}
);

const DEFAULT_FALLBACK_ICON = nodeDefaultIcon;
const DEFAULT_MANAGER_ICON = nodeManagerIcon;
const DEFAULT_PARAMETER_ICON = parameterTriggerIcon;

export interface NodeIconSet {
	manager?: string;
	parameter?: string;
	fallback?: string;
	nodeTypes?: Record<string, string>;
	parameterKinds?: Record<string, string>;
	categories?: Record<string, string>;
}

type ResolvedNodeIconSet = {
	manager: string;
	parameter: string;
	fallback: string;
	nodeTypes: Record<string, string>;
	parameterKinds: Record<string, string>;
	categories: Record<string, string>;
};

const defaultNodeIcons: ResolvedNodeIconSet = {
	manager: DEFAULT_MANAGER_ICON,
	parameter: DEFAULT_PARAMETER_ICON,
	fallback: DEFAULT_FALLBACK_ICON,
	nodeTypes: {
		add: nodeAddIcon,
		folder: folderIcon,
		script: scriptIcon,
		animation_curve: animationControlIcon,
		animation_curve_range: animationControlIcon,
		animation_curve_key: curveKeyIcon,
		animation_curve_easing: animationControlIcon,
		parameter_animation_control: animationControlIcon,
		dashboard: dashboardIcon
	},
	parameterKinds: {
		bool: parameterBoolIcon,
		color: parameterColorIcon,
		enum: parameterEnumIcon,
		float: parameterFloatIcon,
		int: parameterIntIcon,
		css_value: parameterFloatIcon,
		str: parameterStringIcon,
		file: parameterFileIcon,
		trigger: parameterTriggerIcon,
		vec2: parameterVec2Icon,
		vec3: parameterVec3Icon,
		reference: parameterReferenceIcon
	},
	categories: {}
};

const resolveNodeIcons = (overrides?: NodeIconSet): ResolvedNodeIconSet => ({
	manager: overrides?.manager ?? defaultNodeIcons.manager,
	parameter: overrides?.parameter ?? defaultNodeIcons.parameter,
	fallback: overrides?.fallback ?? defaultNodeIcons.fallback,
	nodeTypes: {
		...defaultNodeIcons.nodeTypes,
		...detectedNodeIcons,
		...(overrides?.nodeTypes ?? {})
	},
	parameterKinds: {
		...defaultNodeIcons.parameterKinds,
		...(overrides?.parameterKinds ?? {})
	},
	categories: {
		...defaultNodeIcons.categories,
		...(overrides?.categories ?? {})
	}
});

let activeNodeIcons: ResolvedNodeIconSet = resolveNodeIcons();

export const configureNodeIcons = (overrides?: NodeIconSet): void => {
	activeNodeIcons = resolveNodeIcons(overrides);
};

export const getIconURLForNodeType = (
	nodeType: string | null | undefined,
	itemKind?: string | null
): string | null => {
	const normalizedNodeType = nodeType?.trim() ?? '';
	const normalizedItemKind = itemKind?.trim() ?? '';

	if (normalizedNodeType.length > 0) {
		const icon = activeNodeIcons.nodeTypes[normalizedNodeType];
		if (icon) {
			return icon;
		}
	}

	if (normalizedItemKind.length > 0) {
		const icon = activeNodeIcons.nodeTypes[normalizedItemKind];
		if (icon) {
			return icon;
		}
	}

	return null;
};

export const getIconURLForNode = (node: UiNodeDto | null): string => {
	if (!node) {
		return activeNodeIcons.fallback;
	}

	if (node.meta.presentation?.icon) {
		return node.meta.presentation.icon;
	}

	const isManager = (node.creatable_user_items?.length ?? 0) > 0;

	if (node.data.kind === 'parameter') {
		const parameterKind = node.data.param.value.kind;
		return (
			activeNodeIcons.parameterKinds[parameterKind] ??
			activeNodeIcons.parameter ??
			activeNodeIcons.fallback
		);
	}

	let result = getIconURLForNodeType(node.node_type, node.user_item_kind);
	if (!result) {
		// Fallback to manager icon if it's a node type we don't have a specific icon for, but it has creatable items (i.e. it's a manager of some kind)
		if (isManager) {
			result = activeNodeIcons.manager;
		}
	}
	return result ?? activeNodeIcons.fallback;
};

export const getIconURLForCategory = (category: string | null | undefined): string | null => {
	const normalizedCategory = category?.trim() ?? '';
	if (normalizedCategory.length === 0) {
		return null;
	}
	return activeNodeIcons.categories[normalizedCategory] ?? null;
};

const clampChannel = (value: number, fallback: number): number => {
	if (!Number.isFinite(value)) {
		return fallback;
	}
	return Math.max(0, Math.min(1, value));
};

const normalizeNodeColor = (color: UiColorDto | undefined): UiColorDto | null => {
	if (!color) {
		return null;
	}

	return {
		r: clampChannel(color.r, 0),
		g: clampChannel(color.g, 0),
		b: clampChannel(color.b, 0),
		a: clampChannel(color.a, 1)
	};
};

const toCssRgba = (color: UiColorDto): string => {
	const red = Math.round(color.r * 255);
	const green = Math.round(color.g * 255);
	const blue = Math.round(color.b * 255);
	return `rgba(${red}, ${green}, ${blue}, ${color.a})`;
};

const hashString = (value: string): number => {
	let hash = 2166136261;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index);
		hash = Math.imul(hash, 16777619);
	}
	return hash >>> 0;
};

const hslToRgb = (hue: number, saturation: number, lightness: number): UiColorDto => {
	const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
	const huePrime = hue / 60;
	const second = chroma * (1 - Math.abs((huePrime % 2) - 1));

	let red = 0;
	let green = 0;
	let blue = 0;

	if (huePrime >= 0 && huePrime < 1) {
		red = chroma;
		green = second;
	} else if (huePrime < 2) {
		red = second;
		green = chroma;
	} else if (huePrime < 3) {
		green = chroma;
		blue = second;
	} else if (huePrime < 4) {
		green = second;
		blue = chroma;
	} else if (huePrime < 5) {
		red = second;
		blue = chroma;
	} else {
		red = chroma;
		blue = second;
	}

	const match = lightness - chroma / 2;
	return {
		r: red + match,
		g: green + match,
		b: blue + match,
		a: 1
	};
};

const getDerivedColorKeyForNode = (node: UiNodeDto): string => {
	if (node.data.kind === 'parameter') {
		return `param:${node.data.param.value.kind}`;
	}
	if (node.user_item_kind.trim().length > 0) {
		return `item:${node.user_item_kind}`;
	}
	return `node:${node.node_type}`;
};

const getDerivedColorForNode = (node: UiNodeDto): UiColorDto => {
	let autoColor = node.data.kind === 'parameter';
	if (!autoColor) {
		if (node.node_type === 'folder') {
			return { r: 0.4, g: 0.4, b: 0.4, a: 1 };
		} else if (node.user_item_kind.trim().length > 0) {
			return { r: 0.4, g: 0.4, b: 0.4, a: 1 };
		}
	}

	const hash = hashString(getDerivedColorKeyForNode(node));
	const hue = hash % 360;
	const saturation = 0.52 + ((hash >>> 9) % 18) / 100;
	const lightness = 0.48 + ((hash >>> 17) % 12) / 100;
	return hslToRgb(hue, saturation, lightness);
};

export const getContainerColorForNode = (node: UiNodeDto | null): string => {
	if (!node) {
		return 'rgba(30,30,30, 1)';
	}

	const explicitColor = normalizeNodeColor(
		node.meta.presentation?.color ?? node.meta.presentation?.default_color
	);
	if (explicitColor) {
		return toCssRgba(explicitColor);
	}

	return toCssRgba(getDerivedColorForNode(node));
};

// Utility function to generate rounded box with the name inside. the box is white transparent and text is dark grey
export const generateIconWithText = (text: string, color?: string): string => {
	if (typeof document === 'undefined') {
		return activeNodeIcons.fallback;
	}

	const canvas = document.createElement('canvas');
	const size = 64;
	canvas.width = size;
	canvas.height = size;
	const ctx = canvas.getContext('2d');
	if (!ctx) {
		return activeNodeIcons.fallback;
	}

	// Draw rounded rectangle
	const radius = 12;
	ctx.fillStyle = color ?? 'rgba(255, 255, 255, 0.8)';
	ctx.beginPath();
	ctx.moveTo(radius, 0);
	ctx.lineTo(size - radius, 0);
	ctx.quadraticCurveTo(size, 0, size, radius);
	ctx.lineTo(size, size - radius);
	ctx.quadraticCurveTo(size, size, size - radius, size);
	ctx.lineTo(radius, size);
	ctx.quadraticCurveTo(0, size, 0, size - radius);
	ctx.lineTo(0, radius);
	ctx.quadraticCurveTo(0, 0, radius, 0);
	ctx.fill();

	// Draw text
	ctx.fillStyle = 'rgba(255,255,255,.8)';
	ctx.font = 'bold 24px sans-serif';
	ctx.textAlign = 'center';
	ctx.textBaseline = 'middle';
	ctx.fillText(text, size / 2, size / 2);

	return canvas.toDataURL();
};
