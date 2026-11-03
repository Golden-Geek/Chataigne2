/** Presentation choices only; media ownership and channel defaults are resolved by the backend. */
export interface MediaPreviewState {
	lockedMedia: string | null;
	document: string | null;
	channel: string | null;
	grid: boolean;
	page: number;
}
const uuid = (value: unknown): string | null =>
	typeof value === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value)
		? value
		: null;
export const previewState = (value: Partial<MediaPreviewState> = {}): MediaPreviewState => {
	const media = uuid(value.lockedMedia);
	const document = uuid(value.document);
	return {
		lockedMedia: media && document ? media : null,
		document: media && document ? document : null,
		channel: uuid(value.channel),
		grid: value.grid === true,
		page:
			Number.isSafeInteger(value.page) && value.page! >= 0 ? Math.min(value.page!, 0x3fffffff) : 0
	};
};
// Four bounded tiles per panel allow two grids within the existing eight-view host admission.
export const PREVIEW_PAGE_SIZE = 4;
