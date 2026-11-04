import type { UiNodeDto } from 'golden_ui';
import { ASSET_NODE_TYPE } from './generated/constants';

export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	[
		'mapgyver.media.reference.v3',
		'mapgyver.media.v7',
		'mapgyver.media.folder',
		ASSET_NODE_TYPE
	].includes(node.node_type);
