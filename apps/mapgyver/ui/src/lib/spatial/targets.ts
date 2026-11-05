import type { SpatialEditorTarget } from '../media/generated/SpatialEditorTarget';
export const targetKey = (target: SpatialEditorTarget | null | undefined): string =>
	!target
		? ''
		: target.kind === 'Matrix'
			? `matrix:${target.media}`
			: `composition:${target.reference.media}:${target.reference.channel}`;
export const sameTarget = (
	left: SpatialEditorTarget | null | undefined,
	right: SpatialEditorTarget | null | undefined
) => targetKey(left) === targetKey(right);

const uuid = (value: unknown): value is string =>
	typeof value === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value);
function target(value: SpatialEditorTarget | null | undefined): SpatialEditorTarget | null {
	if (value?.kind === 'Matrix' && uuid(value.media)) return { kind: 'Matrix', media: value.media };
	if (
		value?.kind === 'Composition' &&
		uuid(value.reference?.media) &&
		uuid(value.reference?.channel)
	)
		return {
			kind: 'Composition',
			reference: { media: value.reference.media, channel: value.reference.channel }
		};
	return null;
}
export interface SpatialPanelState {
	document: string | null;
	locked: SpatialEditorTarget | null;
	retained: SpatialEditorTarget | null;
	page: number;
}
export function spatialPanelState(value: Partial<SpatialPanelState> = {}): SpatialPanelState {
	const document = uuid(value.document) ? value.document : null;
	return {
		document,
		locked: document ? target(value.locked) : null,
		retained: document ? target(value.retained) : null,
		page:
			Number.isSafeInteger(value.page) && value.page! >= 0 ? Math.min(value.page!, 0x1ffffff) : 0
	};
}
export const SPATIAL_PAGE_SIZE = 128;
