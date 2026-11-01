import { appState, type UiNodeDto, type UiCreatableUserItem } from 'golden_ui';
import type { MediaKind } from './generated/MediaKind';
import { invokeMediaEdit } from './operations';
export const mediaTreeItems: UiCreatableUserItem[] = [
	'Folder',
	...(['SolidColor', 'Picture', 'SharedTexture', 'Composition', 'Matrix'] satisfies MediaKind[])
].map((kind) => ({
	node_type: kind,
	item_kind: 'media',
	label: kind === 'SolidColor' ? 'Solid Color' : kind === 'SharedTexture' ? 'Shared Texture' : kind,
	menu_path: [],
	initial_params: [],
	select_when_created: true
}));
export const isMediaTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
	!!node &&
	[
		'mapgyver.media',
		'mapgyver.media.channel',
		'mapgyver.media.folder',
		'mapgyver.media.asset'
	].includes(node.node_type);
export async function createMediaTreeItem(
	parent: UiNodeDto,
	item: UiCreatableUserItem,
	selectDocumentNode: (uuid: string) => void
) {
	const session = appState.session;
	if (
		!session ||
		session.status !== 'connected' ||
		session.loading.visible ||
		session.hasActiveEditSession
	)
		return;
	const folder = parent.node_type === 'mapgyver.media.folder' ? parent.uuid : null;
	const result = await invokeMediaEdit(
		session,
		item.node_type === 'Folder'
			? { kind: 'CreateFolder', parent: folder, label: null }
			: folder
				? {
						kind: 'CreateMediaInFolder',
						folder,
						media_kind: item.node_type as MediaKind,
						label: null
					}
				: { kind: 'CreateMedia', media_kind: item.node_type as MediaKind, label: null }
	);
	if (result.kind === 'Created') selectDocumentNode(result.reference.channel);
	else if (result.kind === 'FolderCreated') selectDocumentNode(result.folder);
}
