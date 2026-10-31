import assert from 'node:assert/strict';
import { test } from 'node:test';
import { channelKey, channelReferenceFromKey } from '../identity.ts';

test('snapshot and native-event references match despite different property order', () => {
	const snapshot = { channel: 'channel-a', media: 'media-a' };
	const nativeEvent = { media: 'media-a', channel: 'channel-a' };
	assert.notEqual(JSON.stringify(snapshot), JSON.stringify(nativeEvent));
	assert.equal(channelKey(snapshot), channelKey(nativeEvent));
	assert.notEqual(channelKey(snapshot), channelKey({ ...snapshot, media: 'media-b' }));
	assert.notEqual(channelKey(snapshot), channelKey({ ...snapshot, channel: 'channel-b' }));
});

test('picker values retain exact identities independently of a changing catalog', () => {
	const selected = { media: 'media-a', channel: 'channel-a' };
	assert.deepEqual(channelReferenceFromKey(channelKey(selected)), selected);
	for (const invalid of ['null', '{}', '["only-one"]', '["a", 2]', '["a", "b", "c"]'])
		assert.throws(() => channelReferenceFromKey(invalid));
});
