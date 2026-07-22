import type { Snippet } from 'svelte';
import type { UiNodeDto } from '../../../types';
import type { NodePickerModalView } from '../../../store/node-picker-modal.svelte';
import ScriptNodeInspector from './nodes/ScriptNodeInspector.svelte';
import AnimationCurveNodeInspector from './nodes/AnimationCurveNodeInspector.svelte';
import GradientNodeInspector from './nodes/GradientNodeInspector.svelte';

export type NodeInspectorOrder = 'first' | 'last' | 'solo' | '';

export interface NodeInspectorComponentProps {
	node: UiNodeDto;
	level: number;
	order: NodeInspectorOrder;
	includeChildren?: boolean;
	maxChildLevel?: number | null;
	layoutMode?: 'default' | 'dashboard';
	collapsed?: boolean;
	hasChildren?: boolean;
	toggleCollapsed?: () => void;
	setCollapsed?: (collapsed: boolean) => void;
	defaultHeader?: Snippet<[Snippet?]>;
	defaultContent?: Snippet<[Snippet?, String?]>;
	defaultChildren?: Snippet<[String?]>;
	referencePickerViews?: NodePickerModalView[];
}

export interface NodeInspectorPanelHeaderComponentProps {
	node: UiNodeDto;
	defaultHeader?: Snippet<[Snippet?]>;
}

export interface NodeInspectorEntry {
	component?: any;
	panelHeaderComponent?: any;
}

export type NodeInspectorRegistry = Record<string, NodeInspectorEntry>;

const builtinNodeInspectorRegistry: NodeInspectorRegistry = {
	script: { component: ScriptNodeInspector },
	animation_curve: { component: AnimationCurveNodeInspector },
	gradient: { component: GradientNodeInspector }
};

const customNodeInspectorRegistry = new Map<string, NodeInspectorEntry>();

const normalizeInspectorKey = (key: string): string => key.trim();

export const registerNodeInspector = (key: string, entry: NodeInspectorEntry): void => {
	const normalizedKey = normalizeInspectorKey(key);
	if (!normalizedKey) {
		throw new Error('Node inspector registration requires a non-empty node type or item kind.');
	}
	customNodeInspectorRegistry.set(normalizedKey, entry);
};

export const registerNodeInspectors = (entries: NodeInspectorRegistry): void => {
	for (const [key, entry] of Object.entries(entries)) {
		registerNodeInspector(key, entry);
	}
};

export const unregisterNodeInspector = (key: string): void => {
	customNodeInspectorRegistry.delete(normalizeInspectorKey(key));
};

export const clearCustomNodeInspectors = (): void => {
	customNodeInspectorRegistry.clear();
};

export const resolveNodeInspector = (nodeOrType: UiNodeDto | string): NodeInspectorEntry | null => {
	const normalizedNodeType = normalizeInspectorKey(
		typeof nodeOrType === 'string' ? nodeOrType : nodeOrType.node_type
	);
	if (!normalizedNodeType) {
		return null;
	}
	const normalizedItemKind =
		typeof nodeOrType === 'string' ? '' : normalizeInspectorKey(nodeOrType.user_item_kind);
	return (
		customNodeInspectorRegistry.get(normalizedNodeType) ??
		builtinNodeInspectorRegistry[normalizedNodeType] ??
		(normalizedItemKind ? customNodeInspectorRegistry.get(normalizedItemKind) : null) ??
		null
	);
};
