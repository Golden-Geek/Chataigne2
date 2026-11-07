import { test } from 'node:test';
import assert from 'node:assert/strict';
import { invokeScreenEdit, screenOperationActivity } from '../operations.ts';
import type { ScreenEdit } from '../generated/ScreenEdit';
import type { UiAck } from 'golden_ui/types';

test('Surface geometry forwards applied receipts through the existing user operation', async () => {
	const ack: UiAck = {
		success: true,
		status: 'applied',
		latest_event_time: { tick: 5, micro: 0, seq: 1 }
	};
	let observed = false;
	const applied = (acks: readonly UiAck[]) => {
		assert.deepEqual(acks, [ack]);
		observed = true;
	};
	await invokeScreenEdit(
		{
			invokeAppOperation: async (request, observer, options) => {
				assert.equal(request.namespace, 'mapgyver.screens');
				assert.equal(observer, applied);
				assert.equal(options?.activity, 'user');
				observer?.([ack]);
				return { kind: 'Changed' };
			}
		},
		{
			kind: 'EditSurfaceGeometry',
			reference: { screen: 'screen', surface: 'surface' },
			edit: { kind: 'CropOrigin', point: { x: 0.2, y: 0.3 } }
		},
		applied
	);
	assert(observed);
});

test('each independent editor refresh stays background through the ordinary invocation boundary', async () => {
	for (const editor of ['Screen', 'Surface'] as const) {
		const edit: ScreenEdit = {
			kind: 'InspectEditor',
			query: { editor, selected: null, document: null, locked: null, retained: null, surface: null }
		};
		assert.equal(screenOperationActivity(edit), 'background');
		let activity: string | undefined;
		await invokeScreenEdit(
			{
				invokeAppOperation: async (_request, _applied, options) => {
					activity = options?.activity;
					return { kind: 'Editor', page: null };
				}
			},
			edit
		);
		assert.equal(activity, 'background');
	}
});

test('authoring still retains visible user activity', () => {
	assert.equal(screenOperationActivity({ kind: 'CreateScreen', label: null }), 'user');
});
