import type { ChannelReference } from './generated/ChannelReference';

/** Identity must not depend on object-property order in different serializers. */
export const channelKey = (reference: ChannelReference): string =>
	JSON.stringify([reference.media, reference.channel]);
