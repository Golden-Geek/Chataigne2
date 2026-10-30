import type { PreviewSource } from './generated/PreviewSource';

/** Stable across serde and JS object-property order, with distinct source namespaces. */
export const previewKey = (source: PreviewSource): string =>
	source.kind === 'Channel'
		? JSON.stringify(['Channel', source.reference.media, source.reference.channel])
		: JSON.stringify(['Screen', source.screen]);
