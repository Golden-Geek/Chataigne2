import assert from 'node:assert/strict';
import test from 'node:test';
import { newerDiscovery } from '../discovery-state.ts';
import type { SenderDiscoverySnapshot } from '../generated/SenderDiscoverySnapshot';

test('a late scanning reply cannot overwrite the completed catalog event', () => {
	const completed: SenderDiscoverySnapshot = {
		revision: 2,
		refreshing: false,
		names: ['Exact sender'],
		truncated: false,
		error: null
	};
	const scanning = { ...completed, revision: 1, refreshing: true, names: [] };
	assert.equal(newerDiscovery(completed, scanning), completed);
	assert.equal(newerDiscovery(null, scanning), scanning);
	const restarted = { ...completed, revision: 4, names: ['Renamed sender'] };
	assert.equal(newerDiscovery(completed, restarted), restarted);
});
