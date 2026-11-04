import assert from 'node:assert/strict';
import test from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { MediaPreviewReader, previewReadState } from '../preview-reader.ts';
import { previewState } from '../preview-state.ts';
import type { MediaEdit } from '../generated/MediaEdit';
import type { MediaEditResult } from '../generated/MediaEditResult';
const id = '11111111-1111-1111-1111-111111111111';
test('panel presentation state is independently normalized without sharing mutable objects', () => {
	const first = previewState({ lockedMedia: id, document: id, channel: id, grid: true, page: 4 });
	const second = previewState();
	assert.equal(first.lockedMedia, id);
	assert.equal(second.lockedMedia, null);
	second.page = 5;
	assert.equal(first.page, 4);
	assert.equal(previewState({ page: -1, lockedMedia: 'invalid' }).page, 0);
	assert.equal(previewState({ lockedMedia: 'invalid' }).lockedMedia, null);
	assert.equal(previewState({ lockedMedia: id }).lockedMedia, null);
	assert.equal(previewState({ document: id }).document, null);
});
test('preview reads serialize rapid selections and fence project/history changes', async () => {
	const session = {
		status: 'connected',
		snapshotGeneration: 1,
		currentHistoryStateId: 1,
		loading: { visible: false },
		hasActiveEditSession: false
	};
	const state = previewReadState();
	const calls: { edit: MediaEdit; resolve: (reply: MediaEditResult) => void }[] = [];
	const reader = new MediaPreviewReader(
		() => session,
		state,
		(_, edit) => new Promise((resolve) => calls.push({ edit, resolve }))
	);
	const query = { selected: id, document: null, channel: null, start: 0, count: 4 };
	const reply: MediaEditResult = {
		kind: 'MediaPreview',
		page: {
			document: id,
			document_matches: true,
			media: id,
			label: 'Media',
			channels: null,
			selected: null
		}
	};
	reader.refresh(query);
	reader.refresh({ ...query, start: 4 });
	reader.refresh({ ...query, start: 8 });
	assert.equal(calls.length, 1);
	calls[0].resolve(reply);
	await delay(0);
	assert.equal(calls.length, 2);
	assert.equal(
		(calls[1].edit as Extract<MediaEdit, { kind: 'InspectMediaPreview' }>).query.start,
		8
	);
	session.snapshotGeneration++;
	calls[1].resolve(reply);
	await delay(0);
	assert.equal(reader.page, null);
	reader.refresh(query);
	calls[2].resolve(reply);
	await delay(0);
	assert(reader.page);
	session.currentHistoryStateId++;
	assert(reader.page);
	session.hasActiveEditSession = true;
	reader.refresh(query);
	assert(reader.page);
	assert.equal(calls.length, 3);
	session.hasActiveEditSession = false;
	reader.refresh(query);
	session.currentHistoryStateId++;
	calls[3].resolve({ ...reply, page: { ...reply.page, label: 'Stale' } });
	await delay(0);
	assert.equal(state.page?.label, 'Media');
	session.snapshotGeneration++;
	assert.equal(reader.page, null);
	reader.dispose();
	assert.equal(state.page, null);
});

test('selection changes retain valid content until the latest reply and never accept superseded views', async () => {
	const session = {
		status: 'connected',
		snapshotGeneration: 1,
		currentHistoryStateId: 1,
		loading: { visible: false }
	};
	const state = previewReadState();
	const calls: { resolve: (reply: MediaEditResult) => void }[] = [];
	const reader = new MediaPreviewReader(
		() => session,
		state,
		() => new Promise((resolve) => calls.push({ resolve }))
	);
	const query = { selected: id, document: null, channel: null, start: 0, count: 4 };
	const reply = (label: string): MediaEditResult => ({
		kind: 'MediaPreview',
		page: { document: id, document_matches: true, media: id, label, channels: null, selected: null }
	});
	reader.refresh(query);
	calls[0].resolve(reply('Original'));
	await delay(0);
	const original = reader.page;
	reader.refresh({ ...query, selected: '22222222-2222-2222-2222-222222222222' });
	assert.equal(reader.page, original, 'pending selection retains the displayed page');
	reader.refresh({ ...query, selected: '33333333-3333-3333-3333-333333333333' });
	calls[1].resolve(reply('Superseded'));
	await delay(0);
	assert.equal(reader.page, original, 'superseded reply never replaces the retained page');
	calls[2].resolve(reply('Latest'));
	await delay(0);
	assert.equal(reader.page?.label, 'Latest');
	reader.dispose();
});

test('retained selection content is hidden immediately when its document or connection changes', async () => {
	const session = {
		status: 'connected',
		snapshotGeneration: 1,
		currentHistoryStateId: 1,
		loading: { visible: false }
	};
	const state = previewReadState();
	const calls: { resolve: (reply: MediaEditResult) => void }[] = [];
	const reader = new MediaPreviewReader(
		() => session,
		state,
		() => new Promise((resolve) => calls.push({ resolve }))
	);
	const query = { selected: id, document: null, channel: null, start: 0, count: 4 };
	const reply: MediaEditResult = {
		kind: 'MediaPreview',
		page: {
			document: id,
			document_matches: true,
			media: id,
			label: 'Original',
			channels: null,
			selected: null
		}
	};
	reader.refresh(query);
	calls[0].resolve(reply);
	await delay(0);
	reader.refresh({ ...query, start: 4 });
	assert(reader.page);
	session.snapshotGeneration++;
	assert.equal(reader.page, null);
	calls[1].resolve(reply);
	await delay(0);
	assert.equal(reader.page, null, 'old response cannot revive a prior document');
	reader.refresh(query);
	calls[2].resolve(reply);
	await delay(0);
	assert(reader.page);
	session.status = 'disconnected';
	assert.equal(reader.page, null);
	reader.dispose();
});
