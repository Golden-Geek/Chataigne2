import assert from 'node:assert/strict';
import test from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { ChannelPageReader, channelPageState } from '../channel-page.ts';
import type { MediaEdit } from '../generated/MediaEdit';
import type { MediaEditResult } from '../generated/MediaEditResult';

function fixture() {
	const session = {
		status: 'connected',
		snapshotGeneration: 1,
		currentHistoryStateId: 1,
		loading: { visible: false },
		hasActiveEditSession: false
	};
	const state = channelPageState();
	const calls: {
		edit: MediaEdit;
		resolve(value: MediaEditResult): void;
		reject(error: Error): void;
	}[] = [];
	const reader = new ChannelPageReader(
		() => session,
		state,
		(_, edit) =>
			new Promise((resolve, reject) => {
				calls.push({ edit, resolve, reject });
			})
	);
	const result = (media: string, start = 0): MediaEditResult => ({
		kind: 'Channels',
		page: { media, start, status: 'Ready', total: 200, channels: [] }
	});
	return { session, state, calls, reader, result };
}

test('rapid selection/page changes keep one active read and only the latest replacement', async () => {
	const { state, calls, reader, result } = fixture();
	reader.refresh('first', 0);
	reader.refresh('second', 32);
	reader.refresh('third', 64);
	assert.equal(calls.length, 1);
	calls[0].resolve(result('first'));
	await delay(0);
	assert.equal(state.page, null);
	assert.equal(calls.length, 2);
	assert.deepEqual(calls[1].edit, {
		kind: 'InspectChannels',
		query: { media: 'third', start: 64, count: 32 }
	});
	calls[1].resolve(result('third', 64));
	await delay(0);
	assert.equal(reader.page?.media, 'third');
	assert.equal(state.loading, false);
});

test('history/project/disconnect/edit transitions hide accepted pages and reject late replies', async () => {
	for (const change of [
		(s: ReturnType<typeof fixture>['session']) => {
			s.currentHistoryStateId += 1;
		},
		(s: ReturnType<typeof fixture>['session']) => {
			s.snapshotGeneration += 1;
		},
		(s: ReturnType<typeof fixture>['session']) => {
			s.status = 'disconnected';
		},
		(s: ReturnType<typeof fixture>['session']) => {
			s.loading.visible = true;
		},
		(s: ReturnType<typeof fixture>['session']) => {
			s.hasActiveEditSession = true;
		}
	]) {
		const { session, calls, reader, result } = fixture();
		reader.refresh('reference', 0);
		calls[0].resolve(result('reference'));
		await delay(0);
		assert(reader.page);
		change(session);
		assert.equal(reader.page, null);
		reader.refresh('reference', 0);
		reader.dispose();
		if (calls.length > 1) calls[1].resolve(result('reference'));
		await delay(0);
		assert.equal(reader.page, null);
		const pending = fixture();
		pending.reader.refresh('reference', 0);
		change(pending.session);
		pending.calls[0].resolve(pending.result('reference'));
		await delay(0);
		assert.equal(pending.state.page, null);
		pending.reader.dispose();
	}
});

test('dispose retires queued work and obsolete errors without replacing the current state', async () => {
	const { state, calls, reader } = fixture();
	reader.refresh('old', 0);
	reader.refresh('new', 0);
	reader.dispose();
	calls[0].reject(new Error('obsolete'));
	await delay(0);
	assert.equal(calls.length, 1);
	assert.equal(reader.page, null);
	assert.equal(state.error, '');
	assert.equal(state.loading, false);
});
