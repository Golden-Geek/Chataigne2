import type { UiNodeDto } from 'golden_ui';
import type { MediaKind } from '../media/generated/MediaKind';

/** Read the declared immutable kind for presentation; authored domain behavior stays backend-owned. */
export function mediaIconKind(
	node: UiNodeDto,
	read: (id: number) => UiNodeDto | null | undefined
): MediaKind | null {
	if (node.node_type === 'mapgyver.media.reference.v4') return 'Reference';
	if (node.node_type !== 'mapgyver.media.v8') return null;
	const parameters = node.children.find((id) => read(id)?.decl_id === 'parameters');
	if (parameters === undefined) return null;
	const folder = read(parameters);
	const leaf = folder?.children.find((id) => read(id)?.decl_id === 'parameters/media_kind');
	const data = leaf === undefined ? null : read(leaf)?.data;
	if (data?.kind !== 'parameter' || data.param.value.kind !== 'enum') return null;
	const kind = data.param.value.value;
	return ['Picture', 'Video', 'SolidColor', 'SharedTexture', 'Composition', 'Matrix'].includes(kind)
		? (kind as MediaKind)
		: null;
}
