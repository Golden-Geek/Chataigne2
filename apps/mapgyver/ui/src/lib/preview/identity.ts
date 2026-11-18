import type { PreviewSource } from './generated/PreviewSource';

/** Stable across serde and JS object-property order, with distinct source namespaces. */
export const previewKey = (source: PreviewSource): string =>
	source.kind === 'Channel' || source.kind === 'MatrixPlayback'
		? JSON.stringify([source.kind, source.reference.media, source.reference.channel])
		: source.kind === 'MatrixCell'
			? JSON.stringify(['MatrixCell', source.media, source.cell])
			: source.kind === 'Screen'
				? JSON.stringify(['Screen', source.screen])
				: JSON.stringify(['Calibration', source.mode, source.extent.width, source.extent.height]);
