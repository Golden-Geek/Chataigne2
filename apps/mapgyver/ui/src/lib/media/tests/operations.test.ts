import { test } from 'node:test';
import assert from 'node:assert/strict';
import { invokeMediaEdit, mediaOperationActivity } from '../operations.ts';
import type { WorkbenchSession } from 'golden_ui';
import type { UiAck } from 'golden_ui/types';

test('editor receipts remain on the ordinary authored operation and activity path', async () => {
	const ack: UiAck = {
		success: true,
		status: 'applied',
		latest_event_time: { tick: 8, micro: 0, seq: 1 }
	};
	let observed = false;
	const applied = (acks: readonly UiAck[]) => {
		assert.deepEqual(acks, [ack]);
		observed = true;
	};
	const session: Pick<WorkbenchSession, 'invokeAppOperation'> = {
		async invokeAppOperation(request, observer, options) {
			assert.equal(request.namespace, 'mapgyver.media');
			assert.equal(request.operation, 'edit');
			assert.equal(observer, applied);
			assert.equal(options?.activity, 'user');
			observer?.([ack]);
			return { kind: 'Changed' };
		}
	};
	assert.deepEqual(
		await invokeMediaEdit(
			session,
			{ kind: 'RenameMedia', media: 'media', label: 'Renamed' },
			applied
		),
		{ kind: 'Changed' }
	);
	assert(observed);
});

test('viewport and application inspections stay background while group authoring and launch retain user activity', () => {
	assert.equal(mediaOperationActivity({ kind: 'InspectVideo', media: 'video' }), 'background');
	assert.equal(
		mediaOperationActivity({
			kind: 'InspectVideoSource',
			reference: { media: 'video', channel: 'channel' }
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'InspectVideoFrame',
			reference: { media: 'video', channel: 'channel' }
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({ kind: 'Video', media: 'video', edit: { kind: 'Play' } }),
		'user'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'InspectSpatialEditor',
			query: { selected: null, document: null, locked: null, retained: null }
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'InspectMatrixEditor',
			query: {
				selected: null,
				document: null,
				retained: null,
				locked: null,
				viewport: { row_start: 0, row_count: 8, axis_start: 0, axis_count: 8 },
				selection: null,
				node: null
			}
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: {
				kind: 'InspectViewport',
				viewport: { row_start: 0, row_count: 1, axis_start: 0, axis_count: 1 },
				selection: null
			}
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Applications',
			edit: { kind: 'Inspect', application: 'group' }
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'AddColumnGroup', parent: null, label: null }
		}),
		'user'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'LaunchColumn', column: 'column', mode: 'Cut' }
		}),
		'user'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Applications',
			edit: { kind: 'SetFile', application: 'group', executable: null }
		}),
		'user'
	);
});
