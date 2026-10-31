import type { ChannelReference } from './generated/ChannelReference';

/** Identity must not depend on object-property order in different serializers. */
export const channelKey = (reference: ChannelReference): string =>
	JSON.stringify([reference.media, reference.channel]);

/** Decode a picker value without substituting another source if the catalog changes. */
export const channelReferenceFromKey = (key: string): ChannelReference => {
	const parts: unknown = JSON.parse(key);
	if (
		!Array.isArray(parts) ||
		parts.length !== 2 ||
		!parts.every((part) => typeof part === 'string')
	)
		throw new Error('Invalid channel selection');
	return { media: parts[0], channel: parts[1] };
};
