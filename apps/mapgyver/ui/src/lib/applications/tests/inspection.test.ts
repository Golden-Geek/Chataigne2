import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ApplicationController, applicationState } from '../authoring.ts';
import { catalogState } from '../catalog.ts';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { ApplicationSnapshot } from '../../media/generated/ApplicationSnapshot';

const flush = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 1,
	loading: { visible: false }
});
const definition: ApplicationSnapshot = {
	id: 'selected',
	label: 'Selected',
	enablement: 'Enabled',
	revision: 1,
	definition_instance: 'instance',
	definition: {
		executable: null,
		arguments: [],
		directory: null,
		environment: null,
		window: 'Hidden',
		readiness: { kind: 'ProcessStarted' },
		stop: { graceful_ms: 20, retirement_ms: 100 },
		restart: { enabled: true, initial_ms: 10, maximum_ms: 100 }
	}
};

test('a read queued before deletion clears held Inspector data on authoritative absence and recovers on Undo', async () => {
	const session = connection(),
		state = applicationState();
	let complete!: (value: MediaEditResult) => void;
	let pending = false;
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_, edit) => {
			assert.equal(edit.kind, 'Applications');
			if (edit.edit.kind === 'InspectCatalog')
				return { kind: 'Applications', page: { applications: [], total: 0, start: 0 } };
			assert.equal(edit.edit.kind, 'Inspect', 'inspection never authors or launches');
			if (pending)
				return new Promise<MediaEditResult>((resolve) => {
					complete = resolve;
				});
			return { kind: 'Application', snapshot: definition };
		}
	);
	controller.observe();
	await flush();
	controller.select('selected');
	await flush();
	assert.equal(state.snapshot?.id, 'selected');
	pending = true;
	state.error = 'previous edit failed';
	controller.refresh();
	// Backend deletion can commit before this read arrives, while the UI still has its old graph.
	complete({ kind: 'ApplicationMissing', application: 'selected' });
	await flush();
	assert.equal(state.snapshot, null);
	assert.equal(state.missing, true);
	assert.equal(state.readError, '');
	assert.equal(state.error, '');
	pending = false;
	session.currentHistoryStateId++;
	controller.observe();
	await flush();
	assert.deepEqual(state.snapshot, definition);
	assert.equal(state.missing, false);
	controller.dispose();
});

test('late absence cannot clear another selection and a mismatched identity is rejected', async () => {
	const session = connection(),
		state = applicationState();
	const replies: ((value: MediaEditResult) => void)[] = [];
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async () => new Promise<MediaEditResult>((resolve) => replies.push(resolve))
	);
	controller.select('old');
	controller.select('selected');
	replies[0]({ kind: 'ApplicationMissing', application: 'old' });
	await flush();
	assert.equal(state.missing, false);
	replies[1]({ kind: 'Application', snapshot: definition });
	await flush();
	assert.deepEqual(state.snapshot, definition);
	controller.refresh();
	replies[2]({ kind: 'ApplicationMissing', application: 'different' });
	await flush();
	assert.match(state.readError, /Unexpected application snapshot/);
	assert.equal(state.missing, false);
	controller.dispose();
});
