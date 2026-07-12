import type { NodeId, UiNodeDto, UiParamValueProjection } from '../types';

export interface ViewportAnchorRect {
	left: number;
	top: number;
	right: number;
	bottom: number;
	width: number;
	height: number;
}

export interface NodePickerModalView {
	id: string;
	label: string;
	nodeVisibilityFilter?: (candidate: UiNodeDto) => boolean;
	nodeRowVisibilityFilter?: (candidate: UiNodeDto) => boolean;
}

export interface NodePickerModalOptions {
	rootNode: UiNodeDto | null;
	selectedNodeId?: NodeId | null;
	selectedProjection?: UiParamValueProjection | null;
	title?: string;
	placeholder?: string;
	searchPlaceholder?: string;
	defaultSearchQuery?: string;
	initiallyExpandedDepth?: number;
	clearable?: boolean;
	nodeFilter?: (candidate: UiNodeDto) => boolean;
	nodeVisibilityFilter?: (candidate: UiNodeDto) => boolean;
	nodeSearchText?: (candidate: UiNodeDto) => string;
	views?: NodePickerModalView[];
	projectionOptions?: (candidate: UiNodeDto) => UiParamValueProjection[];
	projectionRequired?: (candidate: UiNodeDto) => boolean;
	projectionLabel?: (projection: UiParamValueProjection) => string;
	anchorElement?: HTMLElement | null;
	anchorRect?: ViewportAnchorRect | null;
}

export type NodePickerModalResult =
	| { kind: 'pick'; node: UiNodeDto; projection?: UiParamValueProjection }
	| { kind: 'clear' }
	| { kind: 'cancel' };

interface NodePickerModalRequest {
	options: NodePickerModalOptions;
	resolve: (result: NodePickerModalResult) => void;
}

export const nodePickerModalState = $state({
	request: null as NodePickerModalRequest | null
});

const toAnchorRect = (
	rect: DOMRect | ViewportAnchorRect | null | undefined
): ViewportAnchorRect | null => {
	if (!rect) {
		return null;
	}
	return {
		left: rect.left,
		top: rect.top,
		right: rect.right,
		bottom: rect.bottom,
		width: rect.width,
		height: rect.height
	};
};

const normalizeOptions = (options: NodePickerModalOptions): NodePickerModalOptions => {
	const anchorRect =
		options.anchorRect ??
		(options.anchorElement ? toAnchorRect(options.anchorElement.getBoundingClientRect()) : null);
	return {
		selectedNodeId: null,
		selectedProjection: null,
		title: 'Select node',
		placeholder: 'Select node',
		searchPlaceholder: 'Search...',
		defaultSearchQuery: '',
		initiallyExpandedDepth: 5,
		clearable: true,
		nodeFilter: (_candidate: UiNodeDto) => true,
		nodeVisibilityFilter: (_candidate: UiNodeDto) => true,
		nodeSearchText: (candidate: UiNodeDto) =>
			`${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type}`,
		views: [],
		projectionOptions: undefined,
		projectionRequired: undefined,
		projectionLabel: undefined,
		anchorElement: null,
		...options,
		anchorRect
	};
};

const settle = (result: NodePickerModalResult): void => {
	const active = nodePickerModalState.request;
	if (!active) {
		return;
	}
	nodePickerModalState.request = null;
	active.resolve(result);
};

export const openNodePickerModal = (
	options: NodePickerModalOptions
): Promise<NodePickerModalResult> => {
	if (nodePickerModalState.request) {
		settle({ kind: 'cancel' });
	}
	const normalized = normalizeOptions(options);
	return new Promise<NodePickerModalResult>((resolve) => {
		nodePickerModalState.request = {
			options: normalized,
			resolve
		};
	});
};

export const closeNodePickerModal = (): void => {
	settle({ kind: 'cancel' });
};

export const resolveNodePickerModal = (result: NodePickerModalResult): void => {
	settle(result);
};
