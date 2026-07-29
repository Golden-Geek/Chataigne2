import type { Snippet } from 'svelte';
import type { UiNodeDto } from '../../../types';
import type { NodePickerModalView } from '../../../store/node-picker-modal.svelte';
import ScriptNodeInspector from './nodes/ScriptNodeInspector.svelte';
import AnimationCurveNodeInspector from './nodes/AnimationCurveNodeInspector.svelte';
import GradientNodeInspector from './nodes/GradientNodeInspector.svelte';

export type NodeInspectorOrder = 'first' | 'last' | 'solo' | '';
export type NodeInspectorChildFilter = (node: UiNodeDto) => boolean;

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
	defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
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
export type NodeInspectorMatcher = (node: UiNodeDto) => boolean;

interface NodeInspectorMatcherEntry {
	key: string;
	matcher: NodeInspectorMatcher;
	entry: NodeInspectorEntry;
}

const builtinNodeInspectorRegistry: NodeInspectorRegistry = {
	script: { component: ScriptNodeInspector },
	animation_curve: { component: AnimationCurveNodeInspector },
	gradient: { component: GradientNodeInspector }
};

const customNodeInspectorRegistry = new Map<string, NodeInspectorEntry>();
const customNodeInspectorMatchers: NodeInspectorMatcherEntry[] = [];

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

export const registerNodeInspectorMatcher = (
	key: string,
	matcher: NodeInspectorMatcher,
	entry: NodeInspectorEntry
): void => {
	const normalizedKey = normalizeInspectorKey(key);
	if (!normalizedKey) {
		throw new Error('Node inspector matcher registration requires a non-empty key.');
	}
	const existingIndex = customNodeInspectorMatchers.findIndex(
		(candidate) => candidate.key === normalizedKey
	);
	const registration = { key: normalizedKey, matcher, entry };
	if (existingIndex >= 0) {
		customNodeInspectorMatchers[existingIndex] = registration;
		return;
	}
	customNodeInspectorMatchers.push(registration);
};

export const unregisterNodeInspector = (key: string): void => {
	customNodeInspectorRegistry.delete(normalizeInspectorKey(key));
};

export const unregisterNodeInspectorMatcher = (key: string): void => {
	const normalizedKey = normalizeInspectorKey(key);
	const existingIndex = customNodeInspectorMatchers.findIndex(
		(candidate) => candidate.key === normalizedKey
	);
	if (existingIndex >= 0) {
		customNodeInspectorMatchers.splice(existingIndex, 1);
	}
};

export const clearCustomNodeInspectors = (): void => {
	customNodeInspectorRegistry.clear();
	customNodeInspectorMatchers.splice(0);
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
	const matchedInspector =
		typeof nodeOrType === 'string'
			? null
			: (customNodeInspectorMatchers.find(({ matcher }) => matcher(nodeOrType))?.entry ?? null);
	return (
		customNodeInspectorRegistry.get(normalizedNodeType) ??
		matchedInspector ??
		builtinNodeInspectorRegistry[normalizedNodeType] ??
		(normalizedItemKind ? customNodeInspectorRegistry.get(normalizedItemKind) : null) ??
		null
	);
};
