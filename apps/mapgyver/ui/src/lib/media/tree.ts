import type { UiNodeDto } from 'golden_ui';

export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	[
		'mapgyver.media.reference.v3',
		'mapgyver.media.v7',
		'mapgyver.media.folder',
		'mapgyver.media.asset'
	].includes(node.node_type);
