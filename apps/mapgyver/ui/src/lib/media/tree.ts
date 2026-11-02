import type { UiNodeDto } from 'golden_ui';

export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	['mapgyver.media.v5', 'mapgyver.media.folder', 'mapgyver.media.asset'].includes(node.node_type);
