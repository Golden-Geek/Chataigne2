import type { UiNodeDto } from 'golden_ui';

export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	[
		'mapgyver.media',
		'mapgyver.media.channel',
		'mapgyver.media.folder',
		'mapgyver.media.asset'
	].includes(node.node_type);
