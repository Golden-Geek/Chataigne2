import type { UiNodeDto } from 'golden_ui';
import { ASSET_NODE_TYPE } from './generated/constants.ts';

const structuralTypes = [
	'mapgyver.matrix.layer.v3',
	'mapgyver.matrix.group.v2',
	'mapgyver.matrix.column.v3',
	'mapgyver.matrix.column_group.v1',
	'mapgyver.matrix.cell.v4'
];

export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	[
		'mapgyver.media.reference.v4',
		'mapgyver.media.v11',
		'mapgyver.media.folder',
		ASSET_NODE_TYPE,
		...structuralTypes
	].includes(node.node_type);

/** Channels bridge the real Matrix hierarchy; Parameters and placement metadata stay hidden. */
export const canRenderMediaTreeChildren = (node: UiNodeDto): boolean =>
	[
		'mapgyver.media.folder',
		'mapgyver.media.v11',
		'mapgyver.media.channel.v2',
		'mapgyver.media.reference.output',
		...structuralTypes
	].includes(node.node_type);
