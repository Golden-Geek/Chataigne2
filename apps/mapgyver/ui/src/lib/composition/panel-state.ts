import type { ChannelReference } from '../media/generated/ChannelReference';

export interface CompositionPanelState {
	locked: ChannelReference | null;
	document: string | null;
	page: number;
}
const uuid = (value: unknown): value is string =>
	typeof value === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value);
export function compositionPanelState(
	value: Partial<CompositionPanelState> = {}
): CompositionPanelState {
	const validLock =
		uuid(value.document) && uuid(value.locked?.media) && uuid(value.locked?.channel);
	return {
		locked: validLock ? { ...value.locked! } : null,
		document: validLock ? value.document! : null,
		page:
			Number.isSafeInteger(value.page) && value.page! >= 0 ? Math.min(value.page!, 0x1ffffff) : 0
	};
}
export const COMPOSITION_PAGE_SIZE = 128;
